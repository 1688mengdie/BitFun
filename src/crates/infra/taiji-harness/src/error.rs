//! Harness 错误类型
//!
//! 来源: modules/harness/接口设计.md §6 — HarnessError

use thiserror::Error;

/// 护山大阵运行时错误
#[derive(Error, Debug, Clone, PartialEq)]
pub enum HarnessError {
    /// 权限不足
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// 资源配额超限
    #[error("quota exceeded: {0}")]
    QuotaExceeded(String),

    /// 权限数据未加载
    #[error("permission data not loaded: {0}")]
    DataNotLoaded(String),

    /// 内部错误
    #[error("internal: {0}")]
    Internal(String),
}
