#![doc = "taiji-infra-monitor — 天眼阵（监控与可观测性）"]

//! LVPA 基础设施层：全系统监控/日志/告警。
//!
//! 提供 `Monitor` trait（counter/gauge/histogram/health/alert），
//! 内置 P0 指标集（module_health/rps/error_rate/latency/token），
//! 告警规则引擎支持阈值/变化率/超时检测。
//!
//! # 设计原则
//!
//! - **分级指标**：P0-P4 四级，L1 路径只采集 P0
//! - **模块健康**：`ModuleHandle` 自动注册/注销，drop 时清理
//! - **告警去重**：同规则重复触发有最小间隔控制
//!
//! # R-ID 映射
//!
//! 对应 R-1-106，子任务分解见 `量价时空/Phase-1-RID矩阵.md`。

pub mod alert;
pub mod error;
pub mod health;
pub mod logging;
pub mod metrics;
pub mod monitor;

pub use alert::{AlertChannel, AlertCondition, AlertEvent, AlertRule, AlertSeverity};
pub use error::{MonitorError, MonitorResult};
pub use health::{ModuleHandle, ModuleHealth, ModuleHealthReport, SystemHealthReport};
pub use metrics::{MetricDef, MetricKind, BUILTIN_METRICS};
pub use monitor::{LvpaMonitor, Monitor};
