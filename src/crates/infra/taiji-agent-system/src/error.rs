//! Agent System 错误类型。

use thiserror::Error;

use taiji_types::error::LvpaError;

/// Agent System 统一的错误类型。
#[derive(Debug, Error)]
pub enum AgentSystemError {
    /// Agent 未找到。
    #[error("agent not found: {0}")]
    AgentNotFound(String),

    /// Agent 重复注册。
    #[error("agent already registered: {0}")]
    AgentAlreadyExists(String),

    /// Agent 状态不合法。
    #[error("invalid state transition for agent '{agent_id}': {reason}")]
    InvalidStateTransition { agent_id: String, reason: String },

    /// 暂未实现。
    #[error("not implemented: {0}")]
    Unimplemented(String),

    /// 身外化身失败。
    #[error("fork failed: {0}")]
    ForkFailed(String),

    /// 转世重生失败。
    #[error("reincarnate failed: {0}")]
    ReincarnateFailed(String),

    /// 序列化错误。
    #[error("serialization error: {0}")]
    Serialization(String),

    /// 内部错误。
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for AgentSystemError {
    fn from(e: serde_json::Error) -> Self {
        AgentSystemError::Serialization(e.to_string())
    }
}

impl From<LvpaError> for AgentSystemError {
    fn from(e: LvpaError) -> Self {
        match e {
            LvpaError::Unimplemented(msg) => AgentSystemError::Unimplemented(msg),
            LvpaError::Config(msg) => AgentSystemError::Internal(msg),
            LvpaError::Serialization(msg) => AgentSystemError::Serialization(msg),
            LvpaError::PermissionDenied(msg) => AgentSystemError::Internal(msg),
            LvpaError::NotFound(msg) => AgentSystemError::AgentNotFound(msg),
            LvpaError::Internal(msg) => AgentSystemError::Internal(msg),
        }
    }
}
