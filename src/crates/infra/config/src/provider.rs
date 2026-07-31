//! 天书阁 — 配置提供者体系。
//!
//! 各模块通过 `ConfigProvider` 注册自己的默认配置、校验逻辑和变更回调。
//!
//! 设计参考：BitFun `crates/assembly/core/src/service/config/providers.rs:26-100`
//! ConfigProvider 提供者注册模式，Rust 翻译实现，非 Cargo 依赖。

use std::collections::HashMap;
use serde_json::Value;
use async_trait::async_trait;

use crate::error::ConfigResult;

/// 配置提供者——模块级配置的注册单元。
///
/// 每个模块实现此 trait，在 ConfigManager 初始化时注册。
#[async_trait]
pub trait ConfigProvider: Send + Sync {
    /// 提供者名称（如 `"monitor"`、`"harness"`）。
    fn name(&self) -> &str;

    /// 返回此模块的默认配置（子集，在合并时叠加到全局默认值）。
    fn get_default_config(&self) -> Value;

    /// 校验配置子集是否合法。
    async fn validate_config(&self, config: &Value) -> ConfigResult<Vec<String>>;

    /// 配置变更回调——当此模块的配置路径发生变化时调用。
    async fn on_config_changed(&self, old: &Value, new: &Value) -> ConfigResult<()>;
}

/// 配置提供者注册表。
pub struct ConfigProviderRegistry {
    providers: HashMap<String, Box<dyn ConfigProvider>>,
}

impl ConfigProviderRegistry {
    /// 创建空的注册表。
    pub fn new() -> Self {
        Self { providers: HashMap::new() }
    }

    /// 注册提供者。
    pub fn register(&mut self, provider: Box<dyn ConfigProvider>) {
        let name = provider.name().to_string();
        self.providers.insert(name, provider);
    }

    /// 获取提供者。
    pub fn get(&self, name: &str) -> Option<&dyn ConfigProvider> {
        self.providers.get(name).map(|v| &**v)
    }

    /// 遍历所有提供者。
    pub fn iter(&self) -> impl Iterator<Item = &Box<dyn ConfigProvider>> {
        self.providers.values()
    }

    /// 提供者数量。
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// 汇总所有提供者的默认配置（合并为一个 Value Object）。
    pub fn aggregate_defaults(&self) -> Value {
        let mut map = serde_json::Map::new();
        for provider in self.providers.values() {
            let defaults = provider.get_default_config();
            if let Value::Object(sub) = defaults {
                for (k, v) in sub {
                    map.insert(k, v);
                }
            }
        }
        Value::Object(map)
    }

    /// 调用所有提供者的 validate_config，汇总警告。
    pub async fn validate_all(&self, config: &Value) -> Vec<String> {
        let mut warnings = Vec::new();
        for provider in self.providers.values() {
            if let Ok(mut w) = provider.validate_config(config).await {
                warnings.append(&mut w);
            }
        }
        warnings
    }

    /// 通知所有提供者配置变更。
    pub async fn notify_all(&self, old: &Value, new: &Value) {
        for provider in self.providers.values() {
            let _ = provider.on_config_changed(old, new).await;
        }
    }
}

impl Default for ConfigProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
