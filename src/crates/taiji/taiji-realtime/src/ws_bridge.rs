//! WsBridge — axum WebSocket 服务器，将 TickData 推送到所有连接的客户端。
//!
//! 架构：
//!   crossbeam Receiver → (dedicated thread) → tokio broadcast → axum WS handlers
//!
//! 每个连接的 WebSocket 客户端收到 JSON 格式的 TickData。

use std::net::SocketAddr;
use std::thread;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use crossbeam::channel::Receiver;
use futures_util::{SinkExt, StreamExt};
use taiji_engine::types::tick::TickData;
use tokio::sync::broadcast;

/// WebSocket 桥接——从 crossbeam channel 读取 TickData，广播到所有 WS 客户端。
pub struct WsBridge {
    port: u16,
    receiver: Option<Receiver<TickData>>,
}

impl WsBridge {
    /// 创建 WsBridge。
    ///
    /// `receiver` 来自 `TickChannel::take_receiver()`。
    pub async fn new(port: u16, receiver: Receiver<TickData>) -> Self {
        Self {
            port,
            receiver: Some(receiver),
        }
    }

    /// 启动 axum WebSocket 服务器。
    ///
    /// 内部 spawn 一个线程将 crossbeam 消息桥接到 tokio broadcast，
    /// 然后启动 axum server 监听指定端口。
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let receiver = self.receiver.take().ok_or("WsBridge already started")?;

        // tokio broadcast: 容量 256，用于向所有 WS 客户端转发
        let (broadcast_tx, _) = broadcast::channel::<String>(256);

        // 专用线程：crossbeam → tokio broadcast
        let tx_clone = broadcast_tx.clone();
        thread::spawn(move || {
            while let Ok(tick) = receiver.recv() {
                let json = match serde_json::to_string(&tick) {
                    Ok(j) => j,
                    Err(e) => {
                        tracing::error!("Failed to serialize tick: {e}");
                        continue;
                    }
                };
                // 忽略无订阅者的错误
                let _ = tx_clone.send(json);
            }
        });

        let app = Router::new()
            .route("/ws", get(ws_handler))
            .with_state(broadcast_tx);

        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        tracing::info!("WsBridge listening on ws://{}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

/// WebSocket 升级处理——每个客户端订阅 broadcast。
async fn ws_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(tx): axum::extract::State<broadcast::Sender<String>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, tx))
}

/// 处理单个 WebSocket 连接——从 broadcast 读取并写入 WS。
async fn handle_socket(socket: WebSocket, tx: broadcast::Sender<String>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let mut rx = tx.subscribe();

    // 忽略客户端发来的消息（只推送）
    let mut recv_task = tokio::spawn(async move { while ws_receiver.next().await.is_some() {} });

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if ws_sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    tokio::select! {
        _ = &mut recv_task => {}
        _ = send_task => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl WsBridge {
        /// 测试用：创建 receiver 已被消费的 WsBridge。
        #[cfg(test)]
        fn new_without_receiver(port: u16) -> Self {
            Self {
                port,
                receiver: None,
            }
        }
    }

    /// 验证 WsBridge 可构造，receiver 存在。
    #[tokio::test]
    async fn ws_bridge_construct() {
        let (_tx, rx) = crossbeam::channel::bounded::<TickData>(4);
        let bridge = WsBridge::new(9876, rx).await;
        assert_eq!(bridge.port, 9876);
        assert!(bridge.receiver.is_some());
    }

    /// 验证 receiver 已被消费时 start 返回错误。
    #[tokio::test]
    async fn ws_bridge_double_start_error() {
        let mut bridge = WsBridge::new_without_receiver(12347);
        let result = bridge.start().await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "WsBridge already started");
    }

    /// 验证 start 可后台启动（不被 abort panic）。
    #[tokio::test]
    async fn ws_bridge_start_and_abort() {
        let (_tx, rx) = crossbeam::channel::bounded::<TickData>(4);
        let mut bridge = WsBridge::new(12348, rx).await;

        let handle = tokio::spawn(async move { bridge.start().await });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        handle.abort();
        // 不 panic 即通过
    }
}
