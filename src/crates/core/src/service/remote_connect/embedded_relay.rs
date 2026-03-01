//! Embedded mini relay server for LAN / ngrok modes.
//!
//! Runs inside the desktop process using axum + WebSocket.
//! Supports the same protocol as the standalone relay-server.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tokio::sync::mpsc;

type ConnId = u64;

struct Participant {
    conn_id: ConnId,
    device_id: String,
    device_type: String,
    public_key: String,
    tx: mpsc::UnboundedSender<String>,
}

struct Room {
    participants: Vec<Participant>,
}

struct RelayState {
    rooms: DashMap<String, Room>,
    conn_to_room: DashMap<ConnId, String>,
    next_id: AtomicU64,
}

impl RelayState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            rooms: DashMap::new(),
            conn_to_room: DashMap::new(),
            next_id: AtomicU64::new(1),
        })
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Inbound {
    CreateRoom {
        room_id: Option<String>,
        device_id: String,
        device_type: String,
        public_key: String,
    },
    JoinRoom {
        room_id: String,
        device_id: String,
        device_type: String,
        public_key: String,
    },
    Relay {
        #[allow(dead_code)]
        room_id: String,
        encrypted_data: String,
        nonce: String,
    },
    Heartbeat,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Outbound {
    RoomCreated { room_id: String },
    PeerJoined { device_id: String, device_type: String, public_key: String },
    Relay { room_id: String, encrypted_data: String, nonce: String },
    PeerDisconnected { device_id: String },
    HeartbeatAck,
    Error { message: String },
}

/// Start the embedded relay and return a shutdown handle.
/// The server listens on `0.0.0.0:{port}`.
pub async fn start_embedded_relay(port: u16) -> anyhow::Result<EmbeddedRelayHandle> {
    let state = RelayState::new();
    let app_state = state.clone();

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/health", get(health))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .map_err(|e| anyhow::anyhow!("failed to bind embedded relay on port {port}: {e}"))?;

    info!("Embedded relay started on 0.0.0.0:{port}");

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async { let _ = shutdown_rx.await; })
            .await
            .ok();
    });

    // Brief wait for server to be ready
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    Ok(EmbeddedRelayHandle { _shutdown: Some(shutdown_tx) })
}

pub struct EmbeddedRelayHandle {
    _shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl EmbeddedRelayHandle {
    pub fn stop(&mut self) {
        if let Some(tx) = self._shutdown.take() {
            let _ = tx.send(());
            info!("Embedded relay stopped");
        }
    }
}

impl Drop for EmbeddedRelayHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "healthy"}))
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<RelayState>>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<RelayState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();

    let conn_id = state.next_id.fetch_add(1, Ordering::Relaxed);

    let write_task = tokio::spawn(async move {
        while let Some(text) = out_rx.recv().await {
            if ws_tx.send(Message::Text(text)).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = ws_rx.next().await {
        if let Message::Text(text) = msg {
            handle_msg(&text, conn_id, &state, &out_tx);
        }
    }

    on_disconnect(conn_id, &state);
    drop(out_tx);
    let _ = write_task.await;
    debug!("Embedded relay: conn {conn_id} closed");
}

fn handle_msg(
    text: &str,
    conn_id: ConnId,
    state: &Arc<RelayState>,
    out_tx: &mpsc::UnboundedSender<String>,
) {
    let msg: Inbound = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            send(out_tx, &Outbound::Error { message: format!("bad message: {e}") });
            return;
        }
    };

    match msg {
        Inbound::CreateRoom { room_id, device_id, device_type, public_key } => {
            let room_id = room_id.unwrap_or_else(gen_room_id);
            let mut room = Room { participants: Vec::with_capacity(2) };
            room.participants.push(Participant {
                conn_id, device_id, device_type, public_key, tx: out_tx.clone(),
            });
            state.rooms.insert(room_id.clone(), room);
            state.conn_to_room.insert(conn_id, room_id.clone());
            send(out_tx, &Outbound::RoomCreated { room_id });
        }

        Inbound::JoinRoom { room_id, device_id, device_type, public_key } => {
            let existing_peer = state.rooms.get(&room_id).and_then(|r| {
                r.participants.first().map(|p| (p.device_id.clone(), p.device_type.clone(), p.public_key.clone()))
            });

            let ok = if let Some(mut room) = state.rooms.get_mut(&room_id) {
                if room.participants.len() < 2 {
                    room.participants.push(Participant {
                        conn_id,
                        device_id: device_id.clone(),
                        device_type: device_type.clone(),
                        public_key: public_key.clone(),
                        tx: out_tx.clone(),
                    });
                    state.conn_to_room.insert(conn_id, room_id.clone());
                    true
                } else {
                    false
                }
            } else {
                false
            };

            if ok {
                if let Some(room) = state.rooms.get(&room_id) {
                    for p in &room.participants {
                        if p.conn_id != conn_id {
                            send(&p.tx, &Outbound::PeerJoined {
                                device_id: device_id.clone(),
                                device_type: device_type.clone(),
                                public_key: public_key.clone(),
                            });
                        }
                    }
                }
                if let Some((pdid, pdt, ppk)) = existing_peer {
                    send(out_tx, &Outbound::PeerJoined {
                        device_id: pdid, device_type: pdt, public_key: ppk,
                    });
                }
            } else {
                send(out_tx, &Outbound::Error { message: format!("cannot join room {room_id}") });
            }
        }

        Inbound::Relay { room_id, encrypted_data, nonce } => {
            if let Some(rid) = state.conn_to_room.get(&conn_id) {
                if let Some(room) = state.rooms.get(rid.value()) {
                    let relay_json = serde_json::to_string(&Outbound::Relay {
                        room_id: room_id.clone(), encrypted_data, nonce,
                    }).unwrap_or_default();
                    for p in &room.participants {
                        if p.conn_id != conn_id {
                            let _ = p.tx.send(relay_json.clone());
                        }
                    }
                }
            }
        }

        Inbound::Heartbeat => {
            send(out_tx, &Outbound::HeartbeatAck);
        }
    }
}

fn on_disconnect(conn_id: ConnId, state: &Arc<RelayState>) {
    if let Some((_, room_id)) = state.conn_to_room.remove(&conn_id) {
        let mut should_remove = false;
        if let Some(mut room) = state.rooms.get_mut(&room_id) {
            let removed = room.participants.iter().position(|p| p.conn_id == conn_id);
            if let Some(idx) = removed {
                let p = room.participants.remove(idx);
                let notif = serde_json::to_string(&Outbound::PeerDisconnected {
                    device_id: p.device_id,
                }).unwrap_or_default();
                for other in &room.participants {
                    let _ = other.tx.send(notif.clone());
                }
            }
            should_remove = room.participants.is_empty();
        }
        if should_remove {
            state.rooms.remove(&room_id);
        }
    }
}

fn send(tx: &mpsc::UnboundedSender<String>, msg: &Outbound) {
    if let Ok(json) = serde_json::to_string(msg) {
        let _ = tx.send(json);
    }
}

fn gen_room_id() -> String {
    let bytes: [u8; 6] = rand::random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
