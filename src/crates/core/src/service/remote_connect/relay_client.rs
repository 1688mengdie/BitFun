//! WebSocket client for connecting to the Relay Server.
//!
//! Manages the desktop-side WebSocket connection, sends/receives relay protocol messages,
//! and dispatches events to the pairing and session bridge layers.

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::tungstenite::Message;

/// Messages in the relay protocol (both directions).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RelayMessage {
    CreateRoom {
        room_id: Option<String>,
        device_id: String,
        device_type: String,
        public_key: String,
    },
    RoomCreated {
        room_id: String,
    },
    JoinRoom {
        room_id: String,
        device_id: String,
        device_type: String,
        public_key: String,
    },
    PeerJoined {
        device_id: String,
        device_type: String,
        public_key: String,
    },
    Relay {
        room_id: String,
        encrypted_data: String,
        nonce: String,
    },
    Heartbeat,
    HeartbeatAck,
    PeerDisconnected {
        device_id: String,
    },
    Error {
        message: String,
    },
}

/// Events emitted by the relay client to the upper layers.
#[derive(Debug, Clone)]
pub enum RelayEvent {
    Connected,
    RoomCreated { room_id: String },
    PeerJoined { public_key: String, device_id: String },
    MessageReceived { encrypted_data: String, nonce: String },
    PeerDisconnected { device_id: String },
    Disconnected,
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

pub struct RelayClient {
    state: Arc<RwLock<ConnectionState>>,
    event_tx: mpsc::UnboundedSender<RelayEvent>,
    cmd_tx: Arc<RwLock<Option<mpsc::UnboundedSender<RelayMessage>>>>,
    room_id: Arc<RwLock<Option<String>>>,
}

impl RelayClient {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<RelayEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let client = Self {
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            event_tx,
            cmd_tx: Arc::new(RwLock::new(None)),
            room_id: Arc::new(RwLock::new(None)),
        };
        (client, event_rx)
    }

    pub async fn connection_state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// Connect to the relay server WebSocket endpoint.
    pub async fn connect(&self, ws_url: &str) -> Result<()> {
        *self.state.write().await = ConnectionState::Connecting;

        let (ws_stream, _) = tokio_tungstenite::connect_async(ws_url)
            .await
            .map_err(|e| anyhow!("WebSocket connect failed: {e}"))?;

        info!("Connected to relay server at {ws_url}");
        *self.state.write().await = ConnectionState::Connected;
        let _ = self.event_tx.send(RelayEvent::Connected);

        let (mut write, mut read) = ws_stream.split();
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<RelayMessage>();
        *self.cmd_tx.write().await = Some(cmd_tx);

        let event_tx = self.event_tx.clone();
        let state = self.state.clone();
        let room_id_store = self.room_id.clone();

        // Read task
        let read_state = state.clone();
        let read_event_tx = event_tx.clone();
        let read_room_id = room_id_store.clone();
        tokio::spawn(async move {
            while let Some(msg_result) = read.next().await {
                match msg_result {
                    Ok(Message::Text(text)) => {
                        match serde_json::from_str::<RelayMessage>(&text) {
                            Ok(relay_msg) => {
                                Self::handle_message(
                                    relay_msg,
                                    &read_event_tx,
                                    &read_room_id,
                                )
                                .await;
                            }
                            Err(e) => {
                                warn!("Failed to parse relay message: {e}");
                            }
                        }
                    }
                    Ok(Message::Ping(_)) => {}
                    Ok(Message::Close(_)) => {
                        info!("Relay server closed connection");
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket read error: {e}");
                        break;
                    }
                    _ => {}
                }
            }
            *read_state.write().await = ConnectionState::Disconnected;
            let _ = read_event_tx.send(RelayEvent::Disconnected);
        });

        // Write task
        tokio::spawn(async move {
            while let Some(msg) = cmd_rx.recv().await {
                match serde_json::to_string(&msg) {
                    Ok(json) => {
                        if let Err(e) = write.send(Message::Text(json)).await {
                            error!("WebSocket write error: {e}");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to serialize relay message: {e}");
                    }
                }
            }
        });

        // Heartbeat task
        let hb_cmd_tx = self.cmd_tx.clone();
        let hb_state = self.state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                if *hb_state.read().await != ConnectionState::Connected {
                    break;
                }
                if let Some(tx) = hb_cmd_tx.read().await.as_ref() {
                    let _ = tx.send(RelayMessage::Heartbeat);
                }
            }
        });

        Ok(())
    }

    async fn handle_message(
        msg: RelayMessage,
        event_tx: &mpsc::UnboundedSender<RelayEvent>,
        room_id_store: &Arc<RwLock<Option<String>>>,
    ) {
        match msg {
            RelayMessage::RoomCreated { room_id } => {
                debug!("Room created: {room_id}");
                *room_id_store.write().await = Some(room_id.clone());
                let _ = event_tx.send(RelayEvent::RoomCreated { room_id });
            }
            RelayMessage::PeerJoined {
                device_id,
                public_key,
                ..
            } => {
                info!("Peer joined: {device_id}");
                let _ = event_tx.send(RelayEvent::PeerJoined {
                    public_key,
                    device_id,
                });
            }
            RelayMessage::Relay {
                encrypted_data,
                nonce,
                ..
            } => {
                let _ = event_tx.send(RelayEvent::MessageReceived {
                    encrypted_data,
                    nonce,
                });
            }
            RelayMessage::PeerDisconnected { device_id } => {
                info!("Peer disconnected: {device_id}");
                let _ = event_tx.send(RelayEvent::PeerDisconnected { device_id });
            }
            RelayMessage::HeartbeatAck => {
                debug!("Heartbeat acknowledged");
            }
            RelayMessage::Error { message } => {
                error!("Relay error: {message}");
                let _ = event_tx.send(RelayEvent::Error { message });
            }
            _ => {}
        }
    }

    /// Send a protocol message to the relay server.
    pub async fn send(&self, msg: RelayMessage) -> Result<()> {
        let guard = self.cmd_tx.read().await;
        let tx = guard
            .as_ref()
            .ok_or_else(|| anyhow!("not connected"))?;
        tx.send(msg)
            .map_err(|e| anyhow!("send failed: {e}"))?;
        Ok(())
    }

    /// Create a room on the relay server, optionally with a client-specified room ID.
    pub async fn create_room(&self, device_id: &str, public_key: &str, room_id: Option<&str>) -> Result<()> {
        self.send(RelayMessage::CreateRoom {
            room_id: room_id.map(|s| s.to_string()),
            device_id: device_id.to_string(),
            device_type: "desktop".to_string(),
            public_key: public_key.to_string(),
        })
        .await
    }

    /// Send an E2E encrypted message through the relay.
    pub async fn send_encrypted(
        &self,
        room_id: &str,
        encrypted_data: &str,
        nonce: &str,
    ) -> Result<()> {
        self.send(RelayMessage::Relay {
            room_id: room_id.to_string(),
            encrypted_data: encrypted_data.to_string(),
            nonce: nonce.to_string(),
        })
        .await
    }

    pub async fn disconnect(&self) {
        // Drop the command sender to make the write task exit,
        // which in turn closes the underlying WebSocket.
        *self.cmd_tx.write().await = None;
        *self.state.write().await = ConnectionState::Disconnected;
        info!("Relay client disconnected (cmd channel dropped)");
    }
}
