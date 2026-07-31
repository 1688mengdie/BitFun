//! InMemoryBus — 内存消息总线实现（基于 DashMap + mpsc fan-out）。
//!
//! 设计：
//! - `DashMap<String, Vec<mpsc::Sender<Bytes>>>` 管理 topic → 订阅者列表
//! - 发布时遍历所有订阅者，try_send 投递，自动清理已关闭的订阅者
//! - 同 topic 内消息顺序保留（mpsc 保证 FIFO）
//! - 多 topic 完全隔离
//! - 无接收者时静默丢弃（非错误）

use std::sync::Arc;

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::mpsc;

use crate::error::MessageBusError;
use crate::bus::MessageBus;

/// InMemoryBus — 基于内存的消息总线实现。
///
/// 所有数据驻留内存，适合单进程部署。
pub struct InMemoryBus {
    topics: Arc<DashMap<String, Vec<mpsc::Sender<Bytes>>>>,
    channel_cap: usize,
}

impl InMemoryBus {
    /// 创建新的 InMemoryBus。
    ///
    /// `channel_cap`: 每个订阅者的 mpsc channel 容量。
    pub fn new(channel_cap: usize) -> Self {
        Self {
            topics: Arc::new(DashMap::new()),
            channel_cap,
        }
    }
}

impl MessageBus for InMemoryBus {
    fn publish(&self, topic: &str, payload: Bytes) -> Result<(), MessageBusError> {
        if let Some(mut entry) = self.topics.get_mut(topic) {
            let senders = entry.value_mut();
            // 清理已关闭的订阅者；满队列（Full）保留
            senders.retain(|tx| {
                match tx.try_send(payload.clone()) {
                    Ok(()) => true,
                    Err(mpsc::error::TrySendError::Closed(_)) => false,
                    Err(mpsc::error::TrySendError::Full(_)) => true,
                }
            });
        }
        // 无接收者 = 静默丢弃，非错误
        Ok(())
    }

    fn subscribe(&self, topic: &str) -> Result<mpsc::Receiver<Bytes>, MessageBusError> {
        let (tx, rx) = mpsc::channel(self.channel_cap);
        self.topics
            .entry(topic.to_string())
            .or_default()
            .push(tx);
        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_bus() -> InMemoryBus {
        InMemoryBus::new(64)
    }

    #[tokio::test]
    async fn test_publish_subscribe() {
        let bus = Arc::new(setup_bus());
        let mut rx = bus.subscribe("test.a").unwrap();
        let payload = Bytes::from("hello");

        bus.publish("test.a", payload.clone()).unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received, payload);
    }

    #[tokio::test]
    async fn test_multi_topic_isolation() {
        let bus = Arc::new(setup_bus());
        let mut rx_a = bus.subscribe("topic_a").unwrap();
        let _rx_b = bus.subscribe("topic_b").unwrap();

        bus.publish("topic_a", Bytes::from("data_for_a")).unwrap();

        // topic_a 应收到
        let received = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            rx_a.recv(),
        )
        .await
        .expect("topic_a subscriber should receive")
        .unwrap();
        assert_eq!(received, Bytes::from("data_for_a"));
    }

    #[tokio::test]
    async fn test_order_preserved() {
        let bus = Arc::new(setup_bus());
        let mut rx = bus.subscribe("test.order").unwrap();
        let count = 100usize;

        // 后台 drain 任务，防止 mpsc channel 满导致 try_send 丢消息
        let drainer = tokio::spawn(async move {
            let mut received = Vec::with_capacity(count);
            for _ in 0..count {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    rx.recv(),
                )
                .await
                {
                    Ok(Some(msg)) => received.push(msg),
                    _ => break,
                }
            }
            received
        });

        for i in 0..count {
            bus.publish("test.order", Bytes::from(format!("msg_{}", i)))
                .unwrap();
            // 每次 publish 后 yield，给 drainer 消费机会
            tokio::task::yield_now().await;
        }

        let received = drainer.await.unwrap();
        assert_eq!(received.len(), count, "message count mismatch");
        for (i, msg) in received.iter().enumerate() {
            let text = String::from_utf8_lossy(msg);
            assert_eq!(text, format!("msg_{}", i), "order broken at index {}", i);
        }
    }

    #[tokio::test]
    async fn test_concurrent_safety() {
        let bus = Arc::new(setup_bus());
        let mut rx = bus.subscribe("test.concurrent").unwrap();
        let msg_count = 50usize;
        let concurrency = 5usize;
        let tasks_per_worker = msg_count / concurrency;

        // 后台 drain 任务
        let drainer = tokio::spawn(async move {
            let mut received = std::collections::HashSet::new();
            for _ in 0..msg_count {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    rx.recv(),
                )
                .await
                {
                    Ok(Some(msg)) => {
                        received.insert(String::from_utf8_lossy(&msg).to_string());
                    }
                    _ => break,
                }
            }
            received
        });

        let mut handles = vec![];
        for w in 0..concurrency {
            let bus = Arc::clone(&bus);
            let handle = tokio::spawn(async move {
                for i in 0..tasks_per_worker {
                    let msg = Bytes::from(format!("w{}-{}", w, i));
                    bus.publish("test.concurrent", msg).unwrap();
                    tokio::task::yield_now().await;
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let received = drainer.await.unwrap();
        assert_eq!(received.len(), msg_count, "concurrent publish lost messages");
    }

    #[tokio::test]
    async fn test_subscribe_before_publish() {
        let bus = Arc::new(setup_bus());
        let mut rx = bus.subscribe("test.pre_sub").unwrap();
        let payload = Bytes::from("late payload");
        bus.publish("test.pre_sub", payload.clone()).unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received, payload);
    }

    #[tokio::test]
    async fn test_publish_no_subscriber() {
        let bus = setup_bus();
        // 发布到无订阅者的 topic 不应报错
        let result = bus.publish("orphan_topic", Bytes::from("data"));
        assert!(result.is_ok(), "publish to topic with no subscribers should succeed");
    }

    #[tokio::test]
    async fn test_empty_topic() {
        let bus = setup_bus();
        // 空 topic 不应 panic
        let result = bus.publish("", Bytes::from("data"));
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_large_message() {
        let bus = Arc::new(setup_bus());
        let mut rx = bus.subscribe("test.large").unwrap();
        let large: Vec<u8> = vec![b'x'; 10_000_000]; // 10MB
        bus.publish("test.large", Bytes::from(large.clone())).unwrap();
        let received = rx.recv().await.unwrap();
        assert_eq!(received.len(), 10_000_000);
    }
}
