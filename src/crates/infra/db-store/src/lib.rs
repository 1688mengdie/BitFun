#![doc = "taiji-infra-db-store — 灵脉（数据库存储抽象层）"]

//! LVPA 基础设施层：结构化数据持久化。
//!
//! 提供 `StorageBackend` trait（CRUD + 批量 + 分页 + 事务）和 `SQLiteBackend` 实现。
//! 包含 `SharedBarBuffer` — L1 实时计算专用的无锁环形缓冲。
//!
//! # 设计原则
//!
//! - **统一 trait**：`StorageBackend` 抽象所有存储操作，SQLite 单后端
//! - **L1 零阻塞**：`SharedBarBuffer` 基于 arc_swap 无锁结构，零 IO
//! - **版本化迁移**：内置 migration 框架 + checksum 校验
//! - **严格约束**：DDL 使用 STRICT、WITHOUT ROWID、CHECK 约束
//!
//! # 模块结构
//!
//! | 模块 | 说明 |
//! |------|------|
//! | `backend` | StorageBackend trait + SQLiteBackend 实现 |
//! | `transaction` | TransactionBackend trait |
//! | `config` | DbConfig + BufferConfig |
//! | `query` | QueryFilter + PaginatedResult |
//! | `models` | Agent / TaskEntity / RawBar / Freq / SymbolInfo |
//! | `error` | DbError + BufferError |
//! | `migration` | 版本化迁移框架 |
//! | `buffer` | SharedBarBuffer（L1 环缓冲） |
//!
//! # R-ID 映射
//!
//! 对应 R-1-104，子任务分解见 `量价时空/Phase-1-RID矩阵.md`。

pub mod backend;
pub mod buffer;
pub mod config;
pub mod error;
pub mod migration;
pub mod models;
pub mod query;
pub mod transaction;

// 核心类型重新导出
pub use backend::{SQLiteBackend, StorageBackend};
pub use buffer::SharedBarBuffer;
pub use config::{BufferConfig, DbConfig};
pub use error::{BufferError, DbError};
pub use models::agent::Agent;
pub use models::bar::{BarUpdate, BufferStats, Freq, RawBar};
pub use models::symbol::SymbolInfo;
pub use models::task::TaskEntity;
pub use query::{PaginatedResult, QueryFilter};
pub use transaction::TransactionBackend;
