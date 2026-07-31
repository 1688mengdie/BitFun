//! Agent 生命周期状态机。
//!
//! 完整状态流转：
//!
//! ```text
//! 注册(register) → Idle ◄── sleep() ──► Sleeping
//!                    │                       │
//!                 reply()                 wake()
//!                    │                       │
//!                    ▼                       │
//!                Running ◄─── wait() ────────┘
//!                    │
//!              ┌─────┴─────┐
//!              │           │
//!              ▼           ▼
//!          (完成回调)   (错误回调)
//!              │           │
//!              └─────┬─────┘
//!                    │
//!                    ▼
//!                  Idle (重置)
//!
//! destroy() 在任何状态可用 → Destroyed（终态）
//! ```

use taiji_types::agent::{AgentId, AgentStatus};

use crate::error::AgentSystemError;

/// 生命周期状态转换结果。
#[derive(Debug, Clone, PartialEq)]
pub struct StateTransition {
    pub agent_id: AgentId,
    pub from: AgentStatus,
    pub to: AgentStatus,
}

/// 生命周期状态机 — 校验状态转换合法性。
pub struct Lifecycle;

impl Lifecycle {
    /// 检查状态转换是否合法。
    /// 返回 Ok(()) 表示允许，Err 表示非法转换。
    pub fn validate(from: AgentStatus, to: AgentStatus) -> Result<StateTransition, AgentSystemError> {
        let allowed = match (from, to) {
            // Idle → Running：收到回复请求
            (AgentStatus::Idle, AgentStatus::Running) => true,
            // Idle → Sleeping：手动休眠
            (AgentStatus::Idle, AgentStatus::Sleeping) => true,
            // Idle → Destroyed：销毁
            (AgentStatus::Idle, AgentStatus::Destroyed) => true,
            // Running → Idle：回复完成
            (AgentStatus::Running, AgentStatus::Idle) => true,
            // Running → Sleeping：等待中进入休眠
            (AgentStatus::Running, AgentStatus::Sleeping) => true,
            // Running → Destroyed：运行中销毁
            (AgentStatus::Running, AgentStatus::Destroyed) => true,
            // Sleeping → Idle：唤醒
            (AgentStatus::Sleeping, AgentStatus::Idle) => true,
            // Sleeping → Running：唤醒后直接工作
            (AgentStatus::Sleeping, AgentStatus::Running) => true,
            // Sleeping → Destroyed：休眠中销毁
            (AgentStatus::Sleeping, AgentStatus::Destroyed) => true,
            // Destroyed 是终态：不可再转换
            (AgentStatus::Destroyed, _) => false,
            // 其他组合：非法
            _ => false,
        };

        if allowed {
            Ok(StateTransition { agent_id: AgentId::default(), from, to })
        } else {
            Err(AgentSystemError::InvalidStateTransition {
                agent_id: "unknown".into(),
                reason: format!("cannot transition from {:?} to {:?}", from, to),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idle_to_running() {
        assert!(Lifecycle::validate(AgentStatus::Idle, AgentStatus::Running).is_ok());
    }

    #[test]
    fn test_running_to_idle() {
        assert!(Lifecycle::validate(AgentStatus::Running, AgentStatus::Idle).is_ok());
    }

    #[test]
    fn test_idle_to_sleeping() {
        assert!(Lifecycle::validate(AgentStatus::Idle, AgentStatus::Sleeping).is_ok());
    }

    #[test]
    fn test_sleeping_to_idle() {
        assert!(Lifecycle::validate(AgentStatus::Sleeping, AgentStatus::Idle).is_ok());
    }

    #[test]
    fn test_sleeping_to_running() {
        assert!(Lifecycle::validate(AgentStatus::Sleeping, AgentStatus::Running).is_ok());
    }

    #[test]
    fn test_any_to_destroyed() {
        for from in &[AgentStatus::Idle, AgentStatus::Running, AgentStatus::Sleeping] {
            assert!(Lifecycle::validate(*from, AgentStatus::Destroyed).is_ok());
        }
    }

    #[test]
    fn test_destroyed_is_terminal() {
        for to in &[AgentStatus::Idle, AgentStatus::Running, AgentStatus::Sleeping] {
            assert!(Lifecycle::validate(AgentStatus::Destroyed, *to).is_err());
        }
    }

    #[test]
    fn test_invalid_transitions() {
        assert!(Lifecycle::validate(AgentStatus::Sleeping, AgentStatus::Sleeping).is_err());
    }
}
