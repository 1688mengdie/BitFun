//! 天书阁 — 配置系统错误类型。
//!
//! 设计参考：标准 Rust thiserror 模式。
//! 所有 ConfigManager 操作返回此错误。

use thiserror::Error;

/// 配置系统错误。
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// 配置加载失败。
    #[error("配置加载失败: {0}")]
    LoadFailed(String),

    /// 配置保存失败。
    #[error("配置保存失败: {0}")]
    SaveFailed(String),

    /// 路径无效（不存在的点路径）。
    #[error("无效配置路径: {0}")]
    InvalidPath(String),

    /// 类型不匹配（`get<T>` 时目标类型与存储类型不符）。
    #[error("类型不匹配: {0}")]
    TypeMismatch(String),

    /// 校验失败（配置值和已知键/约束不匹配）。
    #[error("配置校验失败: {0}")]
    ValidationFailed(String),

    /// 序列化/反序列化错误。
    #[error("序列化错误: {0}")]
    Serialization(String),

    /// IO 错误（文件读写等）。
    #[error("IO 错误: {0}")]
    Io(String),

    /// 内部错误。
    #[error("内部错误: {0}")]
    Internal(String),
}

/// 配置系统 Result 别名。
pub type ConfigResult<T> = Result<T, ConfigError>;
