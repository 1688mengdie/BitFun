//! 天书阁 — 配置变更事件。
//!
//! 配置变更通过 `tokio::sync::broadcast` 发布，订阅者接收后自行处理。
//!
//! 设计参考：LVPA 自定变更事件，通过 tokio::sync::broadcast 发布。

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::plane::ConfigPlane;

/// 配置变更事件——平面合并后或 set()/reset() 操作时触发。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeEvent {
    /// 变更的配置点路径（如 `"monitor.level"`）。
    pub path: String,
    /// 变更前的值（初次加载或新增路径时为 `None`）。
    pub old_value: Option<Value>,
    /// 变更后的值。
    pub new_value: Value,
    /// 此变更的来源平面。
    pub plane: ConfigPlane,
}

impl ConfigChangeEvent {
    /// 创建配置变更事件。
    pub fn new(path: impl Into<String>, old_value: Option<Value>, new_value: Value, plane: ConfigPlane) -> Self {
        Self {
            path: path.into(),
            old_value,
            new_value,
            plane,
        }
    }
}

/// 配置变更事件的广播通道容量。
pub const CONFIG_BROADCAST_CAPACITY: usize = 64;
