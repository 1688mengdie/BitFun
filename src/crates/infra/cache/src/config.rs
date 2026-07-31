//! cache（速递符）— 缓存配置
//!
//! 来源: moka::CacheBuilder 配置模式 (MIT)

use std::time::Duration;

/// 缓存后端类型
///
/// 注意: `QuickCache` 变体当前未实现，预留供后续添加。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum CacheBackendKind {
    /// DashMap（并发 HashMap，无淘汰）
    DashMap,
    /// moka（TTL/LRU，同步）
    Moka,
    /// moka（TTL/LRU，异步）
    MokaAsync,
    /// quick_cache（LRU）— 尚未实现，预留
    QuickCache,
}

/// 缓存配置
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// 缓存后端类型
    pub backend: CacheBackendKind,
    /// 最大容量（条目数）
    pub max_capacity: Option<u64>,
    /// 默认 TTL
    pub default_ttl: Option<Duration>,
    /// 默认空闲过期时间
    pub default_idle_ttl: Option<Duration>,
    /// 是否启用统计
    pub enable_stats: bool,
    /// 缓存名称（用于监控）
    pub name: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            backend: CacheBackendKind::DashMap,
            max_capacity: None,
            default_ttl: None,
            default_idle_ttl: None,
            enable_stats: true,
            name: String::new(),
        }
    }
}
