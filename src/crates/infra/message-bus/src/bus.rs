//! MessageBus trait — 零语义字节传输层核心接口。
//!
//! 参考: modules/message-bus/接口设计.md §1（v2.4）

use crate::error::MessageBusError;
use bytes::Bytes;
use tokio::sync::mpsc;

/// 消息总线 trait — 零语义类型层。
///
/// - `topic`: 路由主题（如 `"layer1.tick"`、`"agent.credit.update"`）
/// - `payload`: 不透明字节。发布者/订阅者自行协商序列化格式。
///
/// # 一致性保证
/// - 至少一次送达（at-least-once delivery）
/// - 同 topic 内消息顺序保留（per-topic ordering）
///
/// # 设计约束
/// - 不实现认证/加密/权限检查 — 这些由上层（gateway/harness）负责
/// - 不实现序列化 — 由 event-bus 的 EventCodec 或发布者自行处理
pub trait MessageBus: Send + Sync {
    /// 发布字节到指定 topic。
    fn publish(&self, topic: &str, payload: Bytes) -> Result<(), MessageBusError>;

    /// 订阅 topic，接收字节流。
    fn subscribe(&self, topic: &str) -> Result<mpsc::Receiver<Bytes>, MessageBusError>;
}
