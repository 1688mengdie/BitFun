//! RawMessage + RawMessageConsumer — L1 零拷贝旁路。
//!
//! L1 Compute 线程不经过 async publish/subscribe 路径，
//! 通过预注册的 SPSC ring buffer 直接消费行情事件。
//!
//! 参考: modules/message-bus/接口设计.md §2（v2.4）
//! 参考: 架构总纲 §1.1（L1 零阻塞: RawMessage 旁路不经过 async 路径）

use bytes::Bytes;

/// L1 零拷贝消息 — 不经过 async message-bus 路径。
///
/// 由 L1 IO 线程写入 SPSC ring buffer，L1 Compute 线程 lock-free 读取。
/// 零拷贝、零分配、零 async。
#[derive(Debug, Clone)]
pub struct RawMessage {
    /// 消息 topic（如 `"tick.bar"`、`"tick.depth"`）。
    pub topic: &'static str,
    /// 消息负载（预分配的固定大小缓冲区）。
    pub data: Bytes,
}

/// L1 消费接口 — 在 message-bus 初始化时注册。
///
/// 实现者在 `consume` 方法中处理 RawMessage，禁止任何阻塞操作。
pub trait RawMessageConsumer: Send + 'static {
    /// 消费 RawMessage（在 L1 Compute 线程调用，禁止任何阻塞操作）。
    fn consume(&self, msg: RawMessage);
}
