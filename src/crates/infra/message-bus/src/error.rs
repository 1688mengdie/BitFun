//! 消息总线错误类型 — MessageBusError。
//!
//! 参考: modules/message-bus/接口设计.md §6 错误类型（v2.4）

use thiserror::Error;

/// 消息总线错误枚举。
#[derive(Debug, Clone, PartialEq, Error)]
pub enum MessageBusError {
    /// topic 不存在。
    #[error("topic not found: {0}")]
    TopicNotFound(String),

    /// 发布失败。
    #[error("publish failed: {0}")]
    PublishFailed(String),

    /// 订阅失败。
    #[error("subscribe failed: {0}")]
    SubscribeFailed(String),

    /// 内部错误。
    #[error("internal error: {0}")]
    Internal(String),
}
