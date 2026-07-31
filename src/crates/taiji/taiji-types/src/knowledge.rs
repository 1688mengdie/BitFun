//! gbrain 共享类型 — 知识库页面、分块、搜索与引擎配置。
//!
//! 参考: gbrain (MIT) types.ts 类型体系
//!       gbrain config.ts:28-399 (MIT) 配置结构
//!       gbrain chunkers/ (MIT) 分块策略

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================
// 标识符
// ============================================================

/// 页面标识（slug）。
pub type PageId = String;

/// 来源标识（用户/团队/系统隔离）。
pub type SourceId = String;

// ============================================================
// 页面
// ============================================================

/// 知识库页面。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub id: PageId,
    pub title: String,
    pub content: String,
    pub source_id: SourceId,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

/// 创建/更新页面请求。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageInput {
    pub title: String,
    pub content: String,
    pub source_id: Option<SourceId>,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
}

// ============================================================
// 分块
// ============================================================

/// 分块策略。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ChunkStrategy {
    /// 固定字符数分块。
    Fixed {
        chunk_size: usize,
        overlap: usize,
    },
    /// 按段落（\n\n）分块。
    Paragraph,
    /// 按句子（. ! ?）分块。
    Sentence,
}

impl Default for ChunkStrategy {
    fn default() -> Self {
        Self::Fixed {
            chunk_size: 512,
            overlap: 64,
        }
    }
}

/// 文本分块 — 含嵌入向量。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub page_id: PageId,
    pub seq: usize,
    pub text: String,
    pub embedding: Option<Vec<f64>>,
}

// ============================================================
// 搜索
// ============================================================

/// 单个搜索结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub chunk: Chunk,
    pub score: f64,
    pub source_id: SourceId,
    pub page_title: String,
}

/// 搜索选项。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchOpts {
    pub top_k: usize,
    pub min_score: f64,
    pub source_filter: Option<Vec<SourceId>>,
    pub use_expansion: bool,
    pub use_keyword: bool,
    pub use_graph: bool,
}

impl Default for SearchOpts {
    fn default() -> Self {
        Self {
            top_k: 10,
            min_score: 0.0,
            source_filter: None,
            use_expansion: true,
            use_keyword: true,
            use_graph: false,
        }
    }
}

// ============================================================
// 引擎配置
// ============================================================

/// gbrain 引擎配置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GBrainConfig {
    pub engine: String,       // "pglite" | "postgres"
    pub database_url: Option<String>,
    pub database_path: Option<String>,
    pub embedding_model: Option<String>,
    pub embedding_dimensions: usize,
}

impl Default for GBrainConfig {
    fn default() -> Self {
        Self {
            engine: "pglite".into(),
            database_url: None,
            database_path: None,
            embedding_model: None,
            embedding_dimensions: 384,
        }
    }
}
