#![doc = "taiji-infra-config — 天书阁（配置管理系统）"]

//! LVPA 基础设施层：三平面配置管理。
//!
//! 优先级链 `env > file > defaults`，支持运行时热更新和变更事件广播。
//! 提供 `ConfigManager` trait + 三平面加载器 + 已知键校验。
//!
//! # 设计原则
//!
//! - **三平面独立**：env / file / defaults 各有独立加载器，按优先级合并
//! - **类型安全**：`get<T: DeserializeOwned>` 泛型读取
//! - **变更追踪**：所有配置变更通过 `ConfigChangeEvent` 广播
//!
//! # R-ID 映射
//!
//! 对应 R-1-105，子任务分解见 `量价时空/Phase-1-RID矩阵.md`。

pub mod error;
pub mod event;
pub mod keys;
pub mod loader;
pub mod manager;
pub mod plane;
pub mod provider;

pub use error::{ConfigError, ConfigResult};
pub use event::ConfigChangeEvent;
pub use keys::KNOWN_CONFIG_KEYS;
pub use loader::LvpaConfigManager;
pub use manager::ConfigManager;
pub use plane::ConfigPlane;
pub use provider::{ConfigProvider, ConfigProviderRegistry};
