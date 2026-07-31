//! 接引台 — 认证错误类型。

use thiserror::Error;

/// 接引台认证错误。
#[derive(Debug, Error)]
pub enum GatewayError {
    /// 认证失败（API Key 不匹配 / JWT 无效 / Nostr 签名错误）。
    #[error("认证失败: {0}")]
    AuthFailed(String),

    /// Session 未找到。
    #[error("会话未找到: {0}")]
    SessionNotFound(String),

    /// Session 已过期。
    #[error("会话已过期: {0}")]
    SessionExpired(String),

    /// 协议错误。
    #[error("协议错误: {0}")]
    Protocol(String),

    /// IO 错误。
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// 序列化错误。
    #[error("序列化错误: {0}")]
    Json(#[from] serde_json::Error),
}

/// Gateway Result 别名。
pub type GatewayResult<T> = Result<T, GatewayError>;
