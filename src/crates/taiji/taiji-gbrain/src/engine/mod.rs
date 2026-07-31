//! GBrainEngine trait — gbrain 引擎抽象层。
//!
//! 屏蔽 PGLite/Postgres/内存 后端差异，提供统一的知识库操作接口。
//! 参考: gbrain (MIT) engine.ts:659+ BrainEngine 接口。

pub mod pglite;
pub mod mock;

use async_trait::async_trait;
use taiji_types::knowledge::{Chunk, GBrainConfig, Page, PageId, PageInput, SearchOpts, SearchResult};

use crate::error::GBrainError;

/// gbrain 引擎 trait — 11 方法全覆盖。
#[async_trait]
pub trait GBrainEngine: Send + Sync {
    // ── 生命周期 ──

    /// 连接引擎。connect() 调用后方可执行其他操作。
    async fn connect(&mut self, config: &GBrainConfig) -> Result<(), GBrainError>;

    /// 断开连接。释放资源。
    async fn disconnect(&mut self) -> Result<(), GBrainError>;

    /// 初始化数据库 schema（建表等）。
    async fn init_schema(&mut self) -> Result<(), GBrainError>;

    // ── 页面 CRUD ──

    /// 获取页面。不存在时返回 Ok(None)。
    async fn get_page(&self, slug: &str) -> Result<Option<Page>, GBrainError>;

    /// 创建或更新页面。
    async fn put_page(&self, slug: &str, input: PageInput) -> Result<(), GBrainError>;

    /// 删除页面。返回是否实际删除了页面。
    async fn delete_page(&self, slug: &str) -> Result<bool, GBrainError>;

    /// 列出所有页面。
    async fn list_pages(&self) -> Result<Vec<Page>, GBrainError>;

    /// 页面总数。
    async fn page_count(&self) -> Result<usize, GBrainError>;

    // ── 搜索 ──

    /// 搜索。返回按相关性降序排列的搜索结果。
    async fn search(
        &self,
        query: &str,
        opts: &SearchOpts,
    ) -> Result<Vec<SearchResult>, GBrainError>;

    // ── 分块操作 ──

    /// 获取页面的全部分块。
    async fn get_chunks(&self, page_id: &PageId) -> Result<Vec<Chunk>, GBrainError>;

    /// 存储页面的全部分块（覆盖式）。
    async fn store_chunks(
        &self,
        page_id: &PageId,
        chunks: &[Chunk],
    ) -> Result<(), GBrainError>;
}
