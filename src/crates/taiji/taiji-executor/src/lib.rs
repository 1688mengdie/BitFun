//! Taiji executor — signal queue, risk check, batch assembly, order management, and position tracking.
//!
//! # Architecture
//!
//! ```text
//! Signal → [SignalQueue] → [RiskChecker] → [BatchBuilder] → [OrderManager] → Bridge
//! ```
//!
//! ## Modules
//!
//! - [`signal`]: Executor-level signal types (ExecSignal, ExecAction, SignalPriority)
//! - [`queue`]: Priority+FIFO signal queue
//! - [`risk`]: Risk checker trait and pass-through implementation
//! - [`batch`]: Batch order assembly (same-instrument merge)
//! - [`executor`]: Top-level orchestrator
//! - [`bridge`]: Execution bridge trait (CTP / paper / mock)
//! - [`order_mgr`]: Order lifecycle state machine
//! - [`position`]: Position tracker
//!   参考: 量价时空/Phase-2-派发提示词.md:843 — R-2-504 — taiji-executor 执行器

pub mod batch;
pub mod bridge;
pub mod cloud_switch;
pub mod executor;
pub mod order_mgr;
pub mod position;
pub mod queue;
pub mod risk;
pub mod signal;
pub mod types;

pub use batch::BatchBuilder;
pub use bridge::ExecutionBridge;
pub use cloud_switch::{
    AutoSwitcher, CloudConditionalBridge, CloudConditionalOrder, CloudConditionalResponse,
    CloudConditionalStatus, ConditionalOrderType, ConditionalStatus, DegradationConfig,
    DefaultCloudBridge, ExecutionMode, LatencyMonitor, LatencySnapshot, order_to_conditional,
};
pub use executor::Executor;
pub use order_mgr::{OrderManager, OrderState};
pub use position::PositionTracker;
pub use queue::SignalQueue;
pub use risk::{PassThroughRiskChecker, RiskChecker, RiskDecision, RiskCheckRequest};
pub use signal::{ExecAction, ExecSignal, SignalPriority};
pub use types::*;
