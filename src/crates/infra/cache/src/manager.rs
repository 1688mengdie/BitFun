//! cache（速递符）— 多级缓存管理器
//!
//! L1: 内存缓存（速递符）
//! L2: 数据库缓存（灵脉 — 可选 fallback）
//!
//! 来源: 多级缓存设计模式

use async_trait::async_trait;

use crate::backend::{CacheBackend, CacheKey, CacheValue};
use crate::error::CacheError;
use crate::stats::CacheStats;

/// 缓存加载器（L2 fallback）
#[async_trait]
pub trait CacheLoader<K, V>: Send + Sync {
    /// 从 L2 加载数据
    async fn load(&self, key: &K) -> Result<Option<V>, CacheError>;

    /// 持久化到 L2
    async fn persist(&self, key: &K, value: &V) -> Result<(), CacheError>;
}

/// 多级缓存管理器
///
/// L1: 内存缓存（速递符）— 毫秒级热点数据
/// L2: 数据库缓存（灵脉）— 可选 fallback 层
pub struct CacheManager<K, V> {
    /// L1 内存缓存
    l1: Box<dyn CacheBackend<K, V>>,
    /// L2 数据库回调（可选）
    l2: Option<Box<dyn CacheLoader<K, V>>>,
}

impl<K, V> CacheManager<K, V>
where
    K: CacheKey,
    V: CacheValue,
{
    /// 创建仅 L1 的缓存管理器
    pub fn new(l1: Box<dyn CacheBackend<K, V>>) -> Self {
        Self { l1, l2: None }
    }

    /// 创建带 L2 fallback 的缓存管理器
    pub fn with_l2(
        l1: Box<dyn CacheBackend<K, V>>,
        l2: Box<dyn CacheLoader<K, V>>,
    ) -> Self {
        Self { l1, l2: Some(l2) }
    }

    /// 读取（L1 → L2 fallback）
    pub async fn get(&self, key: &K) -> Result<Option<V>, CacheError> {
        // 先查 L1
        if let Some(value) = self.l1.get(key)? {
            return Ok(Some(value));
        }

        // L1 未命中，查 L2
        if let Some(loader) = &self.l2 {
            if let Some(value) = loader.load(key).await? {
                // 回填 L1
                self.l1.set(key.clone(), value.clone())?;
                return Ok(Some(value));
            }
        }

        Ok(None)
    }

    /// 读取 L1 统计
    pub fn stats(&self) -> CacheStats {
        self.l1.stats()
    }

    /// 清空 L1
    pub fn clear(&self) -> Result<(), CacheError> {
        self.l1.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashmap::DashMapCache;
    use crate::config::CacheConfig;

    struct MockLoader;
    #[async_trait]
    impl CacheLoader<String, String> for MockLoader {
        async fn load(&self, key: &String) -> Result<Option<String>, CacheError> {
            if key == "db_key" {
                Ok(Some("db_value".into()))
            } else {
                Ok(None)
            }
        }

        async fn persist(&self, _key: &String, _value: &String) -> Result<(), CacheError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_cache_manager_l1_only() {
        let l1 = Box::new(DashMapCache::<String, String>::new(CacheConfig::default()));
        let manager = CacheManager::new(l1);

        // L1 有值
        manager.l1.set("key".into(), "val".into()).unwrap();
        assert_eq!(
            manager.get(&"key".into()).await.unwrap(),
            Some("val".into())
        );

        // L1 无值
        assert_eq!(manager.get(&"missing".into()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_cache_manager_with_l2() {
        let l1 = Box::new(DashMapCache::<String, String>::new(CacheConfig::default()));
        let l2 = Box::new(MockLoader);
        let manager = CacheManager::with_l2(l1, l2);

        // L1 未命中，L2 命中 → 回填 L1
        let val = manager.get(&"db_key".into()).await.unwrap();
        assert_eq!(val, Some("db_value".into()));

        // L1 现在应有值
        let val = manager.get(&"db_key".into()).await.unwrap();
        assert_eq!(val, Some("db_value".into()));
    }
}
