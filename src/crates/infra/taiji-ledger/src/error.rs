//! Ledger 错误类型
//!
//! 来源: modules/ledger/接口设计.md §4 — LedgerError
//! 来源: modules/ledger/接口设计.md:86-101 — GitError

use taiji_types::agent::AgentId;
use taiji_types::economy::{CurrencyAmount, TreasureItem};
use thiserror::Error;

/// Git 操作错误
#[derive(Error, Debug, Clone, PartialEq)]
pub enum GitError {
    #[error("repository not found: {0}")]
    RepositoryNotFound(String),
    #[error("git command failed: {0}")]
    CommandFailed(String),
    #[error("branch not found: {0}")]
    BranchNotFound(String),
    #[error("merge conflict: {0}")]
    MergeConflict(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("git2 error: {0}")]
    Git2(String),
}

impl From<git2::Error> for GitError {
    fn from(e: git2::Error) -> Self {
        GitError::Git2(e.message().to_string())
    }
}

/// Ledger 错误
#[derive(Error, Debug, Clone, PartialEq)]
pub enum LedgerError {
    #[error("git error: {0}")]
    Git(String),
    #[error("agent not found: {0}")]
    AgentNotFound(AgentId),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("internal: {0}")]
    Internal(String),

    /// 天材地宝不足 — 转世重生/夺舍消耗不足。
    #[error("insufficient treasure: agent {agent_id} needs {required} but missing {item:?}")]
    InsufficientTreasure {
        agent_id: AgentId,
        required: CurrencyAmount,
        item: TreasureItem,
    },
}

impl From<GitError> for LedgerError {
    fn from(e: GitError) -> Self {
        LedgerError::Git(e.to_string())
    }
}
