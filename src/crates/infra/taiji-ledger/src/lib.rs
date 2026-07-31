//! taiji-ledger — 功德簿（审计日志 + Git 语义编排）
//!
//! 提供 Ledger trait（被动接收审计记录）+ InMemoryLedger + GitLedger。
//!
//! # 模块
//!
//! - `ledger.rs` — Ledger trait + InMemoryLedger
//! - `audit.rs` — AuditEntry / AuditFilter / AuditSummary / AuditResult
//! - `git_ops.rs` — GitLedger（commit / reincarnate / cherry-pick / fork_branch / get_graph）
//! - `error.rs` — LedgerError + GitError

pub mod audit;
pub mod error;
pub mod git_ops;
pub mod ledger;
pub mod revert_gate;

pub use audit::{AuditEntry, AuditFilter, AuditResult, AuditSummary};
pub use error::{GitError, LedgerError};
pub use git_ops::GitLedger;
pub use ledger::{InMemoryLedger, Ledger};
pub use revert_gate::{LedgerRevertGate, TreasureSpender};
