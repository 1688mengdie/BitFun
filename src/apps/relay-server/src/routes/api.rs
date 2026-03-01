//! REST API routes for the relay server.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::relay::room::{BufferedMessage, MessageDirection};
use crate::relay::RoomManager;

#[derive(Clone)]
pub struct AppState {
    pub room_manager: Arc<RoomManager>,
    pub start_time: std::time::Instant,
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

// ── Polling API ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PollQuery {
    pub since_seq: Option<u64>,
    pub device_type: Option<String>,
}

#[derive(Serialize)]
pub struct PollResponse {
    pub messages: Vec<BufferedMessage>,
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
    let messages = state.room_manager.poll_messages(&room_id, direction, since);
    Ok(Json(PollResponse { messages }))
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
