//! db-store（灵脉）数据库迁移框架
//!
//! 版本化迁移 + checksum 校验。
//! 来源: modules/db-store/接口设计.md:474-481 — schema_migrations 表

use crate::error::DbError;
use sha2::{Digest, Sha256};
use sqlx::{Pool, Sqlite};

/// 迁移版本
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    /// 版本号（升序正整数）
    pub version: i64,
    /// 描述
    pub description: &'static str,
    /// 正向 SQL
    pub up_sql: &'static str,
    /// 回滚 SQL
    pub down_sql: &'static str,
}

/// 内置迁移列表
///
/// 按 version 升序排列。v2.4 DDL 参考见 modules/db-store/接口设计.md §6.1。
///
/// 注意：schema_migrations 表是框架表，由 run_migrations 自动创建，
/// 不作为可回滚的迁移步骤。
pub const BUILTIN_MIGRATIONS: &[Migration] = &[
    Migration {
        version: 20260730001,
        description: "创建 agents 表",
        up_sql: "CREATE TABLE IF NOT EXISTS agents (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            class           TEXT NOT NULL CHECK (class IN ('gold','wood','water','fire','earth')),
            realm           TEXT NOT NULL DEFAULT 'qi_refining'
                            CHECK (realm IN ('qi_refining','foundation','core_formation','nascent_soul',
                                   'divine_transformation','refinement_void','ascension')),
            credit          REAL NOT NULL DEFAULT 0.0 CHECK (credit >= 0 AND credit <= 100),
            spirit_stones   INTEGER NOT NULL DEFAULT 0,
            metadata        TEXT NOT NULL DEFAULT '{}',
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
            updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        ) STRICT;",
        down_sql: "DROP TABLE IF EXISTS agents;",
    },
    Migration {
        version: 20260730002,
        description: "创建 tasks 表",
        up_sql: "CREATE TABLE IF NOT EXISTS tasks (
            id              TEXT PRIMARY KEY,
            agent_id        TEXT NOT NULL REFERENCES agents(id),
            r_id            TEXT NOT NULL,
            status          TEXT NOT NULL DEFAULT 'pending'
                            CHECK (status IN ('pending','running','completed','failed','cancelled')),
            task_type       TEXT NOT NULL,
            priority        INTEGER NOT NULL DEFAULT 50,
            input           TEXT NOT NULL DEFAULT '{}',
            output          TEXT,
            error           TEXT,
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
            completed_at    TEXT
        ) STRICT;
        CREATE INDEX IF NOT EXISTS idx_tasks_agent ON tasks(agent_id);
        CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
        CREATE INDEX IF NOT EXISTS idx_tasks_r_id ON tasks(r_id);",
        down_sql: "DROP TABLE IF EXISTS tasks;",
    },
    Migration {
        version: 20260730003,
        description: "创建 symbols 表",
        up_sql: "CREATE TABLE IF NOT EXISTS symbols (
            symbol              TEXT PRIMARY KEY,
            exchange            TEXT NOT NULL,
            product_group       TEXT NOT NULL,
            name_cn             TEXT NOT NULL,
            name_en             TEXT NOT NULL DEFAULT '',
            contract_multiplier REAL NOT NULL,
            price_tick          REAL NOT NULL,
            margin_rate         REAL,
            listing_date        TEXT,
            delivery_date       TEXT,
            is_active           INTEGER NOT NULL DEFAULT 1,
            metadata            TEXT NOT NULL DEFAULT '{}',
            created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
            updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        ) STRICT;
        CREATE INDEX IF NOT EXISTS idx_symbols_group ON symbols(product_group);
        CREATE INDEX IF NOT EXISTS idx_symbols_exchange ON symbols(exchange);",
        down_sql: "DROP TABLE IF EXISTS symbols;",
    },
    Migration {
        version: 20260730004,
        description: "创建 knowledge_pages + knowledge_chunks 表",
        up_sql: "CREATE TABLE IF NOT EXISTS knowledge_pages (
            id          TEXT PRIMARY KEY,
            title       TEXT NOT NULL,
            source      TEXT,
            metadata    TEXT NOT NULL DEFAULT '{}',
            created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
            updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        ) STRICT;
        CREATE TABLE IF NOT EXISTS knowledge_chunks (
            id          TEXT PRIMARY KEY,
            page_id     TEXT NOT NULL REFERENCES knowledge_pages(id),
            content     TEXT NOT NULL,
            embedding   BLOB,
            metadata    TEXT NOT NULL DEFAULT '{}',
            token_count INTEGER,
            created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
            updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        ) STRICT;
        CREATE INDEX IF NOT EXISTS idx_chunks_page ON knowledge_chunks(page_id);",
        down_sql: "DROP TABLE IF EXISTS knowledge_chunks; DROP TABLE IF EXISTS knowledge_pages;",
    },
];

/// 计算 SQL 的 SHA256 校验和
fn checksum(sql: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sql.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 运行全部待执行迁移
///
/// 1. 创建 schema_migrations 版本表（如果不存在）
/// 2. 查询已应用的迁移版本
/// 3. 按 version 升序执行未应用的迁移
/// 4. 记录迁移执行结果
///
/// 来源: modules/db-store/接口设计.md:473-481 — schema_migrations
pub async fn run_migrations(pool: &Pool<Sqlite>) -> Result<(), DbError> {
    // 确保版本表存在
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version     INTEGER PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
            checksum    TEXT NOT NULL,
            execution_ms INTEGER,
            success     INTEGER NOT NULL DEFAULT 1
        ) STRICT;",
    )
    .execute(pool)
    .await
    .map_err(|e| DbError::Migration(format!("创建版本表失败: {}", e)))?;

    // 查询已应用的版本
    let applied: Vec<i64> = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT version FROM schema_migrations WHERE success = 1 ORDER BY version",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::Migration(format!("查询已应用迁移失败: {}", e)))?
    .into_iter()
    .flatten()
    .collect();

    // 执行未应用的迁移
    for migration in BUILTIN_MIGRATIONS {
        if applied.contains(&migration.version) {
            continue;
        }

        let chk = checksum(migration.up_sql);
        let start = std::time::Instant::now();

        // 在事务中执行迁移
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| DbError::Migration(format!("迁移事务开始失败: {}", e)))?;

        let result = sqlx::query(migration.up_sql).execute(&mut *tx).await;

        match result {
            Ok(_) => {
                let elapsed = start.elapsed().as_millis() as i64;
                sqlx::query(
                    "INSERT INTO schema_migrations (version, description, checksum, execution_ms, success) VALUES (?, ?, ?, ?, 1)",
                )
                .bind(migration.version)
                .bind(migration.description)
                .bind(&chk)
                .bind(elapsed)
                .execute(&mut *tx)
                .await
                .map_err(|e| DbError::Migration(format!("记录迁移版本失败: {}", e)))?;

                tx.commit()
                    .await
                    .map_err(|e| DbError::Migration(format!("迁移提交失败: {}", e)))?;
            }
            Err(e) => {
                tx.rollback().await.ok();
                return Err(DbError::Migration(format!(
                    "迁移 v{} '{}' 失败: {}",
                    migration.version, migration.description, e
                )));
            }
        }
    }

    Ok(())
}

/// 回滚最后 N 步迁移
pub async fn rollback_migrations(pool: &Pool<Sqlite>, steps: u32) -> Result<(), DbError> {
    // 查询最近 N 个已应用的迁移（按 version 降序）
    let to_rollback: Vec<(i64, String)> = sqlx::query_as::<_, (i64, String)>(
        "SELECT version, description FROM schema_migrations WHERE success = 1 ORDER BY version DESC LIMIT ?",
    )
    .bind(steps as i64)
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::Migration(format!("查询待回滚迁移失败: {}", e)))?;

    for (version, description) in &to_rollback {
        // 查找对应的回滚 SQL
        if let Some(migration) = BUILTIN_MIGRATIONS.iter().find(|m| m.version == *version) {
            let mut tx = pool
                .begin()
                .await
                .map_err(|e| DbError::Migration(format!("回滚事务开始失败: {}", e)))?;

            // 先删除迁移记录，再执行回滚 SQL（避免 down_sql 删除 schema_migrations 表自身）
            sqlx::query("DELETE FROM schema_migrations WHERE version = ?")
                .bind(version)
                .execute(&mut *tx)
                .await
                .map_err(|e| DbError::Migration(format!("删除迁移记录失败: {}", e)))?;

            sqlx::query(migration.down_sql)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    DbError::Migration(format!(
                        "回滚 v{} '{}' 失败: {}",
                        version, description, e
                    ))
                })?;

            tx.commit()
                .await
                .map_err(|e| DbError::Migration(format!("回滚提交失败: {}", e)))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_pool() -> Pool<Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(":memory:")
            .await
            .unwrap();
        // 启用 WAL 和外键
        sqlx::query("PRAGMA journal_mode = WAL;")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON;")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn test_migration_roundtrip() {
        let pool = setup_pool().await;

        // 正向迁移
        run_migrations(&pool).await.unwrap();

        // 验证版本表
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();

        // 移除 schema_migrations 版本表迁移后，BUILTIN_MIGRATIONS 为 4 个业务迁移
        assert_eq!(count.0, BUILTIN_MIGRATIONS.len() as i64);

        // 验证表存在
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(tables.contains(&"agents".to_string()));
        assert!(tables.contains(&"tasks".to_string()));
        assert!(tables.contains(&"symbols".to_string()));

        // 验证幂等性（再次运行不应报错）
        run_migrations(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn test_migration_rollback() {
        let pool = setup_pool().await;

        run_migrations(&pool).await.unwrap();

        // 回滚 1 步
        rollback_migrations(&pool, 1).await.unwrap();

        let after_rollback: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            after_rollback.0,
            (BUILTIN_MIGRATIONS.len() - 1) as i64,
            "回滚后应少 1 条迁移记录"
        );
    }

    #[test]
    fn test_checksum_stability() {
        let chk1 = checksum("CREATE TABLE test (id TEXT PRIMARY KEY);");
        let chk2 = checksum("CREATE TABLE test (id TEXT PRIMARY KEY);");
        assert_eq!(chk1, chk2);
        assert_eq!(chk1.len(), 64); // SHA256 hex
    }
}
