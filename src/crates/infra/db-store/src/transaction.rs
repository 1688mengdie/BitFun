//! db-store（灵脉）事务接口
//!
//! 来源: modules/db-store/接口设计.md:51-59 — TransactionBackend trait

use crate::error::DbError;
use serde_json::Value;

/// 事务后端 trait
///
/// 由 `StorageBackend::transaction` 创建，提供事务内的 SQL 执行接口。
/// 委托 sqlx::Transaction 实现。
///
/// 来源: modules/db-store/接口设计.md:51-59 — TransactionBackend
pub trait TransactionBackend: Send + Sync {
    /// 执行 SQL 语句（无返回行）
    fn execute(&mut self, sql: &str, params: &[&dyn sqlx::Encode<'_, sqlx::Sqlite>]) -> Result<u64, DbError>;

    /// 执行查询（返回行）
    fn query(&mut self, sql: &str, params: &[&dyn sqlx::Encode<'_, sqlx::Sqlite>]) -> Result<Vec<Value>, DbError>;

    /// 提交事务
    fn commit(&mut self) -> Result<(), DbError>;

    /// 回滚事务
    fn rollback(&mut self) -> Result<(), DbError>;
}
