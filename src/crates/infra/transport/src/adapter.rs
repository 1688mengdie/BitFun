//! TransportAdapter trait — 传输适配器核心接口
//!
//! 负责将序列化后的消息推送到 Layer 3（TS 前端 / CLI / TUI）。
//! 不感知事件语义，只关心序列化后的 message。
//!
//! # 设计原则
//! - 单一职责：仅定义 send 方法
//! - 可插拔：不同的适配器实现不同的传输后端
//!
//! # 参考来源
//! - H8: modules/transport/接口设计.md:47-51 — TransportAdapter trait 定义
//! - H8: BitFun adapters/transport/ TransportAdapter trait 模式（模式参考，非 Cargo 依赖）

use crate::TransportMessage;
use async_trait::async_trait;
use std::fmt::Debug;

/// 传输适配器 — 连接层核心接口。
///
/// 负责将序列化后的 TransportMessage 推送到 Layer 3 前端。
/// 不感知事件语义，只关心序列化后的 message。
///
/// # 实现要求
/// - 必须实现 `Send + Sync`，支持跨线程共享
/// - 必须实现 `Debug`，支持日志输出
/// - `send()` 必须异步且返回 `anyhow::Result<()>`
#[async_trait]
pub trait TransportAdapter: Send + Sync + Debug {
    /// 发送 TransportMessage 到前端。
    ///
    /// # 参数
    /// - `msg`: 已序列化的传输消息
    ///
    /// # 错误
    /// 返回 `anyhow::Error`，由调用方决定处理策略（重试/降级/记录）。
    async fn send(&self, msg: TransportMessage) -> anyhow::Result<()>;
}

/// MockTransportAdapter — 测试用的模拟适配器。
///
/// 记录所有发送的消息到内部 Vec，不进行实际传输。
#[derive(Debug, Default)]
pub struct MockTransportAdapter {
    /// 已发送的消息列表
    pub sent: std::sync::Mutex<Vec<TransportMessage>>,
}

impl MockTransportAdapter {
    /// 创建一个新的 MockTransportAdapter。
    pub fn new() -> Self {
        Self::default()
    }

    /// 返回已发送的消息数量。
    pub fn sent_count(&self) -> usize {
        self.sent.lock().unwrap().len()
    }

    /// 清空已发送的消息列表。
    pub fn clear(&self) {
        self.sent.lock().unwrap().clear();
    }

    /// 获取第 index 条已发送消息的引用。
    pub fn get(&self, index: usize) -> Option<TransportMessage> {
        self.sent.lock().unwrap().get(index).cloned()
    }
}

#[async_trait]
impl TransportAdapter for MockTransportAdapter {
    async fn send(&self, msg: TransportMessage) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push(msg);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_mock_adapter_send() {
        let adapter = MockTransportAdapter::new();
        let msg = TransportMessage::new("test:event", json!({"a": 1}));
        adapter.send(msg.clone()).await.unwrap();
        assert_eq!(adapter.sent_count(), 1);
        assert_eq!(adapter.get(0).unwrap(), msg);
    }

    #[tokio::test]
    async fn test_mock_adapter_multiple_sends() {
        let adapter = MockTransportAdapter::new();
        for i in 0..5 {
            let msg = TransportMessage::new("test:event", json!({"index": i}));
            adapter.send(msg).await.unwrap();
        }
        assert_eq!(adapter.sent_count(), 5);
    }

    #[tokio::test]
    async fn test_mock_adapter_clear() {
        let adapter = MockTransportAdapter::new();
        adapter.send(TransportMessage::new("evt", json!({}))).await.unwrap();
        assert_eq!(adapter.sent_count(), 1);
        adapter.clear();
        assert_eq!(adapter.sent_count(), 0);
    }

    #[tokio::test]
    async fn test_trait_object() {
        let adapter: Box<dyn TransportAdapter> = Box::new(MockTransportAdapter::new());
        let msg = TransportMessage::new("trait:test", json!({"trait": true}));
        adapter.send(msg).await.unwrap();
    }
}
