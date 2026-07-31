//! taiji-tool-bus — LVPA 法宝台（工具注册与执行中心）。
//!
//! 核心能力：
//! - [`ToolRegistryItem`] trait — 工具注册项接口
//! - [`ToolRegistry`] — IndexMap 注册中心
//! - [`TaijiToolDomain`] — 灵根→工具组绑定
//! - [`execute(&dyn Harness)`] — 强制门控的工具执行
//!
//! 参考: modules/tool-bus/接口设计.md — R-4-301 — LVPA 特有实现

mod domain;
mod registry;
mod tool;

pub use domain::TaijiToolDomain;
pub use registry::{MaterializedToolSnapshot, ToolBusError, ToolRegistry, ToolResult};
pub use tool::{ToolExposure, ToolRef, ToolRegistryItem};
