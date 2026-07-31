//! taiji-gbrain — LVPA 知识库引擎。
//!
//! 提供 GBrainEngine trait（11 方法）、PGLite 引擎实现、三平面配置。
//! 后续 wave 增加分块引擎、嵌入管线、混合检索。

pub mod config;
pub mod engine;
pub mod error;
pub mod page;
pub mod chunk;
pub mod embed;
pub mod search;
pub mod expand;
pub mod cli;

pub use chunk::Chunker;
pub use config::ConfigLoader;
pub use embed::EmbeddingPipeline;
pub use engine::{mock::MockEngine, pglite::PGLiteEngine, GBrainEngine};
pub use error::GBrainError;
pub use expand::QueryExpander;
pub use page::PageManager;
pub use search::graph;
pub use search::Retriever;

#[cfg(test)]
mod tests {
    use super::*;
    use taiji_types::knowledge::{PageInput, SearchOpts};

    fn make_test_config() -> taiji_types::knowledge::GBrainConfig {
        taiji_types::knowledge::GBrainConfig::default()
    }

    #[tokio::test]
    async fn test_pglite_connect_and_schema() {
        let mut engine = PGLiteEngine::new();
        let config = make_test_config();
        engine.connect(&config).await.unwrap();
        engine.init_schema().await.unwrap();
        assert_eq!(engine.page_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_pglite_crud_cycle() {
        let mut engine = PGLiteEngine::new();
        engine.connect(&make_test_config()).await.unwrap();
        engine.init_schema().await.unwrap();

        // CREATE — put_page
        let input = PageInput {
            title: "测试页面".into(),
            content: "这是测试内容。".into(),
            source_id: Some("test_user".into()),
            tags: vec!["test".into()],
            metadata: serde_json::json!({"key": "value"}),
        };
        engine.put_page("test-slug", input).await.unwrap();

        // READ — get_page
        let page = engine.get_page("test-slug").await.unwrap().expect("should exist");
        assert_eq!(page.title, "测试页面");
        assert_eq!(page.tags, vec!["test"]);
        assert_eq!(page.metadata["key"], "value");

        // COUNT
        assert_eq!(engine.page_count().await.unwrap(), 1);

        // LIST
        let pages = engine.list_pages().await.unwrap();
        assert_eq!(pages.len(), 1);

        // UPDATE
        let update = PageInput {
            title: "更新后的标题".into(),
            content: "新内容".into(),
            source_id: None,
            tags: vec![],
            metadata: serde_json::Value::Null,
        };
        engine.put_page("test-slug", update).await.unwrap();
        let updated = engine.get_page("test-slug").await.unwrap().expect("should exist");
        assert_eq!(updated.title, "更新后的标题");
        assert_eq!(updated.tags.len(), 0);

        // DELETE
        let deleted = engine.delete_page("test-slug").await.unwrap();
        assert!(deleted);
        assert!(engine.get_page("test-slug").await.unwrap().is_none());
        assert_eq!(engine.page_count().await.unwrap(), 0);

        // DELETE nonexistent
        let deleted_none = engine.delete_page("nonexistent").await.unwrap();
        assert!(!deleted_none);
    }

    #[tokio::test]
    async fn test_pglite_chunk_operations() {
        let mut engine = PGLiteEngine::new();
        engine.connect(&make_test_config()).await.unwrap();
        engine.init_schema().await.unwrap();

        // 先创建页面
        let input = PageInput {
            title: "分块测试".into(),
            content: "分块内容".into(),
            source_id: None,
            tags: vec![],
            metadata: serde_json::Value::Null,
        };
        engine.put_page("chunk-page", input).await.unwrap();

        // 存储分块
        let chunks = vec![
            taiji_types::knowledge::Chunk {
                id: "chunk:0".into(),
                page_id: "chunk-page".into(),
                seq: 0,
                text: "第一段".into(),
                embedding: Some(vec![0.1, 0.2, 0.3]),
            },
            taiji_types::knowledge::Chunk {
                id: "chunk:1".into(),
                page_id: "chunk-page".into(),
                seq: 1,
                text: "第二段".into(),
                embedding: None,
            },
        ];
        engine.store_chunks(&"chunk-page".into(), &chunks).await.unwrap();

        // 读取分块
        let loaded = engine.get_chunks(&"chunk-page".into()).await.unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "chunk:0");
        assert_eq!(loaded[0].seq, 0);
    }

    #[tokio::test]
    async fn test_pglite_search() {
        let mut engine = PGLiteEngine::new();
        engine.connect(&make_test_config()).await.unwrap();
        engine.init_schema().await.unwrap();

        // 插入多页面
        for (slug, title, content) in &[
            ("arch", "架构总纲", "三层架构设计"),
            ("quant", "量化总纲", "量化交易规则"),
            ("theory", "理论总纲", "量价时空理论"),
        ] {
            let input = PageInput {
                title: title.to_string(),
                content: content.to_string(),
                source_id: Some("system".into()),
                tags: vec![],
                metadata: serde_json::Value::Null,
            };
            engine.put_page(slug, input).await.unwrap();
        }

        // 搜索 "量化"
        let opts = SearchOpts::default();
        let results = engine.search("量化", &opts).await.unwrap();
        assert!(!results.is_empty(), "should find '量化' in title");
        assert_eq!(results[0].page_title, "量化总纲");

        // 搜索 "三层"
        let results2 = engine.search("三层", &opts).await.unwrap();
        assert!(!results2.is_empty(), "should find '三层' in content");
        assert_eq!(results2[0].page_title, "架构总纲");
    }

    #[tokio::test]
    async fn test_pglite_source_filter() {
        let mut engine = PGLiteEngine::new();
        engine.connect(&make_test_config()).await.unwrap();
        engine.init_schema().await.unwrap();

        engine.put_page("public", PageInput {
            title: "公开页面".into(),
            content: "公开内容".into(),
            source_id: Some("public".into()),
            tags: vec![],
            metadata: serde_json::Value::Null,
        }).await.unwrap();

        engine.put_page("private", PageInput {
            title: "私有页面".into(),
            content: "私有内容".into(),
            source_id: Some("private".into()),
            tags: vec![],
            metadata: serde_json::Value::Null,
        }).await.unwrap();

        // 只搜索 private 来源
        let opts = SearchOpts {
            source_filter: Some(vec!["private".into()]),
            ..Default::default()
        };
        let results = engine.search("私有", &opts).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].page_title, "私有页面");
    }

    #[tokio::test]
    async fn test_pglite_not_initialized_error() {
        let engine = PGLiteEngine::new();
        let result = engine.get_page("test").await;
        assert!(result.is_err(), "unconnected engine should return error");
    }

    #[tokio::test]
    async fn test_mock_engine_basic() {
        let mut engine = MockEngine::new();
        engine.connect(&make_test_config()).await.unwrap();
        engine.init_schema().await.unwrap();
        assert_eq!(engine.page_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_disconnect_clears_data() {
        let mut engine = PGLiteEngine::new();
        engine.connect(&make_test_config()).await.unwrap();
        engine.init_schema().await.unwrap();

        engine.put_page("test", PageInput {
            title: "待清除".into(),
            content: "数据".into(),
            source_id: None,
            tags: vec![],
            metadata: serde_json::Value::Null,
        }).await.unwrap();
        assert_eq!(engine.page_count().await.unwrap(), 1);

        engine.disconnect().await.unwrap();
        let result = engine.get_page("test").await;
        assert!(result.is_err(), "disconnected engine should error");
    }
}
