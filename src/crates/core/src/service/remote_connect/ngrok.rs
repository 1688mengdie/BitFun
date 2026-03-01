//! ngrok tunnel mode for Remote Connect.

use anyhow::{anyhow, Result};
use log::info;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

/// Find the ngrok binary, checking common locations beyond just PATH.
fn find_ngrok() -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = vec![
        PathBuf::from("/usr/local/bin/ngrok"),
        PathBuf::from("/opt/homebrew/bin/ngrok"),
        dirs::home_dir()
            .map(|h| h.join("ngrok"))
            .unwrap_or_default(),
        dirs::home_dir()
            .map(|h| h.join(".ngrok/ngrok"))
            .unwrap_or_default(),
        dirs::home_dir()
            .map(|h| h.join("bin/ngrok"))
            .unwrap_or_default(),
        #[cfg(target_os = "windows")]
        {
            let appdata = std::env::var("LOCALAPPDATA").unwrap_or_default();
            PathBuf::from(format!("{appdata}\\ngrok\\ngrok.exe"))
        },
        #[cfg(target_os = "windows")]
        PathBuf::from("C:\\ngrok\\ngrok.exe"),
    ];

    // Try which crate first (uses PATH)
    if let Ok(path) = which::which("ngrok") {
        return Some(path);
    }

    // Check known locations
    for path in candidates {
        if path.exists() && path.is_file() {
            return Some(path);
        }
    }

    None
}

/// Check if ngrok is installed and available.
pub async fn is_ngrok_available() -> bool {
    find_ngrok().is_some()
}

/// Start an ngrok HTTP tunnel and return the public URL.
pub async fn start_ngrok_tunnel(local_port: u16) -> Result<NgrokTunnel> {
    let ngrok_path = find_ngrok()
        .ok_or_else(|| anyhow!("ngrok is not installed. Please install it from https://ngrok.com/download"))?;

    info!("Using ngrok at: {}", ngrok_path.display());

    let child = Command::new(&ngrok_path)
        .args(["http", &local_port.to_string(), "--log", "stdout", "--log-format", "json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("failed to start ngrok: {e}"))?;

    let pid = child.id().unwrap_or(0);
    info!("ngrok process started, pid={pid}");

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let public_url = query_ngrok_api().await?;
    info!("ngrok tunnel established: {public_url}");

    Ok(NgrokTunnel {
        public_url,
        local_port,
        process: Some(child),
    })
}

async fn query_ngrok_api() -> Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("http://127.0.0.1:4040/api/tunnels")
        .send()
        .await
        .map_err(|e| anyhow!("ngrok API query failed: {e}"))?;

    let body: serde_json::Value = resp.json().await?;
    let tunnels = body["tunnels"]
        .as_array()
        .ok_or_else(|| anyhow!("no tunnels in ngrok API response"))?;

    for tunnel in tunnels {
        if let Some(url) = tunnel["public_url"].as_str() {
            if url.starts_with("https://") {
                return Ok(url.to_string());
            }
        }
    }

    tunnels
        .first()
        .and_then(|t| t["public_url"].as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("no public URL found in ngrok tunnels"))
}

pub struct NgrokTunnel {
    pub public_url: String,
    pub local_port: u16,
    process: Option<tokio::process::Child>,
}

impl NgrokTunnel {
    pub fn ws_url(&self) -> String {
        self.public_url
            .replace("https://", "wss://")
            .replace("http://", "ws://")
    }

    pub async fn stop(&mut self) {
        if let Some(ref mut child) = self.process {
            let _ = child.kill().await;
            info!("ngrok tunnel stopped");
        }
        self.process = None;
    }
}

impl Drop for NgrokTunnel {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.process {
            let _ = child.start_kill();
        }
    }
}
