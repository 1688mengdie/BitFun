//! cache（速递符）— DashMap 实现
//!
//! 来源: dashmap (MIT) — https://docs.rs/dashmap/latest/dashmap/

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap as InnerDashMap;

use crate::backend::{CacheBackend, CacheKey, CacheValue};
use crate::config::CacheConfig;
use crate::error::CacheError;
use crate::stats::CacheStats;

/// 缓存条目（含过期时间）
#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    expires_at: Option<Instant>,
}

/// DashMap 实现的缓存后端
///
/// 基于 dashmap 实现的高并发读缓存，支持 TTL 过期。
pub struct DashMapCache<K, V> {
    inner: Arc<InnerDashMap<K, CacheEntry<V>>>,
    config: CacheConfig,
    stats: Arc<CacheStatsAtomic>,
}

/// 原子缓存统计（线程安全）
#[derive(Debug, Default)]
struct CacheStatsAtomic {
    hits: AtomicU64,
    misses: AtomicU64,
    inserts: AtomicU64,
    evictions: AtomicU64,
    lookups: AtomicU64,
}

impl CacheStatsAtomic {
    fn inc_hits(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }
    fn inc_misses(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }
    fn inc_inserts(&self) {
        self.inserts.fetch_add(1, Ordering::Relaxed);
    }
    fn inc_evictions(&self) {
        self.evictions.fetch_add(1, Ordering::Relaxed);
    }
    fn inc_lookups(&self) {
        self.lookups.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> CacheStats {
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            inserts: self.inserts.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            lookups: self.lookups.load(Ordering::Relaxed),
            entry_count: 0,
        }
    }
}

impl<K, V> DashMapCache<K, V>
where
    K: CacheKey,
    V: CacheValue,
{
    /// 创建新的 DashMap 缓存
    pub fn new(config: CacheConfig) -> Self {
        Self {
            inner: Arc::new(InnerDashMap::new()),
            stats: Arc::new(CacheStatsAtomic::default()),
            config,
        }
    }

    /// 检查条目是否已过期
    fn is_expired(entry: &CacheEntry<V>) -> bool {
        entry
            .expires_at
            .is_some_and(|exp| Instant::now() > exp)
    }

    /// 获取或插入（若不存在则计算）
    pub fn get_or_insert<F>(&self, key: K, f: F) -> Result<V, CacheError>
    where
        F: FnOnce() -> V,
    {
        self.stats.inc_lookups();

        // 先检查是否已存在且未过期
        if let Some(entry) = self.inner.get(&key) {
            if !Self::is_expired(&entry) {
                self.stats.inc_hits();
                return Ok(entry.value.clone());
            }
            // 过期，移除
            drop(entry);
            self.inner.remove(&key);
            self.stats.inc_evictions();
        } else {
            self.stats.inc_misses();
        }

        // 计算并插入
        let value = f();
        self.inner.insert(key, CacheEntry {
            value: value.clone(),
            expires_at: self.config.default_ttl.map(|ttl| Instant::now() + ttl),
        });
        self.stats.inc_inserts();
        Ok(value)
    }
}

impl<K, V> CacheBackend<K, V> for DashMapCache<K, V>
where
    K: CacheKey,
    V: CacheValue,
{
    fn set(&self, key: K, value: V) -> Result<(), CacheError> {
        let ttl = self.config.default_ttl;
        self.set_with_ttl(key, value, ttl.unwrap_or(Duration::from_secs(300)))
    }

    fn set_with_ttl(&self, key: K, value: V, ttl: Duration) -> Result<(), CacheError> {
        let entry = CacheEntry {
            value,
            expires_at: Some(Instant::now() + ttl),
        };
        self.inner.insert(key, entry);
        self.stats.inc_inserts();
        Ok(())
    }

    fn get(&self, key: &K) -> Result<Option<V>, CacheError> {
        self.stats.inc_lookups();
        if let Some(entry) = self.inner.get(key) {
            if Self::is_expired(&entry) {
                drop(entry);
                self.inner.remove(key);
                self.stats.inc_misses();
                return Ok(None);
            }
            self.stats.inc_hits();
            Ok(Some(entry.value.clone()))
        } else {
            self.stats.inc_misses();
            Ok(None)
        }
    }

    fn delete(&self, key: &K) -> Result<bool, CacheError> {
        Ok(self.inner.remove(key).is_some())
    }

    fn contains(&self, key: &K) -> Result<bool, CacheError> {
        Ok(self.inner.contains_key(key))
    }

    fn clear(&self) -> Result<(), CacheError> {
        self.inner.clear();
        Ok(())
    }

    fn len(&self) -> Result<usize, CacheError> {
        Ok(self.inner.len())
    }

    fn is_empty(&self) -> Result<bool, CacheError> {
        Ok(self.inner.is_empty())
    }

    fn stats(&self) -> CacheStats {
        self.stats.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashmap_cache_basic() {
        let cache = DashMapCache::<String, String>::new(CacheConfig::default());

        cache.set("key1".into(), "value1".into()).unwrap();
        assert_eq!(
            cache.get(&"key1".into()).unwrap(),
            Some("value1".into())
        );

        cache.delete(&"key1".into()).unwrap();
        assert_eq!(cache.get(&"key1".into()).unwrap(), None);
    }

    #[test]
    fn test_cache_stats() {
        let cache = DashMapCache::<String, String>::new(CacheConfig::default());

        cache.get(&"missing".into()).unwrap(); // 未命中
        cache.set("a".into(), "1".into()).unwrap();
        cache.get(&"a".into()).unwrap(); // 命中

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.inserts, 1);
    }

    #[test]
    fn test_cache_ttl_expiry() {
        let config = CacheConfig {
            default_ttl: Some(Duration::from_millis(10)),
            ..CacheConfig::default()
        };
        let cache = DashMapCache::<String, String>::new(config);

        cache.set("key".into(), "val".into()).unwrap();
        assert_eq!(cache.get(&"key".into()).unwrap(), Some("val".into()));

        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(cache.get(&"key".into()).unwrap(), None);
    }

    #[test]
    fn test_cache_clear() {
        let cache = DashMapCache::<String, String>::new(CacheConfig::default());

        cache.set("a".into(), "1".into()).unwrap();
        cache.set("b".into(), "2".into()).unwrap();
        assert_eq!(cache.len().unwrap(), 2);

        cache.clear().unwrap();
        assert_eq!(cache.len().unwrap(), 0);
        assert!(cache.is_empty().unwrap());
    }

    #[test]
    fn test_cache_contains() {
        let cache = DashMapCache::<String, String>::new(CacheConfig::default());

        assert!(!cache.contains(&"key".into()).unwrap());
        cache.set("key".into(), "val".into()).unwrap();
        assert!(cache.contains(&"key".into()).unwrap());
    }

    #[test]
    fn test_get_or_insert() {
        let cache = DashMapCache::<String, String>::new(CacheConfig::default());

        // 第一次：未命中，通过闭包计算
        let val = cache.get_or_insert("counter".into(), || "computed".into()).unwrap();
        assert_eq!(val, "computed");

        // 第二次：命中
        let val = cache.get_or_insert("counter".into(), || "should_not_run".into()).unwrap();
        assert_eq!(val, "computed");
    }
}
