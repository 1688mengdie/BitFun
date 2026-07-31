//! EventBus 错误类型。

use thiserror::Error;

/// EventBus 统一结果类型。
pub type EventBusResult<T> = Result<T, EventBusError>;

/// EventBus 错误枚举。
#[derive(Debug, Clone, PartialEq, Error)]
pub enum EventBusError {
    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("subscriber error: {0}")]
    Subscriber(String),

    #[error("topic error: {0}")]
    Topic(String),

    #[error("internal error: {0}")]
    Internal(String),
}
