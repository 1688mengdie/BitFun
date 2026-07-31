//! cache（速递符）— 高频缓存加速模块
//!
//! 提供统一缓存的 CacheBackend trait，支持多种后端实现：
//! - DashMap：高并发读多写少的纯内存 KV 缓存
//! - moka：TTL/LRU 生产级缓存（feature-gated）
//!
//! 来源: dashmap (MIT) / moka (MIT)

pub mod backend;
pub mod config;
pub mod stats;
pub mod error;
pub mod dashmap;
pub mod manager;

#[cfg(feature = "moka")]
pub mod moka_impl;

pub use backend::{CacheBackend, AsyncCacheBackend, CacheKey, CacheValue};
pub use config::{CacheConfig, CacheBackendKind};
pub use stats::CacheStats;
pub use error::CacheError;
pub use dashmap::DashMapCache;
pub use manager::CacheManager;

#[cfg(feature = "moka")]
pub use moka_impl::MokaAsyncCache;
