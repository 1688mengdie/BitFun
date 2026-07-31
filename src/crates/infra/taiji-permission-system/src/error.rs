//! PermissionSystemError — 权限系统管理面错误类型。

use serde::{Deserialize, Serialize};
use thiserror::Error;

use taiji_types::agent::AgentId;

/// 权限系统管理面错误。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
pub enum PermissionSystemError {
    #[error("agent not found: {0}")]
    AgentNotFound(AgentId),

    #[error("title not found: {0}")]
    TitleNotFound(String),

    #[error("permission level {0:?} not assignable to title {1}")]
    LevelNotAssignable(String, String),

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("storage error: {0}")]
    Storage(String),
}
