//! HTTP JSON-RPC 客户端
//!
//! 通过 HTTP 发送 JSON-RPC 2.0 请求到后端服务。

use async_trait::async_trait;
use serde_json::Value;
use tracing::debug;

use super::error::TransportError;

/// HTTP JSON-RPC 客户端
pub(crate) struct HttpTransportClient {
    base_url: String,
    client: reqwest::Client,
    timeout: std::time::Duration,
}

impl HttpTransportClient {
    pub(crate) fn new(server_url: &str) -> Self {
        Self {
            base_url: server_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            timeout: std::time::Duration::from_secs(30),
        }
    }
}

#[async_trait]
impl super::TransportClient for HttpTransportClient {
    async fn request(&self, method: &str, params: Value) -> Result<Value, TransportError> {
        let url = format!("{}/api/rpc", self.base_url);
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        debug!("JSON-RPC request: {} {}", method, params);

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| TransportError::Connection(format!("HTTP request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(TransportError::Connection(format!(
                "HTTP {}",
                resp.status()
            )));
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| TransportError::Protocol(format!("Invalid JSON response: {}", e)))?;

        if let Some(err) = json.get("error") {
            let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Err(TransportError::Protocol(format!(
                "JSON-RPC error [{}]: {}",
                code, msg
            )));
        }

        json.get("result")
            .cloned()
            .ok_or_else(|| TransportError::Protocol("Missing 'result' in response".to_string()))
    }

    async fn subscribe(
        &self,
        channel: &str,
    ) -> Result<super::ws_client::WsStream, TransportError> {
        let ws_url = self
            .base_url
            .replace("http://", "ws://")
            .replace("https://", "wss://");
        let url = format!("{}/ws/{}", ws_url, channel);
        super::ws_client::connect(&url).await
    }

    async fn health(&self) -> Result<String, TransportError> {
        let url = format!("{}/health", self.base_url);
        let resp = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| TransportError::Connection(format!("Health check failed: {}", e)))?;
        Ok(format!("OK (HTTP {})", resp.status()))
    }
}
