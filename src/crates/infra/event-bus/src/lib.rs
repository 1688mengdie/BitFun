#![doc = "taiji-infra-event-bus — 任务堂（事件总线 + KPI 调度）"]

//! LVPA 基础设施层：语义事件总线。
//!
//! 在 message-bus 的字节传输之上，提供结构化事件定义、路由分发和 KPI 评分调度。
//!
//! # 设计原则
//!
//! - **泛型化**：`EventBus<M: MessageBus>` 可接受任意消息总线实现
//! - **KPI 驱动**：评分调度器按 success_rate / review_pass_rate / rework_rate / kpi_bonus 派单
//! - **L1 隔离**：L1 topic 事件不经过 event-bus 异步路径
//!
//! # R-ID 映射
//!
//! 对应 R-1-102，子任务分解见 `量价时空/Phase-1-RID矩阵.md`。

pub mod bus;
pub mod codec;
pub mod envelope;
pub mod error;
pub mod event;
pub mod router;
pub mod scheduler;
pub mod tool_event;

pub use bus::{EventBus, EventBusConfig};
pub use codec::{CodecConfig, EventCodec, SerializationFormat};
pub use envelope::{TaijiEventEnvelope, TaijiEventPriority};
pub use error::{EventBusError, EventBusResult};
pub use event::TaijiEvent;
pub use router::{EventRouter, EventSubscriber};
pub use scheduler::{KpiScheduler, TaskResult};
pub use tool_event::{ToolEventData, ToolEventIdentity};
