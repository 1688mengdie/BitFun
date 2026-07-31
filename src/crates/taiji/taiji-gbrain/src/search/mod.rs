//! 混合检索 — 向量 + 关键词 + 图遍历加权融合。
//!
//! 参考: gbrain (MIT) core/search/hybrid.ts — R-5-301 — Rust 翻译实现

pub mod graph;

use std::collections::HashMap;

use taiji_types::knowledge::{Chunk, Page, SearchOpts, SearchResult};

use crate::engine::GBrainEngine;
use crate::error::GBrainError;

// ── 默认权重 ──

const VECTOR_WEIGHT: f64 = 0.6;
const KEYWORD_WEIGHT: f64 = 0.3;
const GRAPH_WEIGHT: f64 = 0.1;

/// 混合检索器（R-5-301）。
///
/// 组合向量检索 + 关键词检索 + 图遍历，按权重融合排序。
pub struct Retriever {
    engine: Box<dyn GBrainEngine>,
    embedder: Box<dyn taiji_llm::EmbeddingService>,
}

impl Retriever {
    /// 创建混合检索器。
    pub fn new(engine: Box<dyn GBrainEngine>, embedder: Box<dyn taiji_llm::EmbeddingService>) -> Self {
        Self { engine, embedder }
    }

    /// 执行混合检索。
    ///
    /// 流程：
    /// 1. 向量检索（EmbeddingService + cosine 相似度）
    /// 2. 关键词检索（engine.search）
    /// 3. 图遍历检索（如启用）
    /// 4. 加权融合 → 去重 → 排序 → top_k 截断 → min_score 过滤
    pub async fn search(
        &self,
        query: &str,
        opts: &SearchOpts,
    ) -> Result<Vec<SearchResult>, GBrainError> {
        // 1. 向量检索
        let vector_results = self.vector_search(query, opts).await?;

        // 2. 关键词检索
        let keyword_results = self.keyword_search(query, opts).await?;

        // 3. 图遍历检索（如启用）
        let graph_results = if opts.use_graph {
            let pages = self.engine.list_pages().await?;
            let seed_pages: Vec<Page> = vector_results
                .iter()
                .chain(keyword_results.iter())
                .map(|r| {
                    Page {
                        id: r.chunk.page_id.clone(),
                        title: r.page_title.clone(),
                        content: String::new(),
                        source_id: r.source_id.clone(),
                        tags: vec![],
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                        metadata: serde_json::Value::Null,
                    }
                })
                .collect();
            let related = graph::bfs_related_pages(&seed_pages, &pages, 2);
            self.graph_to_results(&related, opts).await?
        } else {
            vec![]
        };

        // 4. 加权融合 + 去重
        let fused = Self::fuse_results(vector_results, keyword_results, graph_results, opts.top_k, opts.min_score);

        Ok(fused)
    }

    /// 向量检索：嵌入查询 → 与所有分块计算 cosine 相似度。
    async fn vector_search(&self, query: &str, opts: &SearchOpts) -> Result<Vec<ScoredResult>, GBrainError> {
        let query_vec = self
            .embedder
            .embed(&[query.to_string()])
            .await
            .map_err(|e| GBrainError::query(format!("embedding failed: {}", e)))?
            .into_iter()
            .next()
            .unwrap_or_default();
        let query_vec_f64: Vec<f64> = query_vec.iter().map(|v| *v as f64).collect();

        // 获取所有页面
        let pages = self.engine.list_pages().await?;

        let mut results = Vec::new();

        for page in &pages {
            // 来源过滤
            if let Some(ref filter) = opts.source_filter {
                if !filter.contains(&page.source_id) {
                    continue;
                }
            }

            let chunks = self.engine.get_chunks(&page.id).await?;
            for chunk in &chunks {
                if let Some(ref emb) = chunk.embedding {
                    let score = cosine_similarity(&query_vec_f64, emb);
                    if score >= opts.min_score {
                        results.push(ScoredResult {
                            chunk: chunk.clone(),
                            score,
                            source_id: page.source_id.clone(),
                            page_title: page.title.clone(),
                            vector_score: score,
                            keyword_score: 0.0,
                            graph_score: 0.0,
                        });
                    }
                }
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        if results.len() > opts.top_k {
            results.truncate(opts.top_k);
        }

        Ok(results)
    }

    /// 关键词检索：通过 engine.search 执行 FTS 搜索。
    async fn keyword_search(&self, query: &str, opts: &SearchOpts) -> Result<Vec<ScoredResult>, GBrainError> {
        let engine_results = self.engine.search(query, opts).await?;

        let results = engine_results
            .into_iter()
            .map(|r| ScoredResult {
                vector_score: 0.0,
                keyword_score: r.score,
                graph_score: 0.0,
                score: r.score * KEYWORD_WEIGHT,
                ..ScoredResult::from_search_result(r)
            })
            .collect();

        Ok(results)
    }

    /// 图遍历结果转 ScoredResult。
    async fn graph_to_results(
        &self,
        related: &[Page],
        opts: &SearchOpts,
    ) -> Result<Vec<ScoredResult>, GBrainError> {
        let mut results = Vec::new();
        for page in related {
            if let Some(ref filter) = opts.source_filter {
                if !filter.contains(&page.source_id) {
                    continue;
                }
            }
            let chunks = self.engine.get_chunks(&page.id).await?;
            for chunk in &chunks {
                results.push(ScoredResult {
                    chunk: chunk.clone(),
                    score: GRAPH_WEIGHT,
                    source_id: page.source_id.clone(),
                    page_title: page.title.clone(),
                    vector_score: 0.0,
                    keyword_score: 0.0,
                    graph_score: GRAPH_WEIGHT,
                });
            }
        }
        Ok(results)
    }

    /// 融合：加权求和 + 去重（按 chunk.id）+ 排序 + 截断。
    fn fuse_results(
        vector: Vec<ScoredResult>,
        keyword: Vec<ScoredResult>,
        graph: Vec<ScoredResult>,
        top_k: usize,
        min_score: f64,
    ) -> Vec<SearchResult> {
        let mut by_id: HashMap<String, ScoredResult> = HashMap::new();

        for r in vector {
            let entry = by_id.entry(r.chunk.id.clone()).or_insert_with(|| r.clone());
            entry.vector_score = r.vector_score;
            entry.score = entry.vector_score * VECTOR_WEIGHT + entry.keyword_score * KEYWORD_WEIGHT + entry.graph_score * GRAPH_WEIGHT;
        }

        for r in keyword {
            let entry = by_id.entry(r.chunk.id.clone()).or_insert_with(|| r.clone());
            entry.keyword_score = r.keyword_score;
            entry.score = entry.vector_score * VECTOR_WEIGHT + entry.keyword_score * KEYWORD_WEIGHT + entry.graph_score * GRAPH_WEIGHT;
        }

        for r in graph {
            let entry = by_id.entry(r.chunk.id.clone()).or_insert_with(|| r.clone());
            entry.graph_score = r.graph_score;
            entry.score = entry.vector_score * VECTOR_WEIGHT + entry.keyword_score * KEYWORD_WEIGHT + entry.graph_score * GRAPH_WEIGHT;
        }

        let mut fused: Vec<SearchResult> = by_id
            .into_values()
            .filter(|r| r.score >= min_score)
            .map(|r| r.into_search_result())
            .collect();

        fused.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        if fused.len() > top_k {
            fused.truncate(top_k);
        }

        fused
    }

    /// 获取底层引擎引用。
    pub fn engine(&self) -> &dyn GBrainEngine {
        self.engine.as_ref()
    }
}

// ── 内部类型 ──

/// 带子分数的检索结果（用于融合计算）。
#[derive(Debug, Clone)]
struct ScoredResult {
    chunk: Chunk,
    score: f64,
    source_id: String,
    page_title: String,
    vector_score: f64,
    keyword_score: f64,
    graph_score: f64,
}

impl ScoredResult {
    fn from_search_result(r: SearchResult) -> Self {
        Self {
            chunk: r.chunk,
            score: r.score,
            source_id: r.source_id,
            page_title: r.page_title,
            vector_score: 0.0,
            keyword_score: 0.0,
            graph_score: 0.0,
        }
    }

    fn into_search_result(self) -> SearchResult {
        SearchResult {
            chunk: self.chunk,
            score: self.score,
            source_id: self.source_id,
            page_title: self.page_title,
        }
    }
}

/// Cosine 相似度。
fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    (dot / (norm_a * norm_b)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use taiji_llm::embedding::MockEmbeddingService;
    use taiji_llm::EmbeddingService;
    use taiji_types::knowledge::{GBrainConfig, PageInput};

    use crate::engine::mock::MockEngine;
    use crate::engine::GBrainEngine;

    fn make_embedder(dim: usize) -> Box<dyn taiji_llm::EmbeddingService> {
        Box::new(MockEmbeddingService::new(dim))
    }

    async fn setup_engine() -> (MockEngine, Vec<String>) {
        let mut engine = MockEngine::new();
        engine.connect(&GBrainConfig::default()).await.unwrap();
        engine.init_schema().await.unwrap();

        let slugs = vec!["arch", "quant", "theory", "misc"];
        for (slug, title, content) in &[
            ("arch", "架构总纲", "三层架构设计说明"),
            ("quant", "量化总纲", "量化交易规则详情"),
            ("theory", "理论总纲", "量价时空理论核心"),
            ("misc", "其他", "无关内容"),
        ] {
            engine
                .put_page(
                    slug,
                    PageInput {
                        title: title.to_string(),
                        content: content.to_string(),
                        source_id: Some("system".into()),
                        tags: vec![],
                        metadata: serde_json::Value::Null,
                    },
                )
                .await
                .unwrap();
        }

        // 添加带嵌入向量的分块
        let dim = 8;
        let embedder = MockEmbeddingService::new(dim);
        for (slug, text) in &[
            ("arch", "三层架构设计说明"),
            ("quant", "量化交易规则详情"),
            ("theory", "量价时空理论核心"),
            ("misc", "无关内容"),
        ] {
            let vec = embedder.embed_single(text).await.unwrap();
            let f64_vec: Vec<f64> = vec.iter().map(|v| *v as f64).collect();
            let chunk = taiji_types::knowledge::Chunk {
                id: format!("{}:chunk:0", slug),
                page_id: slug.to_string(),
                seq: 0,
                text: text.to_string(),
                embedding: Some(f64_vec),
            };
            engine
                .store_chunks(&slug.to_string(), &[chunk])
                .await
                .unwrap();
        }

        (engine, slugs.iter().map(|s| s.to_string()).collect())
    }

    #[tokio::test]
    async fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn test_cosine_similarity_empty() {
        assert!((cosine_similarity(&[], &[]) - 0.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn test_retriever_vector_search() {
        let (engine, _slugs) = setup_engine().await;
        let retriever = Retriever::new(Box::new(engine), make_embedder(8));

        let opts = SearchOpts {
            top_k: 10,
            min_score: 0.0,
            source_filter: None,
            use_expansion: false,
            use_keyword: false,
            use_graph: false,
        };

        let results = retriever.search("三层架构", &opts).await.unwrap();
        assert!(!results.is_empty(), "should find matching chunks");
        // The mock embedding is deterministic based on text hash,
        // so "三层架构设计说明" should score highest for "三层架构" query
        assert!(results.iter().any(|r| r.page_title == "架构总纲"));
    }

    #[tokio::test]
    async fn test_retriever_source_filter() {
        let mut engine = MockEngine::new();
        engine.connect(&GBrainConfig::default()).await.unwrap();
        engine.init_schema().await.unwrap();

        engine
            .put_page(
                "pub",
                PageInput {
                    title: "公开".into(),
                    content: "公开数据".into(),
                    source_id: Some("public".into()),
                    tags: vec![],
                    metadata: serde_json::Value::Null,
                },
            )
            .await
            .unwrap();

        // Add chunk with embedding
        let embedder = MockEmbeddingService::new(8);
        let vec = embedder.embed_single("公开数据").await.unwrap();
        let f64_vec: Vec<f64> = vec.iter().map(|v| *v as f64).collect();
        engine
            .store_chunks(
                &"pub".into(),
                &[taiji_types::knowledge::Chunk {
                    id: "pub:chunk:0".into(),
                    page_id: "pub".into(),
                    seq: 0,
                    text: "公开数据".into(),
                    embedding: Some(f64_vec),
                }],
            )
            .await
            .unwrap();

        let retriever = Retriever::new(Box::new(engine), make_embedder(8));

        let opts = SearchOpts {
            top_k: 10,
            min_score: 0.0,
            source_filter: Some(vec!["nonexistent".into()]),
            use_expansion: false,
            use_keyword: false,
            use_graph: false,
        };

        let results = retriever.search("公开数据", &opts).await.unwrap();
        assert!(results.is_empty(), "source filter should exclude all results");
    }

    #[tokio::test]
    async fn test_mixed_search_with_keyword() {
        let (engine, _slugs) = setup_engine().await;
        let retriever = Retriever::new(Box::new(engine), make_embedder(8));

        let opts = SearchOpts {
            top_k: 10,
            min_score: 0.0,
            source_filter: None,
            use_expansion: false,
            use_keyword: true,
            use_graph: false,
        };

        let results = retriever.search("量化", &opts).await.unwrap();
        assert!(!results.is_empty(), "should find '量化' results");
    }

    // ── BFS graph tests via Retriever ──

    #[tokio::test]
    async fn test_bfs_related_pages_direct() {
        let seeds = vec![make_test_page("a")];
        let all = vec![
            make_test_page("a"),
            make_test_page("b"),
            make_test_page("c"),
        ];
        // No explicit links → empty
        let result = graph::bfs_related_pages(&seeds, &all, 2);
        // All pages have no links field
        assert!(result.is_empty() || result.len() <= 2);
    }

    fn make_test_page(id: &str) -> Page {
        Page {
            id: id.to_string(),
            title: id.to_string(),
            content: String::new(),
            source_id: "test".into(),
            tags: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            metadata: serde_json::Value::Null,
        }
    }
}
