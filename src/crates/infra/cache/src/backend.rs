//! cache（速递符）— 缓存后端接口定义
//!
//! 来源: moka / DashMap 接口设计 (MIT)

use std::hash::Hash;
use std::time::Duration;

use async_trait::async_trait;

use crate::error::CacheError;
use crate::stats::CacheStats;

/// 缓存键必须满足的约束
pub trait CacheKey: Eq + Hash + Clone + Send + Sync + 'static {}
impl<T: Eq + Hash + Clone + Send + Sync + 'static> CacheKey for T {}

/// 缓存值必须满足的约束
pub trait CacheValue: Clone + Send + Sync + 'static {}
impl<T: Clone + Send + Sync + 'static> CacheValue for T {}

/// 统一缓存后端 trait（同步）
///
/// 支持 DashMap / quick_cache 等同步缓存库。
/// 设计为对象安全（无泛型方法），支持 `Box<dyn CacheBackend<K, V>>`。
pub trait CacheBackend<K, V>: Send + Sync
where
    K: CacheKey,
    V: CacheValue,
{
    /// 设置缓存条目（使用默认 TTL）
    fn set(&self, key: K, value: V) -> Result<(), CacheError>;

    /// 设置缓存条目（指定 TTL）
    fn set_with_ttl(&self, key: K, value: V, ttl: Duration) -> Result<(), CacheError>;

    /// 获取缓存条目
    fn get(&self, key: &K) -> Result<Option<V>, CacheError>;

    /// 批量获取
    fn get_many(&self, keys: &[K]) -> Result<Vec<Option<V>>, CacheError> {
        keys.iter().map(|k| self.get(k)).collect()
    }

    /// 删除缓存条目
    fn delete(&self, key: &K) -> Result<bool, CacheError>;

    /// 清空缓存
    fn clear(&self) -> Result<(), CacheError>;

    /// 检查是否存在
    fn contains(&self, key: &K) -> Result<bool, CacheError>;

    /// 获取条目数
    fn len(&self) -> Result<usize, CacheError>;

    /// 是否为空
    fn is_empty(&self) -> Result<bool, CacheError>;

    /// 获取缓存统计
    fn stats(&self) -> CacheStats;
}

/// 异步缓存后端 trait（用于 moka::future::Cache）
#[async_trait]
pub trait AsyncCacheBackend<K, V>: Send + Sync
where
    K: CacheKey,
    V: CacheValue,
{
    /// 设置缓存条目（使用默认 TTL）
    async fn set(&self, key: K, value: V) -> Result<(), CacheError>;

    /// 设置缓存条目（指定 TTL）
    async fn set_with_ttl(&self, key: K, value: V, ttl: Duration) -> Result<(), CacheError>;

    /// 获取缓存条目
    async fn get(&self, key: &K) -> Result<Option<V>, CacheError>;

    /// 批量获取
    async fn get_many(&self, keys: &[K]) -> Result<Vec<Option<V>>, CacheError> {
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            results.push(self.get(key).await?);
        }
        Ok(results)
    }

    /// 删除缓存条目
    async fn delete(&self, key: &K) -> Result<bool, CacheError>;

    /// 清空缓存
    async fn clear(&self) -> Result<(), CacheError>;

    /// 获取缓存统计
    async fn stats(&self) -> CacheStats;
}
