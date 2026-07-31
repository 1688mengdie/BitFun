//! WorkshopError — 工坊系统专用错误类型。

use serde::{Deserialize, Serialize};
use thiserror::Error;

use taiji_types::agent::{AgentId, SpiritRoot};
use taiji_types::workshop_dungeon::WorkshopId;

/// 工坊系统错误。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
pub enum WorkshopError {
    #[error("workshop not found: {0}")]
    WorkshopNotFound(WorkshopId),

    #[error("agent {0} is already a member of workshop {1}")]
    AlreadyMember(AgentId, WorkshopId),

    #[error("agent {0} is not a member of workshop {1}")]
    NotMember(AgentId, WorkshopId),

    #[error("spirit root mismatch: agent has {0:?}, workshop requires {1:?}")]
    SpiritRootMismatch(SpiritRoot, Vec<SpiritRoot>),

    #[error("workshop is full: {0}/{1}")]
    WorkshopFull(u32, u32),

    #[error("workshop is not active")]
    WorkshopNotActive,

    #[error("dag cycle detected: {0:?}")]
    DagCycleDetected(Vec<String>),

    #[error("dag node not found: {0}")]
    DagNodeNotFound(String),

    #[error("prerequisites not met for node '{0}': missing inputs {1:?}")]
    PrerequisitesNotMet(String, Vec<String>),

    #[error("config error: {0}")]
    Config(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("internal error: {0}")]
    Internal(String),
}
