//! 天书阁 — ConfigManager trait。
//!
//! 定义统一的配置管理接口：加载、读取、写入、重置、校验、变更订阅。
//!
//! 设计参考：gbrain `src/core/config.ts:665-895` 三平面合并模式 + BitFun
//! `crates/assembly/core/src/service/config/manager.rs:219-708` ConfigManager 操作模式，
//! Rust trait 从零定义，非 Cargo 依赖。

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::sync::broadcast;

use crate::error::ConfigResult;
use crate::event::ConfigChangeEvent;

/// 配置管理器 trait。
///
/// 提供三平面配置（env > file > defaults）的统一操作接口。
#[async_trait]
pub trait ConfigManager: Send + Sync {
    /// 加载并合并全部配置平面。
    ///
    /// 按优先级 env > file > defaults 依次合并。
    async fn load(&mut self) -> ConfigResult<()>;

    /// 按点路径获取配置值。
    ///
    /// `path` 示例：`"monitor.level"`、`"database_url"`。
    async fn get<T: DeserializeOwned + Send>(&self, path: &str) -> ConfigResult<T>;

    /// 按点路径设置配置值。
    ///
    /// 设置后自动广播 ConfigChangeEvent，保存配置文件。
    async fn set<T: Serialize + Send>(&mut self, path: &str, value: T) -> ConfigResult<()>;

    /// 重置指定路径（或全部）配置到默认值。
    async fn reset(&mut self, path: Option<&str>) -> ConfigResult<()>;

    /// 校验配置合法性。
    ///
    /// 返回警告列表（空 = 完全合法）。
    async fn validate(&self) -> ConfigResult<Vec<String>>;

    /// 订阅配置变更事件。
    fn subscribe(&self) -> broadcast::Receiver<ConfigChangeEvent>;
}
