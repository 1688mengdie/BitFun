//! db-store 集成测试 — CRUD + 事务 + 迁移 + 分页
//!
//! 来源: Phase-1-派发提示词.md:543-555 — 测试用例表

use taiji_infra_db_store::{
    DbConfig, QueryFilter, SQLiteBackend, StorageBackend,
};
use serde_json::{json, Value};

/// 创建测试用 SQLiteBackend（:memory:）
async fn setup() -> SQLiteBackend {
    let backend = SQLiteBackend::new(DbConfig::in_memory());
    backend.connect(":memory:").await.unwrap();

    // 创建测试表（使用 STRICT）
    backend
        .execute_raw(
            "CREATE TABLE IF NOT EXISTS test_users (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                score REAL NOT NULL DEFAULT 0.0,
                metadata TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
            ) STRICT",
        )
        .await
        .unwrap();

    // 验证 STRICT 表
    backend
        .execute_raw("INSERT INTO test_users (id, name, email) VALUES ('setup-init', 'init', 'init@test.com')")
        .await
        .unwrap();

    backend
}

#[tokio::test]
async fn test_connect_disconnect() {
    let backend = SQLiteBackend::new(DbConfig::in_memory());
    backend.connect(":memory:").await.unwrap();
    backend.disconnect().await.unwrap();
}

#[tokio::test]
async fn test_full_crud_cycle() {
    let backend = setup().await;

    // Create
    let data = json!({
        "id": "u-1",
        "name": "张三",
        "email": "zhangsan@test.com",
        "status": "active",
        "score": 85.5
    });
    let inserted = backend.insert("test_users", &data).await.unwrap();
    // insert returns the full record
    assert_eq!(inserted["name"], "张三");

    // Read
    let fetched = backend.get("test_users", "u-1").await.unwrap();
    assert!(fetched.is_some());
    let obj = fetched.unwrap();
    assert_eq!(obj["name"], "张三");
    assert_eq!(obj["status"], "active");
    assert_eq!(obj["email"], "zhangsan@test.com");

    // Update
    let update_data = json!({
        "name": "李四",
        "status": "inactive",
        "score": 90.0
    });
    let updated = backend
        .update("test_users", "u-1", &update_data)
        .await
        .unwrap();
    assert_eq!(updated["name"], "李四");
    assert_eq!(updated["status"], "inactive");

    // Delete
    let deleted = backend.delete("test_users", "u-1").await.unwrap();
    assert!(deleted);

    // Verify deleted
    let after = backend.get("test_users", "u-1").await.unwrap();
    assert!(after.is_none());
}

#[tokio::test]
async fn test_list_with_filter() {
    let backend = setup().await;

    // 插入测试数据
    for i in 0..5 {
        let status = if i < 3 { "active" } else { "inactive" };
        let data = json!({
            "id": format!("u-{}", i),
            "name": format!("User {}", i),
            "email": format!("user{}@test.com", i),
            "status": status,
            "score": 60.0 + i as f64 * 5.0
        });
        backend.insert("test_users", &data).await.unwrap();
    }

    // 过滤 active（含 setup-init 的默认 status='active'）
    let filter = QueryFilter::Eq("status".into(), Value::String("active".into()));
    let active_users = backend.list("test_users", Some(filter)).await.unwrap();
    assert_eq!(active_users.len(), 4); // setup-init + u-0, u-1, u-2

    // 过滤 score > 65
    let filter = QueryFilter::Gt("score".into(), Value::Number(serde_json::Number::from_f64(65.0).unwrap()));
    let high_score = backend.list("test_users", Some(filter)).await.unwrap();
    assert_eq!(high_score.len(), 3); // u-2, u-3, u-4 (scores 70, 75, 80)
}

#[tokio::test]
async fn test_pagination() {
    let backend = setup().await;

    for i in 0..25 {
        let data = json!({
            "id": format!("u-{:02}", i),
            "name": format!("User {}", i),
            "email": format!("user{}@test.com", i),
            "status": "active",
            "score": i as f64
        });
        backend.insert("test_users", &data).await.unwrap();
    }

    // 第 1 页，每页 10 条
    let page1 = backend
        .paginate("test_users", None, 1, 10)
        .await
        .unwrap();
    assert_eq!(page1.items.len(), 10);
    assert_eq!(page1.total, 26);
    assert_eq!(page1.total_pages, 3);

    // 第 3 页（26 = 1 setup-init + 25 test items）
    let page3 = backend
        .paginate("test_users", None, 3, 10)
        .await
        .unwrap();
    assert_eq!(page3.items.len(), 6); // 26 - 2*10 = 6
}

#[tokio::test]
async fn test_batch_insert() {
    let backend = setup().await;

    let rows: Vec<Value> = (0..10)
        .map(|i| {
            json!({
                "id": format!("batch-{:02}", i),
                "name": format!("Batch {}", i),
                "email": format!("batch{}@test.com", i),
                "status": "active",
                "score": i as f64 * 10.0
            })
        })
        .collect();

    let count = backend.batch_insert("test_users", &rows).await.unwrap();
    assert_eq!(count, 10);

    let all = backend.list("test_users", None).await.unwrap();
    assert_eq!(all.len(), 11); // 10 batch + 1 init
}

#[tokio::test]
async fn test_execute_and_query_raw() {
    let backend = setup().await;

    let affected = backend
        .execute_raw("INSERT INTO test_users (id, name, email) VALUES ('raw-1', 'Raw SQL', 'raw@test.com')")
        .await
        .unwrap();
    assert_eq!(affected, 1);

    let rows = backend
        .query_raw("SELECT id, name FROM test_users WHERE id = 'raw-1'")
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], "Raw SQL");
}

#[tokio::test]
async fn test_migration() {
    let backend = SQLiteBackend::new(DbConfig::in_memory());
    backend.connect(":memory:").await.unwrap();

    // 执行迁移
    backend.migrate().await.unwrap();

    // 验证核心表存在
    let tables = backend
        .query_raw("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .await
        .unwrap();
    let table_names: Vec<&str> = tables
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(table_names.contains(&"agents"));
    assert!(table_names.contains(&"tasks"));
    assert!(table_names.contains(&"symbols"));
    assert!(table_names.contains(&"schema_migrations"));
}

#[tokio::test]
async fn test_ddl_strict_constraints() {
    let backend = setup().await;

    // STRICT 表拒绝未定义列
    let result = backend
        .execute_raw("INSERT INTO test_users (id, name, email, undefined_col) VALUES ('err-1', 'Err', 'e@t.com', 'x')")
        .await;
    assert!(result.is_err());

    // NOT NULL 约束
    let result = backend
        .execute_raw("INSERT INTO test_users (id) VALUES ('err-2')")
        .await;
    assert!(result.is_err());
}
