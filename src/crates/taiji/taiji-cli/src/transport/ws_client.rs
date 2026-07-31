//! WebSocket 客户端（剑气传书）
//!
//! 用于 TUI 仪表盘接收实时行情推送。

use futures::SinkExt;
use futures::StreamExt;
use tokio_tungstenite::connect_async;
use tracing::debug;

use super::error::TransportError;

/// WebSocket 消息流
#[allow(dead_code)]
pub(crate) type WsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

/// 连接到 WebSocket 端点
#[allow(dead_code)]
pub(crate) async fn connect(url: &str) -> Result<WsStream, TransportError> {
    debug!("WebSocket connecting to: {}", url);
    let (ws_stream, _) = connect_async(url)
        .await
        .map_err(|e| TransportError::Connection(format!("WebSocket connection failed: {}", e)))?;
    debug!("WebSocket connected");
    Ok(ws_stream)
}

/// 读取下一条 WebSocket 消息（文本）
#[allow(dead_code)]
pub(crate) async fn read_text(stream: &mut WsStream) -> Result<String, TransportError> {
    loop {
        match stream.next().await {
            Some(Ok(msg)) => {
                if let Ok(text) = msg.to_text() {
                    return Ok(text.to_string());
                }
                // Ping/Pong/Close 等控制帧跳过
                continue;
            }
            Some(Err(e)) => {
                return Err(TransportError::Protocol(format!(
                    "WebSocket error: {}",
                    e
                )))
            }
            None => return Err(TransportError::Connection("WebSocket closed".to_string())),
        }
    }
}

/// 发送文本消息到 WebSocket
#[allow(dead_code)]
pub(crate) async fn send_text(
    stream: &mut WsStream,
    text: &str,
) -> Result<(), TransportError> {
    use tokio_tungstenite::tungstenite::Message;
    stream
        .send(Message::Text(text.to_string().into()))
        .await
        .map_err(|e| TransportError::Protocol(format!("WebSocket send error: {}", e)))
}
