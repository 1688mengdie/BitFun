//! 传输层 — 千里传音/剑气传书
//!
//! CLI 版 transport 客户端。通过 JSON-RPC 2.0 和 WebSocket 与后端通信。
//! Layer 3 消费不参与 — CLI 不直接调 L1，不订阅事件总线。

pub(crate) mod client;
pub(crate) mod error;
pub(crate) mod ws_client;

use async_trait::async_trait;
use serde_json::Value;

/// Transport 客户端接口
#[async_trait]
#[allow(dead_code)]
pub(crate) trait TransportClient: Send + Sync {
    /// JSON-RPC 请求（阻塞等待响应）
    async fn request(&self, method: &str, params: Value) -> Result<Value, error::TransportError>;

    /// WebSocket 订阅（持续接收推送）
    async fn subscribe(&self, channel: &str) -> Result<ws_client::WsStream, error::TransportError>;

    /// 健康检查
    async fn health(&self) -> Result<String, error::TransportError>;
}

/// 创建默认 Transport 客户端（HTTP + WebSocket）
pub(crate) fn create_client(server_url: &str) -> impl TransportClient {
    client::HttpTransportClient::new(server_url)
}
