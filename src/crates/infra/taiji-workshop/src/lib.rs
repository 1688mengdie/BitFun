//! taiji-workshop — LVPA 工坊系统（装备堂·固定工作流 DAG）
//!
//! 4 条固定工作流：天机坊（代码/开发）、金算坊（交易/量化）、
//! 丹青坊（美术/设计）、留影坊（视频/内容）。
//!
//! 核心概念：工坊 = 固定组织，长期工作流。Agent 按本命魂卡属性加入。
//! 多公会支持：一个 Agent 可加入多个工坊。
//!
//! # 模块
//!
//! - [`config`] — 工坊配置（TOML 驱动）
//! - [`workshop`] — Workshop 结构体 + 成员管理 + 资格校验
//! - [`dag`] — DAG 拓扑排序 + 执行节点查询
//! - [`output`] — 工坊产出记录
//! - [`manager`] — WorkshopManager trait + DefaultWorkshopManager
//! - [`error`] — WorkshopError
//!
//! 参考: 架构总纲 §7.1 — 工坊与副本
//!       Phase-工坊系统-类型契约.md — R-WD-101~103

mod config;
mod dag;
mod error;
mod manager;
mod output;
mod workshop;

pub use config::{load_workshop_configs, WorkshopConfig, DEFAULT_WORKSHOP_TOML};
pub use dag::WorkshopDag;
pub use error::WorkshopError;
pub use manager::{DefaultWorkshopManager, WorkshopManager};
pub use output::WorkshopOutputStore;
pub use workshop::Workshop;
