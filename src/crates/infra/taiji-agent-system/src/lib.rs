//! taiji-agent-system — LVPA Agent 运行时核心。
//!
//! # 模块
//!
//! - [`agent`] — AgentTrait 定义（9 方法 + 默认实现）
//! - [`lifecycle`] — Agent 生命周期状态机
//! - [`manager`] — AgentManager（注册/查找/销毁/fork/reincarnate）
//! - [`event_bus`] — 事件总线接口（EventBus trait + MockEventBus）
//! - [`event`] — AgentEvent 事件类型
//! - [`error`] — AgentSystemError 错误类型

pub mod agent;
pub mod error;
pub mod event;
pub mod event_bus;
pub mod lifecycle;
pub mod manager;

pub use agent::AgentTrait;
pub use error::AgentSystemError;
pub use event::AgentEvent;
pub use event_bus::{AgentStatusChangeEvent, EventBus, MockEventBus, NoopEventBus};
pub use lifecycle::{Lifecycle, StateTransition};
pub use manager::AgentManager;
