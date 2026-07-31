//! WsTransportAdapter — 基于 broadcast 的 WebSocket 传输适配器
//!
//! 使用 `tokio::sync::broadcast` 实现内存内发布/订阅。
//! 发送者调用 `send()` 广播消息，所有订阅者通过 `Receiver` 接收。
//!
//! # 设计原则
//! - 基于标准 crate `tokio::sync::broadcast` 实现，非 tokio-tungstenite 全功能 WS 服务端
//! - 非阻塞：send 不等待消费者处理
//! - 背压：broadcast channel 满时旧消息被丢弃（lagging 订阅者）
//!
//! # 参考来源
//! - H8: modules/transport/接口设计.md:77-94 — WsTransportAdapter 定义
//! - H8: tokio::sync::broadcast 标准 crate API

use crate::{TransportAdapter, TransportMessage};
use anyhow::Context;
use async_trait::async_trait;
use std::fmt;
use tokio::sync::broadcast;

/// 基于 broadcast channel 的传输适配器。
///
/// 使用 tokio::sync::broadcast 实现内存内消息广播。
/// 适用于 Web 前端、CLI 等进程内消费者场景。
///
/// # 默认 channel 容量
/// 默认为 256 条消息。订阅者消费速度慢于发送速度时，
/// 旧消息会被自动丢弃以防止内存无限增长。
pub struct WsTransportAdapter {
    /// broadcast 发送端
    tx: broadcast::Sender<TransportMessage>,
    /// channel 容量
    capacity: usize,
}

impl fmt::Debug for WsTransportAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WsTransportAdapter")
            .field("capacity", &self.capacity)
            .field("receiver_count", &self.tx.receiver_count())
            .finish()
    }
}

impl WsTransportAdapter {
    /// 创建一个新的 WsTransportAdapter。
    ///
    /// 返回 (适配器, 广播接收器)。调用者可将接收器传递给 WebSocket 处理循环。
    ///
    /// # 默认容量
    /// channel 默认容量为 256 条消息。
    ///
    /// # 示例
    /// ```
    /// use taiji_infra_transport::WsTransportAdapter;
    /// use taiji_infra_transport::TransportAdapter;
    /// use serde_json::json;
    ///
    /// let (adapter, mut rx) = WsTransportAdapter::new();
    /// let msg = taiji_infra_transport::TransportMessage::new("test", json!({}));
    ///
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     adapter.send(msg).await.unwrap();
    ///     let received = rx.recv().await.unwrap();
    ///     assert_eq!(received.event_name, "test");
    /// });
    /// ```
    pub fn new() -> (Self, broadcast::Receiver<TransportMessage>) {
        let capacity = 256;
        let (tx, rx) = broadcast::channel(capacity);
        (
            Self {
                tx,
                capacity,
            },
            rx,
        )
    }

    /// 创建指定容量的 WsTransportAdapter。
    pub fn with_capacity(capacity: usize) -> (Self, broadcast::Receiver<TransportMessage>) {
        let (tx, rx) = broadcast::channel(capacity);
        (
            Self { tx, capacity },
            rx,
        )
    }

    /// 返回当前订阅者数量。
    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }

    /// 返回 channel 容量。
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// 创建一个新的广播订阅者。
    ///
    /// 返回一个新的 `broadcast::Receiver`，可独立接收所有通过此适配器发送的消息。
    pub fn subscribe(&self) -> broadcast::Receiver<TransportMessage> {
        self.tx.subscribe()
    }
}

#[async_trait]
impl TransportAdapter for WsTransportAdapter {
    async fn send(&self, msg: TransportMessage) -> anyhow::Result<()> {
        self.tx
            .send(msg)
            .map_err(|_| anyhow::anyhow!("WsTransportAdapter: broadcast channel closed"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn test_send_receive() {
        let (adapter, mut rx) = WsTransportAdapter::new();
        let msg = TransportMessage::new("test:event", json!({"seq": 1}));

        adapter.send(msg.clone()).await.unwrap();
        let received = rx.recv().await.unwrap();
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn test_multiple_receivers() {
        let (adapter, mut rx1) = WsTransportAdapter::new();
        let mut rx2 = adapter.tx.subscribe();

        let msg = TransportMessage::new("broadcast", json!({"to": "all"}));
        adapter.send(msg.clone()).await.unwrap();

        assert_eq!(rx1.recv().await.unwrap(), msg);
        assert_eq!(rx2.recv().await.unwrap(), msg);
    }

    #[tokio::test]
    async fn test_send_order_preserved() {
        let (adapter, mut rx) = WsTransportAdapter::new();
        let count = 10;

        for i in 0..count {
            let msg = TransportMessage::new("ordered", json!({"index": i}));
            adapter.send(msg).await.unwrap();
        }

        for i in 0..count {
            let received = rx.recv().await.unwrap();
            assert_eq!(received.payload, json!({"index": i}));
        }
    }

    #[tokio::test]
    async fn test_lagging_receiver_dropped() {
        // 小容量 channel，验证滞后订阅者消息被丢弃
        let (adapter, mut rx) = WsTransportAdapter::with_capacity(4);
        let capacity = 4;

        // 发送超过容量的消息
        for i in 0..(capacity + 2) {
            let msg = TransportMessage::new("flood", json!({"index": i}));
            adapter.send(msg).await.unwrap();
        }

        // 滞后接收者应收到 RecvError::Lagged
        let result = rx.recv().await;
        assert!(result.is_err());
        match result {
            Err(broadcast::error::RecvError::Lagged(n)) => {
                assert!(n >= 2);
            }
            _ => panic!("Expected Lagged error"),
        }
    }

    #[tokio::test]
    async fn test_adapter_debug() {
        let (adapter, _rx) = WsTransportAdapter::new();
        let debug_str = format!("{:?}", adapter);
        assert!(debug_str.contains("WsTransportAdapter"));
        assert!(debug_str.contains("capacity"));
    }

    #[tokio::test]
    async fn test_trait_object_compatible() {
        let (adapter, _rx) = WsTransportAdapter::new();
        let boxed: Box<dyn TransportAdapter> = Box::new(adapter);
        let msg = TransportMessage::new("trait:test", json!({}));
        boxed.send(msg).await.unwrap();
    }

    #[tokio::test]
    async fn test_with_capacity() {
        let (adapter, _rx) = WsTransportAdapter::with_capacity(16);
        assert_eq!(adapter.capacity(), 16);
    }

    #[tokio::test]
    async fn test_receiver_count() {
        let (adapter, _rx1) = WsTransportAdapter::new();
        let _rx2 = adapter.tx.subscribe();
        assert_eq!(adapter.receiver_count(), 2);
    }
}
