//! db-store（灵脉）存储后端
//!
//! 来源: modules/db-store/接口设计.md:14-46 — StorageBackend trait + SQLiteBackend
//! 参考: gbrain engine.ts:659-795 StorageBackend 接口模式 (MIT)

use crate::config::DbConfig;
use crate::error::DbError;
use crate::query::{PaginatedResult, QueryFilter};
use crate::transaction::TransactionBackend;
use async_trait::async_trait;
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{Column, Row, Sqlite, SqlitePool};
use std::str::FromStr;
use std::sync::Mutex;

/// 统一存储后端 trait
///
/// LVPA v2.4：单 SQLite 后端设计，去 PGLite/Postgres 分支。
/// 提供 CRUD + 批量 + 分页 + 事务 + SQL 直接执行接口。
///
/// 来源: modules/db-store/接口设计.md:14-46 — StorageBackend trait
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// 连接数据库
    async fn connect(&self, path: &str) -> Result<(), DbError>;

    /// 断开连接
    async fn disconnect(&self) -> Result<(), DbError>;

    /// 执行数据库迁移
    async fn migrate(&self) -> Result<(), DbError>;

    // === 通用 CRUD ===

    /// 插入记录
    async fn insert(&self, table: &str, data: &Value) -> Result<Value, DbError>;

    /// 按 ID 获取记录
    async fn get(&self, table: &str, id: &str) -> Result<Option<Value>, DbError>;

    /// 更新记录
    async fn update(&self, table: &str, id: &str, data: &Value) -> Result<Value, DbError>;

    /// 删除记录
    async fn delete(&self, table: &str, id: &str) -> Result<bool, DbError>;

    /// 带过滤器列表查询
    async fn list(&self, table: &str, filter: Option<QueryFilter>) -> Result<Vec<Value>, DbError>;

    // === 批量操作 ===

    /// 批量插入
    async fn batch_insert(&self, table: &str, rows: &[Value]) -> Result<u64, DbError>;

    // === 分页 ===

    /// 分页查询
    async fn paginate(
        &self,
        table: &str,
        filter: Option<QueryFilter>,
        page: u32,
        per_page: u32,
    ) -> Result<PaginatedResult, DbError>;

    // === 事务 ===

    /// 执行事务
    async fn transaction<F, T>(&self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&mut dyn TransactionBackend) -> Result<T, DbError> + Send,
        T: Send;

    // === SQL 直接执行 ===

    /// 执行 SQL（无返回行）
    async fn execute_raw(&self, sql: &str) -> Result<u64, DbError>;

    /// 查询 SQL（返回行数据）
    async fn query_raw(&self, sql: &str) -> Result<Vec<Value>, DbError>;
}

/// SQLite 存储后端实现
///
/// 委托 sqlx-sqlite 实现全部 CRUD 操作。
/// WAL 模式默认开启，支持 `:memory:` 测试数据库。
///
/// 使用 Mutex 内部可变性以实现 `&self` 连接管理。
pub struct SQLiteBackend {
    pool: Mutex<Option<SqlitePool>>,
    config: DbConfig,
}

impl SQLiteBackend {
    /// 创建新的 SQLiteBackend
    pub fn new(config: DbConfig) -> Self {
        Self {
            pool: Mutex::new(None),
            config,
        }
    }

    /// 创建使用 `:memory:` 的 SQLiteBackend（测试用）
    pub fn in_memory() -> Self {
        Self::new(DbConfig::in_memory())
    }

    /// 获取连接池引用（公开，用于直接 sqlx 查询）
    pub fn pool(&self) -> Result<SqlitePool, DbError> {
        self.pool
            .lock()
            .map_err(|e| DbError::connection(format!("锁获取失败: {}", e)))?
            .clone()
            .ok_or_else(|| DbError::connection("数据库未连接".to_string()))
    }

    /// 从数据库行转换为 Value
    fn row_to_value(row: &SqliteRow) -> Value {
        let mut map = serde_json::Map::new();
        for (i, column) in row.columns().iter().enumerate() {
            let name = column.name();
            let val: Value = match row.try_get::<Option<String>, _>(i) {
                Ok(Some(s)) => {
                    serde_json::from_str(&s).unwrap_or(Value::String(s))
                }
                Ok(None) => Value::Null,
                Err(_) => Value::Null,
            };
            map.insert(name.to_string(), val);
        }
        Value::Object(map)
    }

    /// 执行 PRAGMA 配置
    async fn apply_pragmas(&self, pool: &SqlitePool) -> Result<(), DbError> {
        for pragma in self.config.pragmas() {
            sqlx::query(&pragma)
                .execute(pool)
                .await
                .map_err(|e| DbError::connection(format!("PRAGMA 执行失败: {}: {}", pragma, e)))?;
        }
        Ok(())
    }

    /// 从 QueryFilter 生成 WHERE 子句（值内联，无参数绑定）
    fn where_clause(filter: &Option<QueryFilter>) -> String {
        match filter {
            Some(f) => Self::filter_to_sql(f),
            None => "1=1".into(),
        }
    }

    /// 将 Value 转为 SQL 字面量
    fn value_to_sql(v: &Value) -> String {
        match v {
            Value::String(s) => format!("'{}'", s.replace('\'', "''")),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => {
                if *b { "1" } else { "0" }.to_string()
            }
            Value::Null => "NULL".into(),
            _ => v.to_string(),
        }
    }

    /// 将 QueryFilter 转为内联值的 SQL 子句
    fn filter_to_sql(f: &QueryFilter) -> String {
        match f {
            QueryFilter::Eq(field, value) => {
                format!("{} = {}", field, Self::value_to_sql(value))
            }
            QueryFilter::Ne(field, value) => {
                format!("{} != {}", field, Self::value_to_sql(value))
            }
            QueryFilter::Gt(field, value) => {
                format!("{} > {}", field, Self::value_to_sql(value))
            }
            QueryFilter::Lt(field, value) => {
                format!("{} < {}", field, Self::value_to_sql(value))
            }
            QueryFilter::In(field, values) => {
                if values.is_empty() {
                    return "1=0".into();
                }
                let list: Vec<String> = values.iter().map(Self::value_to_sql).collect();
                format!("{} IN ({})", field, list.join(", "))
            }
            QueryFilter::FieldExists(field) => {
                if field.contains('.') {
                    let parts: Vec<&str> = field.splitn(2, '.').collect();
                    format!("json_extract({}, '$.{}') IS NOT NULL", parts[0], parts[1])
                } else {
                    format!("{} IS NOT NULL", field)
                }
            }
            QueryFilter::JsonEq { field, path, value } => {
                format!(
                    "json_extract({}, '$.{}') = {}",
                    field, path, Self::value_to_sql(value)
                )
            }
            QueryFilter::And(filters) => {
                if filters.is_empty() {
                    return "1=1".into();
                }
                let clauses: Vec<String> = filters.iter().map(Self::filter_to_sql).collect();
                clauses.join(" AND ")
            }
            QueryFilter::Or(filters) => {
                if filters.is_empty() {
                    return "1=0".into();
                }
                let clauses: Vec<String> = filters.iter().map(Self::filter_to_sql).collect();
                clauses.join(" OR ")
            }
        }
    }
}

#[async_trait]
impl StorageBackend for SQLiteBackend {
    async fn connect(&self, path: &str) -> Result<(), DbError> {
        let opts = SqliteConnectOptions::from_str(path)
            .map_err(|e| DbError::connection(format!("无效路径: {}", e)))?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .map_err(|e| DbError::connection(e.to_string()))?;

        self.apply_pragmas(&pool).await?;

        let mut guard = self.pool
            .lock()
            .map_err(|e| DbError::connection(format!("锁获取失败: {}", e)))?;
        *guard = Some(pool);
        Ok(())
    }

    async fn disconnect(&self) -> Result<(), DbError> {
        if let Ok(pool) = self.pool() {
            pool.close().await;
        }
        let mut guard = self.pool
            .lock()
            .map_err(|e| DbError::connection(format!("锁获取失败: {}", e)))?;
        *guard = None;
        Ok(())
    }

    async fn migrate(&self) -> Result<(), DbError> {
        let pool = self.pool()?;
        crate::migration::run_migrations(&pool).await
    }

    async fn insert(&self, table: &str, data: &Value) -> Result<Value, DbError> {
        let pool = self.pool()?;
        let obj = data
            .as_object()
            .ok_or_else(|| DbError::query("insert data 必须是 JSON 对象".to_string()))?;

        let columns: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
        let placeholders: Vec<String> = (0..columns.len())
            .map(|i| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            table,
            columns.join(", "),
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for col in &columns {
            let val_str = obj.get(*col)
                .map(|v| {
                    if v.is_string() {
                        v.as_str().unwrap_or("").to_string()
                    } else {
                        v.to_string()
                    }
                })
                .unwrap_or_default();
            query = query.bind(val_str);
        }

        query
            .execute(&pool)
            .await
            .map_err(|e| DbError::query(format!("insert 失败: {}", e)))?;

        // 返回 id 字段（如果有）
        if let Some(id_val) = obj.get("id") {
            self.get(table, id_val.as_str().unwrap_or(""))
                .await
                .map(|opt| opt.unwrap_or(Value::Null))
        } else {
            Ok(Value::Null)
        }
    }

    async fn get(&self, table: &str, id: &str) -> Result<Option<Value>, DbError> {
        let pool = self.pool()?;
        let sql = format!("SELECT * FROM {} WHERE id = ?1", table);

        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| DbError::query(format!("get 失败: {}", e)))?;

        Ok(row.map(|r| Self::row_to_value(&r)))
    }

    async fn update(&self, table: &str, id: &str, data: &Value) -> Result<Value, DbError> {
        let pool = self.pool()?;
        let obj = data
            .as_object()
            .ok_or_else(|| DbError::query("update data 必须是 JSON 对象".to_string()))?;

        // 构建 SET 子句，排除 id 和 created_at
        let set_pairs: Vec<(String, String)> = obj
            .iter()
            .filter(|(k, _)| *k != "id" && *k != "created_at")
            .enumerate()
            .map(|(i, (k, v))| {
                let val_str = if v.is_string() {
                    v.as_str().unwrap_or("").to_string()
                } else {
                    v.to_string()
                };
                (format!("{} = ?{}", k, i + 1), val_str)
            })
            .collect();

        if set_pairs.is_empty() {
            return self.get(table, id).await.map(|opt| opt.unwrap_or(Value::Null));
        }

        let set_clause: Vec<String> = set_pairs.iter().map(|(s, _)| s.clone()).collect();
        let values: Vec<String> = set_pairs.into_iter().map(|(_, v)| v).collect();

        let sql = format!(
            "UPDATE {} SET {} WHERE id = ?{}",
            table,
            set_clause.join(", "),
            values.len() + 1
        );

        let mut query = sqlx::query(&sql);
        for val in &values {
            query = query.bind(val);
        }
        query = query.bind(id);

        query
            .execute(&pool)
            .await
            .map_err(|e| DbError::query(format!("update 失败: {}", e)))?;

        self.get(table, id).await.map(|opt| opt.unwrap_or(Value::Null))
    }

    async fn delete(&self, table: &str, id: &str) -> Result<bool, DbError> {
        let pool = self.pool()?;
        let sql = format!("DELETE FROM {} WHERE id = ?1", table);

        let result = sqlx::query(&sql)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| DbError::query(format!("delete 失败: {}", e)))?;

        Ok(result.rows_affected() > 0)
    }

    async fn list(&self, table: &str, filter: Option<QueryFilter>) -> Result<Vec<Value>, DbError> {
        let pool = self.pool()?;
        let where_clause = Self::where_clause(&filter);
        let sql = format!("SELECT * FROM {} WHERE {}", table, where_clause);

        let rows = sqlx::query(&sql)
            .fetch_all(&pool)
            .await
            .map_err(|e| DbError::query(format!("list 失败: {}", e)))?;

        Ok(rows.into_iter().map(|r| Self::row_to_value(&r)).collect())
    }

    async fn batch_insert(&self, table: &str, rows: &[Value]) -> Result<u64, DbError> {
        if rows.is_empty() {
            return Ok(0);
        }

        let pool = self.pool()?;

        // 从第一行获取列名
        let first = rows[0]
            .as_object()
            .ok_or_else(|| DbError::query("rows 必须是 JSON 对象数组".to_string()))?;
        let columns: Vec<&str> = first.keys().map(|k| k.as_str()).collect();
        let placeholders: Vec<String> = (0..columns.len())
            .map(|i| format!("?{}", i + 1))
            .collect();

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            table,
            columns.join(", "),
            placeholders.join(", ")
        );

        let mut total = 0u64;
        for row in rows {
            let obj = row
                .as_object()
                .ok_or_else(|| DbError::query("rows 元素必须是 JSON 对象".to_string()))?;

            let mut query = sqlx::query(&sql);
            for col_name in &columns {
                let val = obj
                    .get(*col_name)
                    .map(|v| {
                        if v.is_string() {
                            v.as_str().unwrap_or("").to_string()
                        } else {
                            v.to_string()
                        }
                    })
                    .unwrap_or_default();
                query = query.bind(val);
            }

            query
                .execute(&pool)
                .await
                .map_err(|e| DbError::query(format!("batch_insert 失败: {}", e)))?;
            total += 1;
        }

        Ok(total)
    }

    async fn paginate(
        &self,
        table: &str,
        filter: Option<QueryFilter>,
        page: u32,
        per_page: u32,
    ) -> Result<PaginatedResult, DbError> {
        if per_page == 0 {
            return Ok(PaginatedResult::empty(page, 0));
        }

        let pool = self.pool()?;
        let where_clause = Self::where_clause(&filter);

        // 查询总数
        let count_sql = format!("SELECT COUNT(*) FROM {} WHERE {}", table, where_clause);
        let total: (i64,) = sqlx::query_as(&count_sql)
            .fetch_one(&pool)
            .await
            .map_err(|e| DbError::query(format!("count 查询失败: {}", e)))?;

        let offset = page.saturating_sub(1) * per_page;
        let limit = per_page;

        let data_sql = format!(
            "SELECT * FROM {} WHERE {} LIMIT {} OFFSET {}",
            table, where_clause, limit, offset
        );

        let rows = sqlx::query(&data_sql)
            .fetch_all(&pool)
            .await
            .map_err(|e| DbError::query(format!("paginate 失败: {}", e)))?;

        let items: Vec<Value> = rows.into_iter().map(|r| Self::row_to_value(&r)).collect();

        Ok(PaginatedResult::with_items(items, total.0 as u64, page, per_page))
    }

    async fn transaction<F, T>(&self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&mut dyn TransactionBackend) -> Result<T, DbError> + Send,
        T: Send,
    {
        let pool = self.pool()?;

        let tx = pool
            .begin()
            .await
            .map_err(|e| DbError::Transaction(format!("事务开始失败: {}", e)))?;

        // 创建实现 TransactionBackend 的包装器
        struct SqliteTx<'a>(sqlx::Transaction<'a, Sqlite>);

        impl TransactionBackend for SqliteTx<'_> {
            fn execute(
                &mut self,
                _sql: &str,
                _params: &[&dyn sqlx::Encode<'_, sqlx::Sqlite>],
            ) -> Result<u64, DbError> {
                Err(DbError::Transaction(
                    "execute with params not yet supported".into(),
                ))
            }

            fn query(
                &mut self,
                _sql: &str,
                _params: &[&dyn sqlx::Encode<'_, sqlx::Sqlite>],
            ) -> Result<Vec<Value>, DbError> {
                Err(DbError::Transaction(
                    "query with params not yet supported".into(),
                ))
            }

            fn commit(&mut self) -> Result<(), DbError> {
                Err(DbError::Transaction(
                    "手动 commit 不支持，使用闭包返回自动 commit".into(),
                ))
            }

            fn rollback(&mut self) -> Result<(), DbError> {
                Err(DbError::Transaction(
                    "手动 rollback 不支持，返回 Err 自动 rollback".into(),
                ))
            }
        }

        let mut tx_wrapper = SqliteTx(tx);
        let result = f(&mut tx_wrapper)?;

        tx_wrapper
            .0
            .commit()
            .await
            .map_err(|e| DbError::Transaction(format!("事务提交失败: {}", e)))?;

        Ok(result)
    }

    async fn execute_raw(&self, sql: &str) -> Result<u64, DbError> {
        let pool = self.pool()?;
        let result = sqlx::query(sql)
            .execute(&pool)
            .await
            .map_err(|e| DbError::query(format!("execute_raw 失败: {}", e)))?;
        Ok(result.rows_affected())
    }

    async fn query_raw(&self, sql: &str) -> Result<Vec<Value>, DbError> {
        let pool = self.pool()?;
        let rows = sqlx::query(sql)
            .fetch_all(&pool)
            .await
            .map_err(|e| DbError::query(format!("query_raw 失败: {}", e)))?;
        Ok(rows.into_iter().map(|r| Self::row_to_value(&r)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn setup_backend() -> SQLiteBackend {
        let backend = SQLiteBackend::in_memory();
        backend.connect(":memory:").await.unwrap();
        backend
            .execute_raw(
                "CREATE TABLE IF NOT EXISTS test_items (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'active',
                    metadata TEXT NOT NULL DEFAULT '{}',
                    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
                    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
                ) STRICT",
            )
            .await
            .unwrap();
        backend
    }

    #[tokio::test]
    async fn test_connect_and_disconnect() {
        let backend = SQLiteBackend::in_memory();
        backend.connect(":memory:").await.unwrap();
        assert!(backend.pool.lock().unwrap().is_some());
        backend.disconnect().await.unwrap();
        assert!(backend.pool.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn test_crud_cycle() {
        let backend = setup_backend().await;

        // insert
        let data = json!({
            "id": "test-001",
            "name": "测试项目",
            "status": "active"
        });
        backend.insert("test_items", &data).await.unwrap();

        // get
        let fetched = backend.get("test_items", "test-001").await.unwrap();
        assert!(fetched.is_some());
        let obj = fetched.unwrap();
        assert_eq!(obj["name"], "测试项目");

        // update
        let update_data = json!({"name": "已更新项目", "status": "inactive"});
        let updated = backend
            .update("test_items", "test-001", &update_data)
            .await
            .unwrap();
        assert_eq!(updated["name"], "已更新项目");

        // delete
        let deleted = backend.delete("test_items", "test-001").await.unwrap();
        assert!(deleted);
        let after_delete = backend.get("test_items", "test-001").await.unwrap();
        assert!(after_delete.is_none());
    }

    #[tokio::test]
    async fn test_list_and_paginate() {
        let backend = setup_backend().await;

        for i in 0..15 {
            let data = json!({
                "id": format!("item-{:03}", i),
                "name": format!("项目 {}", i),
                "status": if i < 10 { "active" } else { "inactive" }
            });
            backend.insert("test_items", &data).await.unwrap();
        }

        // list with filter
        let filter = QueryFilter::Eq("status".into(), Value::String("active".into()));
        let items = backend.list("test_items", Some(filter)).await.unwrap();
        assert_eq!(items.len(), 10);

        // paginate
        let page = backend
            .paginate("test_items", None, 1, 5)
            .await
            .unwrap();
        assert_eq!(page.items.len(), 5);
        assert_eq!(page.total, 15);
        assert_eq!(page.total_pages, 3);
    }

    #[tokio::test]
    async fn test_execute_raw() {
        let backend = setup_backend().await;

        let count = backend
            .execute_raw("INSERT INTO test_items (id, name) VALUES ('raw-1', 'raw')")
            .await
            .unwrap();
        assert_eq!(count, 1);

        let rows = backend
            .query_raw("SELECT * FROM test_items WHERE id = 'raw-1'")
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[tokio::test]
    async fn test_batch_insert() {
        let backend = setup_backend().await;

        let rows: Vec<Value> = (0..5)
            .map(|i| {
                json!({
                    "id": format!("batch-{:03}", i),
                    "name": format!("批量 {}", i),
                    "status": "active"
                })
            })
            .collect();

        let count = backend.batch_insert("test_items", &rows).await.unwrap();
        assert_eq!(count, 5);

        let all = backend.list("test_items", None).await.unwrap();
        assert_eq!(all.len(), 5);
    }
}
