//! cache（速递符）— 错误类型
//!
//! 来源: thiserror 模式

use thiserror::Error;

/// 缓存操作错误
#[derive(Debug, Error)]
pub enum CacheError {
    /// 键不存在
    #[error("键不存在: {0}")]
    KeyNotFound(String),

    /// 序列化错误
    #[error("序列化错误: {0}")]
    Serialization(String),

    /// 反序列化错误
    #[error("反序列化错误: {0}")]
    Deserialization(String),

    /// 内部错误
    #[error("内部错误: {0}")]
    Internal(String),
}
