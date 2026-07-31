//! LVPA 错误类型体系。
//!
//! 所有 LVPA 基础设施模块共享的错误枚举。
//! 使用 thiserror 派生，支持 Display + Error trait。

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// LVPA 统一结果类型。
pub type LvpaResult<T> = Result<T, LvpaError>;

/// 错误分类枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorKind {
    /// 配置类错误。
    Config,
    /// 权限类错误。
    Permission,
    /// 序列化类错误。
    Serialization,
    /// 未找到类错误。
    NotFound,
    /// 内部类错误。
    Internal,
    /// 暂未实现。
    Unimplemented,
}

/// LVPA 错误枚举。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
pub enum LvpaError {
    /// 模块尚未实现。
    #[error("not implemented: {0}")]
    Unimplemented(String),

    /// 配置错误。
    #[error("configuration error: {0}")]
    Config(String),

    /// 消息序列化/反序列化错误。
    #[error("serialization error: {0}")]
    Serialization(String),

    /// 权限拒绝。
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// 实体未找到。
    #[error("not found: {0}")]
    NotFound(String),

    /// 内部错误。
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for LvpaError {
    fn from(e: serde_json::Error) -> Self {
        LvpaError::Serialization(e.to_string())
    }
}
