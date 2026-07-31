//! 状态管理 — 快照 + 恢复
//!
//! 包含快照管理（StateManager / SnapshotManager）和崩溃恢复（StateRecovery）。
//! 参考: 量价时空/Phase-2-派发提示词.md:527 — R-2-204 — StateStore + 快照恢复

pub mod recovery;
pub mod snapshot;
