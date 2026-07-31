//! PGLite 引擎实现 — 基于 HashMap 的内存引擎模拟。
//!
//! 在 WASM PGLite 运行时就绪前，使用 HashMap 模拟 PGLite 的页面/分块存储。
//! 接口与 trait 签名完全一致，后续可替换为真正的 PGLite SQL 实现。

use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::RwLock;
use taiji_types::knowledge::{
    Chunk, GBrainConfig, Page, PageId, PageInput, SearchOpts, SearchResult,
};

use super::GBrainEngine;
use crate::error::GBrainError;

/// PGLite 引擎 — 内存模拟实现。
pub struct PGLiteEngine {
    pages: RwLock<HashMap<String, Page>>,
    chunks: RwLock<HashMap<PageId, Vec<Chunk>>>,
    connected: RwLock<bool>,
}

impl PGLiteEngine {
    pub fn new() -> Self {
        Self {
            pages: RwLock::new(HashMap::new()),
            chunks: RwLock::new(HashMap::new()),
            connected: RwLock::new(false),
        }
    }
}

impl Default for PGLiteEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GBrainEngine for PGLiteEngine {
    async fn connect(&mut self, _config: &GBrainConfig) -> Result<(), GBrainError> {
        let mut connected = self.connected.write().map_err(|e| {
            GBrainError::connection(format!("lock error: {}", e))
        })?;
        *connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), GBrainError> {
        let mut connected = self.connected.write().map_err(|e| {
            GBrainError::connection(format!("lock error: {}", e))
        })?;
        *connected = false;
        self.pages.write().map_err(|e| GBrainError::connection(format!("lock error: {}", e)))?.clear();
        self.chunks.write().map_err(|e| GBrainError::connection(format!("lock error: {}", e)))?.clear();
        Ok(())
    }

    async fn init_schema(&mut self) -> Result<(), GBrainError> {
        // 内存引擎无需建表，仅确认已连接
        let connected = self.connected.read().map_err(|e| {
            GBrainError::connection(format!("lock error: {}", e))
        })?;
        if !*connected {
            return Err(GBrainError::NotInitialized);
        }
        Ok(())
    }

    async fn get_page(&self, slug: &str) -> Result<Option<Page>, GBrainError> {
        let _ = self.ensure_connected()?;
        let pages = self.pages.read().map_err(|e| {
            GBrainError::query(format!("lock error: {}", e))
        })?;
        Ok(pages.get(slug).cloned())
    }

    async fn put_page(&self, slug: &str, input: PageInput) -> Result<(), GBrainError> {
        let _ = self.ensure_connected()?;
        let now = Utc::now();
        let mut pages = self.pages.write().map_err(|e| {
            GBrainError::query(format!("lock error: {}", e))
        })?;

        let page = if let Some(existing) = pages.get(slug) {
            // 更新已有页面
            Page {
                id: slug.to_string(),
                title: input.title,
                content: input.content,
                source_id: input.source_id.unwrap_or_else(|| existing.source_id.clone()),
                tags: input.tags,
                created_at: existing.created_at,
                updated_at: now,
                metadata: input.metadata,
            }
        } else {
            // 新建页面
            Page {
                id: slug.to_string(),
                title: input.title,
                content: input.content,
                source_id: input.source_id.unwrap_or_default(),
                tags: input.tags,
                created_at: now,
                updated_at: now,
                metadata: input.metadata,
            }
        };
        pages.insert(slug.to_string(), page);
        Ok(())
    }

    async fn delete_page(&self, slug: &str) -> Result<bool, GBrainError> {
        let _ = self.ensure_connected()?;
        let mut pages = self.pages.write().map_err(|e| {
            GBrainError::query(format!("lock error: {}", e))
        })?;
        let existed = pages.remove(slug).is_some();
        // 同时删除关联分块
        let mut chunks = self.chunks.write().map_err(|e| {
            GBrainError::query(format!("lock error: {}", e))
        })?;
        chunks.remove(slug);
        Ok(existed)
    }

    async fn list_pages(&self) -> Result<Vec<Page>, GBrainError> {
        let _ = self.ensure_connected()?;
        let pages = self.pages.read().map_err(|e| {
            GBrainError::query(format!("lock error: {}", e))
        })?;
        let mut result: Vec<Page> = pages.values().cloned().collect();
        result.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
        Ok(result)
    }

    async fn page_count(&self) -> Result<usize, GBrainError> {
        let _ = self.ensure_connected()?;
        let pages = self.pages.read().map_err(|e| {
            GBrainError::query(format!("lock error: {}", e))
        })?;
        Ok(pages.len())
    }

    async fn search(
        &self,
        query: &str,
        opts: &SearchOpts,
    ) -> Result<Vec<SearchResult>, GBrainError> {
        let _ = self.ensure_connected()?;
        let pages = self.pages.read().map_err(|e| {
            GBrainError::query(format!("lock error: {}", e))
        })?;
        let query_lower = query.to_lowercase();

        let mut results: Vec<SearchResult> = pages
            .values()
            .filter(|page| {
                // 来源过滤
                if let Some(ref filter) = opts.source_filter {
                    if !filter.contains(&page.source_id) {
                        return false;
                    }
                }
                // 关键词匹配（简化 BM25 模拟）
                query_lower.is_empty()
                    || page.title.to_lowercase().contains(&query_lower)
                    || page.content.to_lowercase().contains(&query_lower)
            })
            .map(|page| {
                let score = if page.title.to_lowercase().contains(&query_lower) {
                    0.95
                } else if page.content.to_lowercase().contains(&query_lower) {
                    0.75
                } else {
                    0.5
                };
                SearchResult {
                    chunk: Chunk {
                        id: format!("chunk:{}:0", page.id),
                        page_id: page.id.clone(),
                        seq: 0,
                        text: page.content.clone(),
                        embedding: None,
                    },
                    score,
                    source_id: page.source_id.clone(),
                    page_title: page.title.clone(),
                }
            })
            .collect();

        // 按分数降序排列
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // 截取 top_k
        results.truncate(opts.top_k);

        Ok(results)
    }

    async fn get_chunks(&self, page_id: &PageId) -> Result<Vec<Chunk>, GBrainError> {
        let _ = self.ensure_connected()?;
        let chunks = self.chunks.read().map_err(|e| {
            GBrainError::query(format!("lock error: {}", e))
        })?;
        Ok(chunks.get(page_id).cloned().unwrap_or_default())
    }

    async fn store_chunks(
        &self,
        page_id: &PageId,
        chunks: &[Chunk],
    ) -> Result<(), GBrainError> {
        let _ = self.ensure_connected()?;
        let mut store = self.chunks.write().map_err(|e| {
            GBrainError::query(format!("lock error: {}", e))
        })?;
        store.insert(page_id.clone(), chunks.to_vec());
        Ok(())
    }
}

impl PGLiteEngine {
    fn ensure_connected(&self) -> Result<(), GBrainError> {
        let connected = self.connected.read().map_err(|e| {
            GBrainError::connection(format!("lock error: {}", e))
        })?;
        if !*connected {
            return Err(GBrainError::NotInitialized);
        }
        Ok(())
    }
}
