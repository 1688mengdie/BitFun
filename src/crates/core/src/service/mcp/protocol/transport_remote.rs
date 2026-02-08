//! Remote MCP transport (HTTP/SSE)
//!
//! Handles communication with remote MCP servers over HTTP and SSE.

use super::{MCPMessage, MCPNotification, MCPRequest, MCPResponse};
use crate::util::errors::{BitFunError, BitFunResult};
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use log::{debug, error, info, warn};
use reqwest::Client;
use serde_json::Value;
use std::error::Error;
use tokio::sync::mpsc;

/// Remote MCP transport.
pub struct RemoteMCPTransport {
    url: String,
    client: Client,
    session_id: tokio::sync::RwLock<Option<String>>,
    auth_token: Option<String>,
}

impl RemoteMCPTransport {
    /// Creates a new remote transport instance.
    pub fn new(url: String, auth_token: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .danger_accept_invalid_certs(false) // Production should validate certificates.
            .use_rustls_tls()
            .build()
            .unwrap_or_else(|e| {
                warn!("Failed to create HTTP client, using default config: {}", e);
                Client::new()
            });

        if auth_token.is_some() {
            debug!("Authorization token configured for remote transport");
        }

        Self {
            url,
            client,
            session_id: tokio::sync::RwLock::new(None),
            auth_token,
        }
    }

    /// Sends a JSON-RPC request to the remote server.
    pub async fn send_request(&self, request: &MCPRequest) -> BitFunResult<Value> {
        debug!("Sending request to {}: method={}", self.url, request.method);

        let mut request_builder = self
            .client
            .post(&self.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .header("User-Agent", "BitFun-MCP-Client/1.0");

        if let Some(ref token) = self.auth_token {
            request_builder = request_builder.header("Authorization", token);
        }

        let response = request_builder.json(request).send().await.map_err(|e| {
            let error_detail = if e.is_timeout() {
                "Request timed out, please check network connection"
            } else if e.is_connect() {
                "Unable to connect to server, please check URL and network"
            } else if e.is_request() {
                "Request build failed"
            } else if e.is_body() {
                "Request body serialization failed"
            } else {
                "Unknown error"
            };

            error!("HTTP request failed: {} (type: {})", e, error_detail);
            if let Some(url_err) = e.url() {
                error!("URL: {}", url_err);
            }
            if let Some(source) = e.source() {
                error!("Cause: {}", source);
            }

            BitFunError::MCPError(format!("HTTP request failed ({}): {}", error_detail, e))
        })?;

        let status = response.status();

        if let Some(session_id) = response
            .headers()
            .get("x-session-id")
            .or_else(|| response.headers().get("session-id"))
            .or_else(|| response.headers().get("sessionid"))
        {
            if let Ok(session_id_str) = session_id.to_str() {
                debug!("Received sessionId: {}", session_id_str);
                let mut sid = self.session_id.write().await;
                *sid = Some(session_id_str.to_string());
            }
        }

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!("Server returned error status {}: {}", status, error_text);
            return Err(BitFunError::MCPError(format!(
                "Server error {}: {}",
                status, error_text
            )));
        }

        let response_text = response.text().await.map_err(|e| {
            error!("Failed to read response body: {}", e);
            BitFunError::MCPError(format!("Failed to read response body: {}", e))
        })?;

        let json_response: Value =
            if response_text.starts_with("event:") || response_text.starts_with("data:") {
                Self::parse_sse_response(&response_text)?
            } else {
                serde_json::from_str(&response_text).map_err(|e| {
                    error!(
                        "Failed to parse JSON response: {} (content: {})",
                        e, response_text
                    );
                    BitFunError::MCPError(format!("Failed to parse response: {}", e))
                })?
            };

        Ok(json_response)
    }

    /// Returns the current session ID.
    pub async fn get_session_id(&self) -> Option<String> {
        self.session_id.read().await.clone()
    }

    /// Returns the auth token.
    pub fn get_auth_token(&self) -> Option<String> {
        self.auth_token.clone()
    }

    /// Parses an SSE-formatted response and extracts JSON from the `data` field.
    fn parse_sse_response(sse_text: &str) -> BitFunResult<Value> {
        // SSE format example:
        // event: message
        // id: xxx
        // data: {"jsonrpc":"2.0",...}

        for line in sse_text.lines() {
            let line = line.trim();
            if line.starts_with("data:") {
                let json_str = line.strip_prefix("data:").unwrap_or("").trim();
                if !json_str.is_empty() {
                    return serde_json::from_str(json_str).map_err(|e| {
                        error!(
                            "Failed to parse SSE data as JSON: {} (data: {})",
                            e, json_str
                        );
                        BitFunError::MCPError(format!("Failed to parse SSE data as JSON: {}", e))
                    });
                }
            }
        }

        error!("No data field found in SSE response");
        Err(BitFunError::MCPError(
            "No data field found in SSE response".to_string(),
        ))
    }

    /// Starts the SSE receive loop.
    pub fn start_sse_loop(
        url: String,
        session_id: Option<String>,
        auth_token: Option<String>,
        message_tx: mpsc::UnboundedSender<MCPMessage>,
    ) {
        tokio::spawn(async move {
            if let Err(e) = Self::sse_loop(url, session_id, auth_token, message_tx).await {
                error!("SSE connection failed: {}", e);
            }
        });
    }

    /// SSE receive loop.
    async fn sse_loop(
        url: String,
        session_id: Option<String>,
        auth_token: Option<String>,
        message_tx: mpsc::UnboundedSender<MCPMessage>,
    ) -> BitFunResult<()> {
        let sse_url = if url.ends_with("/mcp") {
            url.replace("/mcp", "/sse")
        } else {
            url.clone()
        };

        info!("Connecting to SSE stream: {}", sse_url);
        if let Some(ref sid) = session_id {
            debug!("Using sessionId: {}", sid);
        }

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300)) // 5-minute timeout
            .build()
            .unwrap_or_else(|_| Client::new());

        let mut request_builder = client
            .get(&sse_url)
            .header("Accept", "text/event-stream, application/json")
            .header("User-Agent", "BitFun-MCP-Client/1.0");

        if let Some(ref token) = auth_token {
            request_builder = request_builder.header("Authorization", token);
        }

        if let Some(sid) = session_id {
            request_builder = request_builder
                .header("X-Session-Id", &sid)
                .header("Session-Id", &sid)
                .query(&[("sessionId", &sid), ("session_id", &sid)]);
        }

        let response = request_builder.send().await.map_err(|e| {
            error!("Failed to connect to SSE stream: {}", e);
            BitFunError::MCPError(format!("Failed to connect to SSE stream: {}", e))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!("Server returned error status {}: {}", status, error_text);
            return Err(BitFunError::MCPError(format!(
                "SSE connection failed: {}",
                status
            )));
        }

        info!("SSE connection established");

        let mut stream = response.bytes_stream().eventsource();

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    let data = event.data;
                    if data.trim().is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<Value>(&data) {
                        Ok(json_value) => {
                            if let Some(message) = Self::parse_message(&json_value) {
                                if let Err(e) = message_tx.send(message) {
                                    error!("Failed to send message to handler: {}", e);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            warn!(
                                "Failed to parse JSON from SSE event: {} (data: {})",
                                e, data
                            );
                        }
                    }
                }
                Err(e) => {
                    error!("SSE event error: {}", e);
                    break;
                }
            }
        }

        warn!("SSE stream closed");
        Ok(())
    }

    /// Parses JSON into an MCP message.
    fn parse_message(value: &Value) -> Option<MCPMessage> {
        if value.get("id").is_some()
            && (value.get("result").is_some() || value.get("error").is_some())
        {
            if let Ok(response) = serde_json::from_value::<MCPResponse>(value.clone()) {
                return Some(MCPMessage::Response(response));
            }
        }

        if value.get("method").is_some() && value.get("id").is_none() {
            if let Ok(notification) = serde_json::from_value::<MCPNotification>(value.clone()) {
                return Some(MCPMessage::Notification(notification));
            }
        }

        if value.get("method").is_some() && value.get("id").is_some() {
            if let Ok(request) = serde_json::from_value::<MCPRequest>(value.clone()) {
                return Some(MCPMessage::Request(request));
            }
        }

        warn!("Unknown message format: {:?}", value);
        None
    }
}
