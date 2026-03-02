//! REST API routes for the relay server.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::relay::room::{BufferedMessage, MessageDirection};
use crate::relay::RoomManager;

#[derive(Clone)]
pub struct AppState {
    pub room_manager: Arc<RoomManager>,
    pub start_time: std::time::Instant,
    /// Base directory for per-room uploaded mobile-web files.
    pub room_web_dir: String,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub rooms: usize,
    pub connections: usize,
}

pub async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        rooms: state.room_manager.room_count(),
        connections: state.room_manager.connection_count(),
    })
}

#[derive(Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub protocol_version: u8,
}

pub async fn server_info() -> Json<ServerInfo> {
    Json(ServerInfo {
        name: "BitFun Relay Server".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: 1,
    })
}

#[derive(Deserialize)]
pub struct JoinRoomRequest {
    pub device_id: String,
    pub device_type: String,
    pub public_key: String,
}

/// `POST /api/rooms/:room_id/join`
pub async fn join_room(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<JoinRoomRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let conn_id = state.room_manager.next_conn_id();
    let existing_peer = state.room_manager.get_peer_info(&room_id, conn_id);

    let ok = state.room_manager.join_room(
        &room_id,
        conn_id,
        &body.device_id,
        &body.device_type,
        &body.public_key,
        None, // HTTP client, no websocket tx
    );

    if ok {
        let joiner_notification = serde_json::to_string(&crate::routes::websocket::OutboundProtocol::PeerJoined {
            device_id: body.device_id.clone(),
            device_type: body.device_type.clone(),
            public_key: body.public_key.clone(),
        }).unwrap_or_default();
        state.room_manager.send_to_others_in_room(&room_id, conn_id, &joiner_notification);

        if let Some((peer_did, peer_dt, peer_pk)) = existing_peer {
            Ok(Json(serde_json::json!({
                "status": "joined",
                "peer": {
                    "device_id": peer_did,
                    "device_type": peer_dt,
                    "public_key": peer_pk
                }
            })))
        } else {
            Ok(Json(serde_json::json!({
                "status": "joined",
                "peer": null
            })))
        }
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

#[derive(Deserialize)]
pub struct RelayMessageRequest {
    pub device_id: String,
    pub encrypted_data: String,
    pub nonce: String,
}

/// `POST /api/rooms/:room_id/message`
pub async fn relay_message(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<RelayMessageRequest>,
) -> StatusCode {
    // Find conn_id by device_id in the room
    if let Some(conn_id) = state.room_manager.get_conn_id_by_device(&room_id, &body.device_id) {
        if state.room_manager.relay_message(conn_id, &body.encrypted_data, &body.nonce) {
            StatusCode::OK
        } else {
            StatusCode::NOT_FOUND
        }
    } else {
        StatusCode::UNAUTHORIZED
    }
}

#[derive(Deserialize)]
pub struct PollQuery {
    pub since_seq: Option<u64>,
    pub device_type: Option<String>,
}

#[derive(Serialize)]
pub struct PollResponse {
    pub messages: Vec<BufferedMessage>,
    pub peer_connected: bool,
}

/// `GET /api/rooms/:room_id/poll?since_seq=0&device_type=mobile`
pub async fn poll_messages(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Query(query): Query<PollQuery>,
) -> Result<Json<PollResponse>, StatusCode> {
    let since = query.since_seq.unwrap_or(0);
    let direction = match query.device_type.as_deref() {
        Some("desktop") => MessageDirection::ToDesktop,
        _ => MessageDirection::ToMobile,
    };
    
    let peer_connected = state.room_manager.has_peer(&room_id, query.device_type.as_deref().unwrap_or("mobile"));
    let messages = state.room_manager.poll_messages(&room_id, direction, since);
    
    Ok(Json(PollResponse { messages, peer_connected }))
}

#[derive(Deserialize)]
pub struct AckRequest {
    pub ack_seq: u64,
    pub device_type: Option<String>,
}

/// `POST /api/rooms/:room_id/ack`
pub async fn ack_messages(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<AckRequest>,
) -> StatusCode {
    let direction = match body.device_type.as_deref() {
        Some("desktop") => MessageDirection::ToDesktop,
        _ => MessageDirection::ToMobile,
    };
    state
        .room_manager
        .ack_messages(&room_id, direction, body.ack_seq);
    StatusCode::OK
}

// ── Per-room mobile-web upload & serving ───────────────────────────────────

#[derive(Deserialize)]
pub struct UploadWebRequest {
    pub files: HashMap<String, String>,
}

/// `POST /api/rooms/:room_id/upload-web`
///
/// Desktop uploads mobile-web dist files (base64-encoded) so the mobile
/// browser can load the exact same version the desktop is running.
pub async fn upload_web(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<UploadWebRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};

    if !state.room_manager.room_exists(&room_id) {
        return Err(StatusCode::NOT_FOUND);
    }

    let room_dir = std::path::PathBuf::from(&state.room_web_dir).join(&room_id);
    if let Err(e) = std::fs::create_dir_all(&room_dir) {
        tracing::error!("Failed to create room web dir {}: {e}", room_dir.display());
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let mut written = 0usize;
    for (rel_path, b64_content) in &body.files {
        if rel_path.contains("..") {
            continue;
        }
        let decoded = B64.decode(b64_content).map_err(|_| StatusCode::BAD_REQUEST)?;
        let dest = room_dir.join(rel_path);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&dest, &decoded).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        written += 1;
    }

    tracing::info!("Room {room_id}: uploaded {written} mobile-web files");
    Ok(Json(serde_json::json!({ "status": "ok", "files_written": written })))
}

/// `GET /r/{*rest}` — serve per-room mobile-web static files.
///
/// The `rest` path is expected to be `room_id` or `room_id/file/path`.
/// Falls back to `index.html` for SPA routing.
pub async fn serve_room_web_catchall(
    State(state): State<AppState>,
    Path(rest): Path<String>,
) -> Result<axum::response::Response, StatusCode> {
    use axum::body::Body;
    use axum::http::header;
    use axum::response::IntoResponse;

    let rest = rest.trim_start_matches('/');
    let (room_id, file_path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx + 1..]),
        None => (rest, ""),
    };

    if room_id.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    let room_dir = std::path::PathBuf::from(&state.room_web_dir).join(room_id);
    if !room_dir.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let target = if file_path.is_empty() {
        room_dir.join("index.html")
    } else {
        room_dir.join(file_path)
    };

    let file = if target.is_file() {
        target
    } else {
        room_dir.join("index.html")
    };

    if !file.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }

    let content = std::fs::read(&file).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mime = mime_from_path(&file);

    Ok(([(header::CONTENT_TYPE, mime)], Body::from(content)).into_response())
}

fn mime_from_path(p: &std::path::Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// Remove the per-room web directory (called on room cleanup).
pub fn cleanup_room_web(room_web_dir: &str, room_id: &str) {
    let dir = std::path::PathBuf::from(room_web_dir).join(room_id);
    if dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            tracing::warn!("Failed to clean up room web dir {}: {e}", dir.display());
        } else {
            tracing::info!("Cleaned up room web dir for {room_id}");
        }
    }
}
