//! Remote Connect service module.
//!
//! Provides phone-to-desktop remote connection capabilities with E2E encryption.
//! Supports multiple connection methods: LAN, ngrok, relay server, and bots.

pub mod bot;
pub mod device;
pub mod embedded_relay;
pub mod encryption;
pub mod lan;
pub mod ngrok;
pub mod pairing;
pub mod qr_generator;
pub mod relay_client;
pub mod remote_server;

pub use device::DeviceIdentity;
pub use encryption::{decrypt_from_base64, encrypt_to_base64, KeyPair};
pub use pairing::{PairingProtocol, PairingState};
pub use qr_generator::QrGenerator;
pub use relay_client::RelayClient;
pub use remote_server::RemoteServer;

use anyhow::Result;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;


/// Supported connection methods.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionMethod {
    Lan,
    Ngrok,
    BitfunServer,
    CustomServer { url: String },
    BotFeishu,
    BotTelegram,
}

/// Configuration for Remote Connect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConnectConfig {
    pub lan_port: u16,
    pub bitfun_server_url: String,
    pub web_app_url: String,
    pub custom_server_url: Option<String>,
    pub bot_feishu: Option<bot::BotConfig>,
    pub bot_telegram: Option<bot::BotConfig>,
    /// Path to mobile-web build output directory. When provided, the embedded
    /// relay (LAN/Ngrok) will serve these static files so that the mobile
    /// browser can load the web app from the local relay instead of the remote
    /// BitFun server.
    pub mobile_web_dir: Option<String>,
}

impl Default for RemoteConnectConfig {
    fn default() -> Self {
        Self {
            lan_port: 9700,
            bitfun_server_url: "http://116.204.120.240/relay".to_string(),
            web_app_url: "http://116.204.120.240/relay".to_string(),
            custom_server_url: None,
            bot_feishu: None,
            bot_telegram: None,
            mobile_web_dir: None,
        }
    }
}

/// Result of starting a remote connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionResult {
    pub method: ConnectionMethod,
    pub qr_data: Option<String>,
    pub qr_svg: Option<String>,
    pub qr_url: Option<String>,
    pub bot_pairing_code: Option<String>,
    pub bot_link: Option<String>,
    pub pairing_state: PairingState,
}

/// Unified Remote Connect service that orchestrates all connection methods.
pub struct RemoteConnectService {
    config: RemoteConnectConfig,
    device_identity: DeviceIdentity,
    pairing: Arc<RwLock<PairingProtocol>>,
    relay_client: Arc<RwLock<Option<RelayClient>>>,
    remote_server: Arc<RwLock<Option<RemoteServer>>>,
    active_method: Arc<RwLock<Option<ConnectionMethod>>>,
    ngrok_tunnel: Arc<RwLock<Option<ngrok::NgrokTunnel>>>,
    embedded_relay: Arc<RwLock<Option<embedded_relay::EmbeddedRelayHandle>>>,
}

impl RemoteConnectService {
    pub fn new(config: RemoteConnectConfig) -> Result<Self> {
        let device_identity = DeviceIdentity::from_current_machine()?;
        let pairing = PairingProtocol::new(device_identity.clone());

        Ok(Self {
            config,
            device_identity,
            pairing: Arc::new(RwLock::new(pairing)),
            relay_client: Arc::new(RwLock::new(None)),
            remote_server: Arc::new(RwLock::new(None)),
            active_method: Arc::new(RwLock::new(None)),
            ngrok_tunnel: Arc::new(RwLock::new(None)),
            embedded_relay: Arc::new(RwLock::new(None)),
        })
    }

    pub fn device_identity(&self) -> &DeviceIdentity {
        &self.device_identity
    }

    /// All connection methods are always available in the UI (ngrok shows warning if missing).
    pub async fn available_methods(&self) -> Vec<ConnectionMethod> {
        vec![
            ConnectionMethod::Lan,
            ConnectionMethod::Ngrok,
            ConnectionMethod::BitfunServer,
            ConnectionMethod::CustomServer {
                url: self.config.custom_server_url.clone().unwrap_or_default(),
            },
            ConnectionMethod::BotFeishu,
            ConnectionMethod::BotTelegram,
        ]
    }

    /// Start a remote connection with the given method.
    pub async fn start(&self, method: ConnectionMethod) -> Result<ConnectionResult> {
        info!("Starting remote connect: {method:?}");

        let static_dir = self.config.mobile_web_dir.as_deref();

        let relay_url = match &method {
            ConnectionMethod::Lan => {
                let handle =
                    embedded_relay::start_embedded_relay(self.config.lan_port, static_dir).await?;
                *self.embedded_relay.write().await = Some(handle);
                lan::build_lan_relay_url(self.config.lan_port)?
            }
            ConnectionMethod::Ngrok => {
                let handle =
                    embedded_relay::start_embedded_relay(self.config.lan_port, static_dir).await?;
                *self.embedded_relay.write().await = Some(handle);

                let tunnel = ngrok::start_ngrok_tunnel(self.config.lan_port).await?;
                let url = tunnel.public_url.clone();
                *self.ngrok_tunnel.write().await = Some(tunnel);
                url
            }
            ConnectionMethod::BitfunServer => self.config.bitfun_server_url.clone(),
            ConnectionMethod::CustomServer { url } => url.clone(),
            ConnectionMethod::BotFeishu | ConnectionMethod::BotTelegram => {
                return self.start_bot_connection(&method).await;
            }
        };

        let mut pairing = self.pairing.write().await;
        pairing.reset().await;
        let qr_payload = pairing.initiate(&relay_url).await?;

        // Desktop relay client WebSocket URL:
        // For LAN/Ngrok the relay runs locally — connect to localhost directly
        // instead of going through the public URL (ngrok would block with 401).
        let ws_url = match &method {
            ConnectionMethod::Lan | ConnectionMethod::Ngrok => {
                format!("ws://127.0.0.1:{}/ws", self.config.lan_port)
            }
            _ => {
                format!(
                    "{}/ws",
                    relay_url
                        .replace("https://", "wss://")
                        .replace("http://", "ws://")
                )
            }
        };

        let (client, mut event_rx) = RelayClient::new();
        client.connect(&ws_url).await?;
        client
            .create_room(
                &self.device_identity.device_id,
                &qr_payload.public_key,
                Some(&qr_payload.room_id),
            )
            .await?;

        // For BitfunServer / CustomServer: upload mobile-web files to the relay
        // server so the mobile browser loads the same version as the desktop.
        // The QR URL then points to /r/{room_id}/ on the relay server.
        let web_app_url: String = match &method {
            ConnectionMethod::Lan | ConnectionMethod::Ngrok => relay_url.clone(),
            ConnectionMethod::BitfunServer | ConnectionMethod::CustomServer { .. } => {
                if let Some(web_dir) = static_dir {
                    match upload_mobile_web(&relay_url, &qr_payload.room_id, web_dir).await {
                        Ok(()) => {
                            let url = format!("{}/r/{}", relay_url.trim_end_matches('/'), qr_payload.room_id);
                            info!("Uploaded mobile-web to relay: {url}");
                            url
                        }
                        Err(e) => {
                            error!("Failed to upload mobile-web to relay: {e}; falling back to server-hosted version");
                            self.config.web_app_url.clone()
                        }
                    }
                } else {
                    info!("No mobile_web_dir configured; using server-hosted mobile web");
                    self.config.web_app_url.clone()
                }
            }
            _ => self.config.web_app_url.clone(),
        };

        let qr_url = QrGenerator::build_url(&qr_payload, &web_app_url);
        let qr_svg = QrGenerator::generate_svg_from_url(&qr_url)?;
        let qr_data = QrGenerator::generate_png_base64_from_url(&qr_url)?;

        *self.active_method.write().await = Some(method.clone());
        *self.relay_client.write().await = Some(client);

        let pairing_arc = self.pairing.clone();
        let relay_arc = self.relay_client.clone();
        let server_arc = self.remote_server.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    relay_client::RelayEvent::PeerJoined {
                        public_key,
                        device_id,
                    } => {
                        info!("Peer joined: {device_id}");
                        let mut p = pairing_arc.write().await;
                        match p.on_peer_joined(&public_key).await {
                            Ok(challenge) => {
                                if let Some(secret) = p.shared_secret() {
                                    let challenge_json =
                                        serde_json::to_string(&challenge).unwrap_or_default();
                                    if let Ok((enc, nonce)) =
                                        encryption::encrypt_to_base64(secret, &challenge_json)
                                    {
                                        if let Some(ref client) = *relay_arc.read().await {
                                            if let Some(room) = p.room_id() {
                                                let _ = client
                                                    .send_encrypted(room, &enc, &nonce)
                                                    .await;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Pairing error on peer_joined: {e}");
                            }
                        }
                    }
                    relay_client::RelayEvent::MessageReceived {
                        encrypted_data,
                        nonce,
                    } => {
                        let is_paired = server_arc.read().await.is_some();

                        if is_paired {
                            let server_guard = server_arc.read().await;
                            if let Some(ref server) = *server_guard {
                                match server.decrypt_command(&encrypted_data, &nonce) {
                                    Ok((cmd, request_id)) => {
                                        info!("Remote command: {cmd:?}");

                                        let response = server.dispatch(&cmd).await;
                                        match server.encrypt_response(&response, request_id.as_deref()) {
                                            Ok((enc, resp_nonce)) => {
                                                if let Some(ref client) = *relay_arc.read().await {
                                                    let p = pairing_arc.read().await;
                                                    if let Some(room) = p.room_id() {
                                                        let _ = client.send_encrypted(room, &enc, &resp_nonce).await;
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error!("Failed to encrypt response: {e}");
                                            }
                                        }
                                    }
                                    Err(e) => error!("Failed to decrypt command: {e}"),
                                }
                            }
                        } else {
                            // Not yet paired — try to verify pairing response
                            let p = pairing_arc.read().await;
                            if let Some(secret) = p.shared_secret() {
                                if let Ok(json) =
                                    encryption::decrypt_from_base64(secret, &encrypted_data, &nonce)
                                {
                                    if let Ok(response) =
                                        serde_json::from_str::<pairing::PairingResponse>(&json)
                                    {
                                        drop(p);
                                        let mut pw = pairing_arc.write().await;
                                        match pw.verify_response(&response).await {
                                            Ok(true) => {
                                                info!("Pairing verified successfully");
                                                if let Some(s) = pw.shared_secret() {
                                                    let (stream_tx, mut stream_rx) = tokio::sync::mpsc::unbounded_channel::<remote_server::EncryptedPayload>();
                                                    
                                                    let relay_for_stream = relay_arc.clone();
                                                    let pairing_for_stream = pairing_arc.clone();
                                                    tokio::spawn(async move {
                                                        while let Some((enc, nonce)) = stream_rx.recv().await {
                                                            if let Some(ref client) = *relay_for_stream.read().await {
                                                                let p = pairing_for_stream.read().await;
                                                                if let Some(room) = p.room_id() {
                                                                    let _ = client.send_encrypted(room, &enc, &nonce).await;
                                                                }
                                                            }
                                                        }
                                                    });

                                                    let server = RemoteServer::new(*s, stream_tx);

                                                    // Push initial sync (workspace + sessions) immediately after pairing
                                                    let initial_sync = server.generate_initial_sync().await;
                                                    if let Ok((enc, nonce)) = server.encrypt_response(&initial_sync, None) {
                                                        if let Some(ref client) = *relay_arc.read().await {
                                                            if let Some(room) = pw.room_id() {
                                                                info!("Sending initial sync to mobile after pairing");
                                                                let _ = client.send_encrypted(room, &enc, &nonce).await;
                                                            }
                                                        }
                                                    }

                                                    *server_arc.write().await = Some(server);
                                                }
                                            }
                                            Ok(false) => {
                                                error!("Pairing verification failed");
                                            }
                                            Err(e) => {
                                                error!("Pairing verification error: {e}");
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    relay_client::RelayEvent::PeerDisconnected { device_id } => {
                        info!("Peer disconnected: {device_id}");
                        pairing_arc.write().await.disconnect().await;
                        *server_arc.write().await = None;
                    }
                    relay_client::RelayEvent::Reconnected => {
                        // Relay reconnected and room was recreated.
                        // Reset pairing so mobile can re-pair with fresh keys.
                        info!("Relay reconnected, resetting pairing state");
                        pairing_arc.write().await.disconnect().await;
                        *server_arc.write().await = None;
                    }
                    relay_client::RelayEvent::Disconnected => {
                        info!("Relay disconnected");
                    }
                    relay_client::RelayEvent::Error { message } => {
                        error!("Relay error: {message}");
                        if message.contains("Room not found") {
                            info!("Room expired, disconnecting");
                            pairing_arc.write().await.disconnect().await;
                            *server_arc.write().await = None;
                        }
                    }
                    _ => {}
                }
            }
        });

        let state = pairing.state().await;
        Ok(ConnectionResult {
            method,
            qr_data: Some(qr_data),
            qr_svg: Some(qr_svg),
            qr_url: Some(qr_url),
            bot_pairing_code: None,
            bot_link: None,
            pairing_state: state,
        })
    }

    async fn start_bot_connection(&self, method: &ConnectionMethod) -> Result<ConnectionResult> {
        let pairing_code = PairingProtocol::generate_bot_pairing_code();

        let bot_link = match method {
            ConnectionMethod::BotFeishu => {
                "https://open.feishu.cn/open-apis/bot".to_string()
            }
            ConnectionMethod::BotTelegram => {
                "https://t.me/your_bitfun_bot".to_string()
            }
            _ => String::new(),
        };

        *self.active_method.write().await = Some(method.clone());

        Ok(ConnectionResult {
            method: method.clone(),
            qr_data: None,
            qr_svg: None,
            qr_url: None,
            bot_pairing_code: Some(pairing_code),
            bot_link: Some(bot_link),
            pairing_state: PairingState::WaitingForScan,
        })
    }

    pub async fn pairing_state(&self) -> PairingState {
        self.pairing.read().await.state().await
    }

    pub async fn stop(&self) {
        if let Some(ref client) = *self.relay_client.read().await {
            client.disconnect().await;
        }
        *self.relay_client.write().await = None;
        *self.remote_server.write().await = None;
        *self.active_method.write().await = None;

        if let Some(ref mut tunnel) = *self.ngrok_tunnel.write().await {
            tunnel.stop().await;
        }
        *self.ngrok_tunnel.write().await = None;

        if let Some(ref mut relay) = *self.embedded_relay.write().await {
            relay.stop();
        }
        *self.embedded_relay.write().await = None;

        self.pairing.write().await.reset().await;
        info!("Remote connect stopped");
    }

    pub async fn is_connected(&self) -> bool {
        self.pairing.read().await.state().await == PairingState::Connected
    }

    pub async fn active_method(&self) -> Option<ConnectionMethod> {
        self.active_method.read().await.clone()
    }

    pub async fn peer_device_name(&self) -> Option<String> {
        self.pairing.read().await.peer_device_name().map(String::from)
    }
}

/// Read all files from `web_dir` and upload them (base64-encoded) to
/// the relay server at `POST {relay_url}/api/rooms/{room_id}/upload-web`.
async fn upload_mobile_web(relay_url: &str, room_id: &str, web_dir: &str) -> Result<()> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use std::collections::HashMap;

    let base = std::path::Path::new(web_dir);
    if !base.join("index.html").exists() {
        return Err(anyhow::anyhow!(
            "mobile-web dir missing index.html: {}",
            web_dir
        ));
    }

    let mut files: HashMap<String, String> = HashMap::new();
    collect_files(base, base, &mut files, &B64)?;

    let url = format!(
        "{}/api/rooms/{}/upload-web",
        relay_url.trim_end_matches('/'),
        room_id
    );

    info!(
        "Uploading {} mobile-web files ({} bytes base64) to {url}",
        files.len(),
        files.values().map(|v| v.len()).sum::<usize>()
    );

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "files": files }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("upload mobile-web: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "upload mobile-web failed: HTTP {status} — {body}"
        ));
    }

    Ok(())
}

fn collect_files(
    base: &std::path::Path,
    dir: &std::path::Path,
    out: &mut std::collections::HashMap<String, String>,
    b64: &base64::engine::GeneralPurpose,
) -> Result<()> {
    use base64::Engine;

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(base, &path, out, b64)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let content = std::fs::read(&path)?;
            out.insert(rel, b64.encode(&content));
        }
    }
    Ok(())
}
