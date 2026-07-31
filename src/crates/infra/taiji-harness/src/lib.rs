//! taiji-harness — LVPA 护山大阵（运行时权限门控）。
//!
//! 提供 DefaultHarness 实现 + PermissionDataSource trait。
//!
//! 参考: modules/harness/接口设计.md — R-4-401 — LVPA 特有实现

pub mod harness;
pub mod permission;
pub mod error;

pub use harness::DefaultHarness;
pub use permission::PermissionDataSource;
pub use error::HarnessError;

