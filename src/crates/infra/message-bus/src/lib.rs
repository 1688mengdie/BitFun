#![doc = "taiji-infra-message-bus — 传音符（异步消息总线）"]

//! LVPA 基础设施层：零语义字节传输层。
//!
//! 提供 `MessageBus` trait 抽象（publish/subscribe）和 `InMemoryBus` 实现，
//! 基于 tokio broadcast + DashMap。是所有跨模块异步通信的底层通道。
//!
//! # 设计原则
//!
//! - **零语义**：不感知消息内容，仅负责字节级别可靠投递
//! - **topic 隔离**：不同 topic 不同 channel，互不干扰
//! - **L1 旁路**：`RawMessage + RawMessageConsumer` 为 Layer 1 实时计算提供零拷贝直通路径
//!
//! # R-ID 映射
//!
//! 对应 R-1-101，子任务分解见 `量价时空/Phase-1-RID矩阵.md`。

pub mod bus;
pub mod error;
pub mod in_memory;
pub mod raw;

pub use bus::MessageBus;
pub use error::MessageBusError;
pub use in_memory::InMemoryBus;
pub use raw::{RawMessage, RawMessageConsumer};
