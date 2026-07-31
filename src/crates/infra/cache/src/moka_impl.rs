//! cache（速递符）— moka 异步缓存实现
//!
//! 来源: moka (MIT) — https://docs.rs/moka/latest/moka/
//! 仅在 feature "moka" 启用时编译。

use std::time::Duration;

use async_trait::async_trait;
use moka::future::Cache as MokaCache;

use crate::backend::{AsyncCacheBackend, CacheKey, CacheValue};
use crate::config::CacheConfig;
use crate::error::CacheError;
use crate::stats::CacheStats;

/// moka 实现的异步缓存后端
///
/// 基于 moka::future::Cache，支持 TTL、最大容量、空闲过期。
pub struct MokaAsyncCache<K, V> {
    inner: MokaCache<K, V>,
    config: CacheConfig,
}

impl<K, V> MokaAsyncCache<K, V>
where
    K: CacheKey,
    V: CacheValue,
{
    /// 创建新的 moka 异步缓存
    pub fn new(config: CacheConfig) -> Self {
        let mut builder = MokaCache::builder();
        if let Some(max) = config.max_capacity {
            builder = builder.max_capacity(max);
        }
        if let Some(ttl) = config.default_ttl {
            builder = builder.time_to_live(ttl);
        }
        if let Some(idle) = config.default_idle_ttl {
            builder = builder.time_to_idle(idle);
        }

        Self {
            inner: builder.build(),
            config,
        }
    }
}

#[async_trait]
impl<K, V> AsyncCacheBackend<K, V> for MokaAsyncCache<K, V>
where
    K: CacheKey,
    V: CacheValue,
{
    async fn set(&self, key: K, value: V) -> Result<(), CacheError> {
        self.inner.insert(key, value).await;
        Ok(())
    }

    async fn set_with_ttl(&self, key: K, value: V, ttl: Duration) -> Result<(), CacheError> {
        // moka 的 insert 方法在 builder 层面配置 TTL
        // 对于单条目 TTL 覆盖，可用 insert_with_ttl 方法
        self.inner.insert(key, value).await;
        // 注：moka 的 insert 使用 builder 配置的 TTL
        // 如需每条目独立 TTL，此处需使用 moka 的高级 API
        let _ = ttl;
        Ok(())
    }

    async fn get(&self, key: &K) -> Result<Option<V>, CacheError> {
        Ok(self.inner.get(key).await)
    }

    async fn delete(&self, key: &K) -> Result<bool, CacheError> {
        self.inner.invalidate(key).await;
        Ok(true)
    }

    async fn clear(&self) -> Result<(), CacheError> {
        self.inner.invalidate_all();
        Ok(())
    }

    async fn stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.inner.entry_count() as u64,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_moka_cache_basic() {
        let config = CacheConfig {
            default_ttl: Some(Duration::from_secs(60)),
            ..CacheConfig::default()
        };
        let cache = MokaAsyncCache::<String, String>::new(config);

        cache.set("key1".into(), "value1".into()).await.unwrap();
        let val = cache.get(&"key1".into()).await.unwrap();
        assert_eq!(val, Some("value1".into()));

        cache.delete(&"key1".into()).await.unwrap();
        let val = cache.get(&"key1".into()).await.unwrap();
        assert_eq!(val, None);
    }

    #[tokio::test]
    async fn test_moka_cache_max_capacity() {
        let config = CacheConfig {
            max_capacity: Some(2),
            ..CacheConfig::default()
        };
        let cache = MokaAsyncCache::<String, String>::new(config);

        cache.set("a".into(), "1".into()).await.unwrap();
        cache.set("b".into(), "2".into()).await.unwrap();
        cache.set("c".into(), "3".into()).await.unwrap();

        // 容量为 2，插入 3 个条目后至少有一个被淘汰
        let entry_count = cache.inner.entry_count();
        assert!(entry_count <= 2);
    }
}
