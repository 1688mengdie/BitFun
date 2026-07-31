//! file-store（储物阁）— 错误类型
//!
//! 来源: thiserror 模式

use thiserror::Error;

/// 文件存储错误
#[derive(Debug, Error)]
pub enum FileStoreError {
    /// 文件未找到
    #[error("文件未找到: {0}")]
    NotFound(String),

    /// 文件已存在
    #[error("文件已存在: {0}")]
    AlreadyExists(String),

    /// 路径非法
    #[error("路径非法: {0}")]
    InvalidPath(String),

    /// 权限不足
    #[error("权限不足: {0}")]
    PermissionDenied(String),

    /// IO 错误
    #[error("IO 错误: {0}")]
    Io(String),

    /// S3 错误
    #[error("S3 错误: {0}")]
    S3(String),

    /// 存储空间不足
    #[error("存储空间不足: {0}")]
    StorageFull(String),
}
