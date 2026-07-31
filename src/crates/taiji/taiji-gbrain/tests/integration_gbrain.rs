//! taiji-gbrain 集成测试
//!
//! 从外部视角测试 GBrainEngine trait 和公共 API。
//! 使用 MockEngine + MockEmbeddingService 避免外部依赖。

use taiji_gbrain::engine::{mock::MockEngine, GBrainEngine};
use taiji_gbrain::Chunker;
use taiji_gbrain::EmbeddingPipeline;
use taiji_gbrain::QueryExpander;
use taiji_llm::EmbeddingService;
use taiji_llm::client::MockClient;
use taiji_types::knowledge::{
    ChunkStrategy, GBrainConfig, PageInput, SearchOpts,
};

// ============================================================================
// GBrainEngine 集成测试
// ============================================================================

/// 创建并初始化一个 MockEngine。
async fn setup_engine() -> MockEngine {
    let mut engine = MockEngine::new();
    engine.connect(&GBrainConfig::default()).await.unwrap();
    engine.init_schema().await.unwrap();
    engine
}

#[tokio::test]
async fn test_engine_lifecycle() {
    let mut engine = MockEngine::new();

    engine.connect(&GBrainConfig::default()).await.unwrap();
    engine.init_schema().await.unwrap();
    assert_eq!(engine.page_count().await.unwrap(), 0);

    engine.disconnect().await.unwrap();
    let result = engine.get_page("any").await;
    assert!(result.is_err(), "disconnected engine should error");
}

#[tokio::test]
async fn test_engine_bulk_crud() {
    let engine = setup_engine().await;

    // 批量写入 10 个页面
    for i in 0..10 {
        let slug = format!("page-{}", i);
        let input = PageInput {
            title: format!("页面 {}", i),
            content: format!("第 {} 页的内容描述", i),
            source_id: Some("bulk-test".into()),
            tags: vec!["test".into()],
            metadata: serde_json::json!({"index": i}),
        };
        engine.put_page(&slug, input).await.unwrap();
    }

    // 验证 page_count
    assert_eq!(engine.page_count().await.unwrap(), 10);

    // 验证 list_pages 返回全部
    let pages = engine.list_pages().await.unwrap();
    assert_eq!(pages.len(), 10);

    // 验证按 updated_at 倒序
    for w in pages.windows(2) {
        assert!(w[0].updated_at >= w[1].updated_at);
    }

    // 验证 get_page
    let page = engine.get_page("page-5").await.unwrap().expect("page-5 should exist");
    assert_eq!(page.title, "页面 5");
    assert_eq!(page.metadata["index"], 5);

    // 逐页删除
    for i in 0..10 {
        let slug = format!("page-{}", i);
        assert!(engine.delete_page(&slug).await.unwrap());
    }
    assert_eq!(engine.page_count().await.unwrap(), 0);

    // 删除不存在页面返回 false
    assert!(!engine.delete_page("nonexistent").await.unwrap());
}

#[tokio::test]
async fn test_engine_update_preserves_created_at() {
    let engine = setup_engine().await;

    let input = PageInput {
        title: "原始标题".into(),
        content: "原始内容".into(),
        source_id: Some("test".into()),
        tags: vec!["v1".into()],
        metadata: serde_json::Value::Null,
    };
    engine.put_page("updatable", input).await.unwrap();

    let original = engine.get_page("updatable").await.unwrap().unwrap();
    let created_at = original.created_at;

    // 更新
    let update = PageInput {
        title: "新标题".into(),
        content: "新内容".into(),
        source_id: Some("test".into()),
        tags: vec!["v2".into()],
        metadata: serde_json::json!({"updated": true}),
    };
    engine.put_page("updatable", update).await.unwrap();

    let updated = engine.get_page("updatable").await.unwrap().unwrap();
    assert_eq!(updated.title, "新标题");
    assert_eq!(updated.created_at, created_at, "created_at should be preserved");
    assert!(updated.updated_at > created_at || updated.updated_at == created_at);
}

#[tokio::test]
async fn test_engine_search_empty_mock() {
    let engine = setup_engine().await;

    // MockEngine.search() 返回空结果
    let results = engine.search("anything", &SearchOpts::default()).await.unwrap();
    assert!(results.is_empty(), "MockEngine should return empty search");
}

#[tokio::test]
async fn test_engine_chunks_basic() {
    let engine = setup_engine().await;

    // 无页面时 chunks 为空
    let chunks = engine.get_chunks(&"no-such-page".into()).await.unwrap();
    assert!(chunks.is_empty());

    // 先创建页面
    engine.put_page("chunk-test", PageInput {
        title: "分块测试".into(),
        content: "测试内容".into(),
        source_id: None,
        tags: vec![],
        metadata: serde_json::Value::Null,
    }).await.unwrap();

    // 存储分块
    use taiji_types::knowledge::Chunk;
    let test_chunks = vec![
        Chunk {
            id: "chunk-test:0".into(),
            page_id: "chunk-test".into(),
            seq: 0,
            text: "第一段".into(),
            embedding: None,
        },
        Chunk {
            id: "chunk-test:1".into(),
            page_id: "chunk-test".into(),
            seq: 1,
            text: "第二段".into(),
            embedding: None,
        },
    ];
    engine.store_chunks(&"chunk-test".into(), &test_chunks).await.unwrap();

    let loaded = engine.get_chunks(&"chunk-test".into()).await.unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].id, "chunk-test:0");
    assert_eq!(loaded[1].text, "第二段");
}

// ============================================================================
// Chunker 集成测试（跨策略）
// ============================================================================

#[test]
fn test_chunker_fixed_edge_cases() {
    // 超大重叠（超过 chunk_size）
    let chunks = Chunker::chunk_text("hello world", &ChunkStrategy::Fixed { chunk_size: 5, overlap: 100 });
    assert!(!chunks.is_empty(), "should clamp overlap and still produce chunks");

    // overlap = chunk_size
    let chunks = Chunker::chunk_text("abcdefghij", &ChunkStrategy::Fixed { chunk_size: 5, overlap: 5 });
    assert!(!chunks.is_empty(), "overlap equal to chunk_size should work");

    // 超长文本
    let long_text = "a".repeat(10000);
    let chunks = Chunker::chunk_text(&long_text, &ChunkStrategy::Fixed { chunk_size: 100, overlap: 10 });
    assert!(chunks.len() > 10, "10000 chars / step=90 should produce ~112 chunks");
}

#[test]
fn test_chunker_paragraph_with_various_newlines() {
    // 正常 \n\n 分隔
    let text = "一段\n\n二段\n\n三段";
    let chunks = Chunker::chunk_text(text, &ChunkStrategy::Paragraph);
    assert_eq!(chunks.len(), 3, "should split on \\n\\n");
}

#[test]
fn test_chunker_sentence_no_period_at_end() {
    let text = "Hello world. How are you? I am fine";
    let chunks = Chunker::chunk_text(text, &ChunkStrategy::Sentence);
    assert_eq!(chunks.len(), 3, "last sentence without period should still be captured");
}

// ============================================================================
// EmbeddingPipeline 集成测试
// ============================================================================

fn make_mock_embedder(dim: usize) -> Box<dyn EmbeddingService> {
    Box::new(taiji_llm::embedding::MockEmbeddingService::new(dim))
}

#[tokio::test]
async fn test_pipeline_process_text_paragraph() {
    let engine = MockEngine::new();
    let embedder = make_mock_embedder(4);
    let pipeline = EmbeddingPipeline::new(
        embedder,
        Box::new(engine),
        ChunkStrategy::Paragraph,
    );

    let text = "第一段。\n\n第二段。\n\n第三段。";
    let chunks = pipeline.process_text(text).await.unwrap();
    assert_eq!(chunks.len(), 3);
    for chunk in &chunks {
        assert!(chunk.embedding.is_some());
        assert_eq!(chunk.embedding.as_ref().unwrap().len(), 4);
    }
}

#[tokio::test]
async fn test_pipeline_process_text_sentence() {
    let engine = MockEngine::new();
    let embedder = make_mock_embedder(4);
    let pipeline = EmbeddingPipeline::new(
        embedder,
        Box::new(engine),
        ChunkStrategy::Sentence,
    );

    let text = "Hello. World! Test? Done.";
    let chunks = pipeline.process_text(text).await.unwrap();
    assert_eq!(chunks.len(), 4);
}

// ============================================================================
// QueryExpander 集成测试
// ============================================================================

#[tokio::test]
async fn test_query_expander_with_mock() {
    let client = Box::new(MockClient::new("量价时空分析\n时空量价研究\nvolume price analysis"));
    let expander = QueryExpander::new(client);

    let variants = expander.expand("量价时空").await.unwrap();
    assert!(!variants.is_empty(), "should produce variants");
    assert!(variants.len() <= 3, "should produce at most 3 variants");
}

#[tokio::test]
async fn test_query_expander_empty_fallback() {
    // MockClient returning empty response
    let client = Box::new(MockClient::new(""));
    let expander = QueryExpander::new(client);

    let _variants = expander.expand("测试查询").await.unwrap();
    // Should fall back to empty (expander returns original as fallback)
    // Actually empty query returns empty vec
    let empty_variants = expander.expand("").await.unwrap();
    assert!(empty_variants.is_empty(), "empty query should return empty");

    let expanded = expander.expand_with_original("核心查询").await.unwrap();
    assert!(!expanded.is_empty(), "expand_with_original should include original");
}
