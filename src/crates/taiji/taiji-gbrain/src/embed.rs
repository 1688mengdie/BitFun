//! EmbeddingPipeline — 嵌入管线。
//!
//! 页面 put 后自动分块 + 嵌入 + 存储。支持批量嵌入优化。
//!
//! 参考: gbrain (MIT) core/contextual-retrieval-service.ts — R-5-201 — Rust 翻译实现
//!       taiji-llm EmbeddingService trait — R-5-201 — 通过 trait 调用，不自行调用 LLM API

use taiji_types::knowledge::{Chunk, ChunkStrategy};

use crate::chunk::Chunker;
use crate::engine::GBrainEngine;
use crate::error::GBrainError;

/// 嵌入管线 — 分块 + 嵌入 + 存储全链路（R-5-201）。
pub struct EmbeddingPipeline {
    embedder: Box<dyn taiji_llm::EmbeddingService>,
    engine: Box<dyn GBrainEngine>,
    strategy: ChunkStrategy,
}

impl EmbeddingPipeline {
    /// 创建嵌入管线。
    pub fn new(
        embedder: Box<dyn taiji_llm::EmbeddingService>,
        engine: Box<dyn GBrainEngine>,
        strategy: ChunkStrategy,
    ) -> Self {
        Self {
            embedder,
            engine,
            strategy,
        }
    }

    /// 处理页面：读取 → 分块 → 嵌入 → 存储。
    ///
    /// 完整流程：
    /// 1. 从引擎读取页面内容
    /// 2. 按策略分块
    /// 3. 批量生成嵌入向量
    /// 4. 存储分块到引擎
    pub async fn process_page(&self, page_slug: &str) -> Result<Vec<Chunk>, GBrainError> {
        let page = self
            .engine
            .get_page(page_slug)
            .await?
            .ok_or_else(|| GBrainError::not_found(format!("page '{}' not found", page_slug)))?;

        let chunks = self.process_text(&page.content).await?;

        // 赋值 page_id 和 id
        let chunks: Vec<Chunk> = chunks
            .into_iter()
            .map(|c| Chunk {
                id: format!("{}:chunk:{}", page_slug, c.seq),
                page_id: page_slug.to_string(),
                seq: c.seq,
                text: c.text,
                embedding: c.embedding,
            })
            .collect();

        self.engine
            .store_chunks(&page_slug.to_string(), &chunks)
            .await?;

        Ok(chunks)
    }

    /// 处理文本：分块 → 嵌入（不存储）。
    ///
    /// 返回带嵌入向量的 Chunk 列表（page_id 为空，需调用方赋值）。
    pub async fn process_text(&self, text: &str) -> Result<Vec<Chunk>, GBrainError> {
        let inputs = Chunker::chunk_text(text, &self.strategy);
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        // 批量嵌入优化：一次性嵌入所有分块
        let texts: Vec<String> = inputs.iter().map(|c| c.text.clone()).collect();
        let embeddings = self
            .embedder
            .embed(&texts)
            .await
            .map_err(|e| GBrainError::query(format!("embedding failed: {}", e)))?;

        let dim = self.embedder.dimension();

        let chunks: Vec<Chunk> = inputs
            .into_iter()
            .zip(embeddings)
            .map(|(input, vec)| {
                // f32 → f64 转换
                let f64_vec: Vec<f64> = if vec.len() == dim {
                    vec.into_iter().map(|v| v as f64).collect()
                } else {
                    vec![0.0_f64; dim]
                };
                Chunk {
                    id: String::new(), // 由 process_page 赋值
                    page_id: String::new(),
                    seq: input.seq,
                    text: input.text,
                    embedding: Some(f64_vec),
                }
            })
            .collect();

        Ok(chunks)
    }

    /// 获取底层引擎引用。
    pub fn engine(&self) -> &dyn GBrainEngine {
        self.engine.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taiji_llm::EmbeddingService;
    use taiji_types::knowledge::PageInput;

    use crate::engine::mock::MockEngine;
    use crate::engine::GBrainEngine;

    fn make_mock_embedder(dim: usize) -> Box<dyn EmbeddingService> {
        Box::new(taiji_llm::embedding::MockEmbeddingService::new(dim))
    }

    async fn make_pipeline(
        strategy: ChunkStrategy,
    ) -> (EmbeddingPipeline, Box<dyn GBrainEngine>) {
        let mut engine = MockEngine::new();
        engine
            .connect(&taiji_types::knowledge::GBrainConfig::default())
            .await
            .unwrap();
        engine.init_schema().await.unwrap();

        let embedder = make_mock_embedder(8);
        let pipeline = EmbeddingPipeline::new(embedder, Box::new(MockEngine::new()), strategy);

        (pipeline, Box::new(engine))
    }

    #[tokio::test]
    async fn test_process_text_fixed() {
        let pipeline = make_pipeline(ChunkStrategy::Fixed {
            chunk_size: 10,
            overlap: 0,
        })
        .await
        .0;

        let text = "abcdefghijklmnopqrstuvwxyz";
        let chunks = pipeline.process_text(text).await.unwrap();

        assert!(!chunks.is_empty(), "should produce chunks");
        for chunk in &chunks {
            assert!(
                chunk.embedding.is_some(),
                "chunk {} should have embedding",
                chunk.seq
            );
            let emb = chunk.embedding.as_ref().unwrap();
            assert_eq!(emb.len(), 8, "embedding dimension should be 8");
        }
    }

    #[tokio::test]
    async fn test_process_text_empty() {
        let pipeline = make_pipeline(ChunkStrategy::default()).await.0;
        let chunks = pipeline.process_text("").await.unwrap();
        assert!(chunks.is_empty());
    }

    #[tokio::test]
    async fn test_process_page_flow() {
        let mut engine = MockEngine::new();
        engine
            .connect(&taiji_types::knowledge::GBrainConfig::default())
            .await
            .unwrap();
        engine.init_schema().await.unwrap();

        // 先创建页面
        engine
            .put_page(
                "test-page",
                PageInput {
                    title: "测试页面".into(),
                    content: "第一段。\n\n第二段。\n\n第三段。".into(),
                    source_id: Some("test".into()),
                    tags: vec![],
                    metadata: serde_json::Value::Null,
                },
            )
            .await
            .unwrap();

        // 使用同一个引擎创建管线
        let pipeline = EmbeddingPipeline::new(
            make_mock_embedder(8),
            Box::new(engine),
            ChunkStrategy::Paragraph,
        );

        let chunks = pipeline.process_page("test-page").await.unwrap();
        assert!(!chunks.is_empty(), "should produce and store chunks");
        assert_eq!(chunks.len(), 3);
        assert!(
            chunks[0].embedding.is_some(),
            "chunk should have embedding"
        );

        // 验证 chunks 已存储
        let stored = pipeline
            .engine()
            .get_chunks(&"test-page".into())
            .await
            .unwrap();
        assert_eq!(stored.len(), 3);
    }

    #[tokio::test]
    async fn test_process_page_not_found() {
        let (pipeline, _engine) = make_pipeline(ChunkStrategy::default()).await;
        let result = pipeline.process_page("nonexistent").await;
        assert!(result.is_err(), "should error on missing page");
    }
}
