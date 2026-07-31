//! MockEngine — 用于测试的模拟引擎。

use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::RwLock;
use taiji_types::knowledge::{
    Chunk, GBrainConfig, Page, PageId, PageInput, SearchOpts, SearchResult,
};

use super::GBrainEngine;
use crate::error::GBrainError;

/// 模拟引擎 — 纯内存实现，专用于单元测试。
pub struct MockEngine {
    pages: RwLock<HashMap<String, Page>>,
    chunks: RwLock<HashMap<String, Vec<Chunk>>>,
    connected: RwLock<bool>,
}

impl MockEngine {
    pub fn new() -> Self {
        Self {
            pages: RwLock::new(HashMap::new()),
            chunks: RwLock::new(HashMap::new()),
            connected: RwLock::new(false),
        }
    }
}

impl Default for MockEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GBrainEngine for MockEngine {
    async fn connect(&mut self, _config: &GBrainConfig) -> Result<(), GBrainError> {
        *self.connected.write().map_err(|e| GBrainError::connection(e.to_string()))? = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), GBrainError> {
        *self.connected.write().map_err(|e| GBrainError::connection(e.to_string()))? = false;
        self.pages.write().map_err(|e| GBrainError::connection(e.to_string()))?.clear();
        Ok(())
    }

    async fn init_schema(&mut self) -> Result<(), GBrainError> {
        if !*self.connected.read().map_err(|e| GBrainError::connection(e.to_string()))? {
            return Err(GBrainError::NotInitialized);
        }
        Ok(())
    }

    async fn get_page(&self, slug: &str) -> Result<Option<Page>, GBrainError> {
        if !*self.connected.read().map_err(|e| GBrainError::connection(e.to_string()))? {
            return Err(GBrainError::NotInitialized);
        }
        let pages = self.pages.read().map_err(|e| GBrainError::query(e.to_string()))?;
        Ok(pages.get(slug).cloned())
    }

    async fn put_page(&self, slug: &str, input: PageInput) -> Result<(), GBrainError> {
        let now = Utc::now();
        let mut pages = self.pages.write().map_err(|e| GBrainError::query(e.to_string()))?;
        let page = if let Some(existing) = pages.get(slug) {
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
        let mut pages = self.pages.write().map_err(|e| GBrainError::query(e.to_string()))?;
        Ok(pages.remove(slug).is_some())
    }

    async fn list_pages(&self) -> Result<Vec<Page>, GBrainError> {
        let pages = self.pages.read().map_err(|e| GBrainError::query(e.to_string()))?;
        let mut result: Vec<Page> = pages.values().cloned().collect();
        result.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
        Ok(result)
    }

    async fn page_count(&self) -> Result<usize, GBrainError> {
        if !*self.connected.read().map_err(|e| GBrainError::connection(e.to_string()))? {
            return Err(GBrainError::NotInitialized);
        }
        let pages = self.pages.read().map_err(|e| GBrainError::query(e.to_string()))?;
        Ok(pages.len())
    }

    async fn search(&self, _query: &str, _opts: &SearchOpts) -> Result<Vec<SearchResult>, GBrainError> {
        Ok(vec![])
    }

    async fn get_chunks(&self, page_id: &PageId) -> Result<Vec<Chunk>, GBrainError> {
        let map = self.chunks.read().map_err(|e| GBrainError::query(e.to_string()))?;
        Ok(map.get(page_id).cloned().unwrap_or_default())
    }

    async fn store_chunks(&self, page_id: &PageId, chunks: &[Chunk]) -> Result<(), GBrainError> {
        let mut map = self.chunks.write().map_err(|e| GBrainError::query(e.to_string()))?;
        map.insert(page_id.clone(), chunks.to_vec());
        Ok(())
    }
}
