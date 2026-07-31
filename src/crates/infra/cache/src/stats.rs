//! cache（速递符）— 缓存命中统计
//!
//! 来源: moka CacheStats API (MIT)

use serde::Serialize;

/// 缓存命中统计
#[derive(Debug, Clone, Default, Serialize)]
pub struct CacheStats {
    /// 命中次数
    pub hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// 当前条目数
    pub entry_count: u64,
    /// 插入次数
    pub inserts: u64,
    /// 淘汰次数（过期/LRU）
    pub evictions: u64,
    /// 总查找次数
    pub lookups: u64,
}

impl CacheStats {
    /// 命中率
    pub fn hit_rate(&self) -> f64 {
        if self.lookups == 0 {
            0.0
        } else {
            self.hits as f64 / self.lookups as f64
        }
    }

    /// 未命中率
    pub fn miss_rate(&self) -> f64 {
        if self.lookups == 0 {
            0.0
        } else {
            self.misses as f64 / self.lookups as f64
        }
    }
}
