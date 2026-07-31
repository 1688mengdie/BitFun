//! 天书阁 — 三平面配置加载器 + 完整实现。
//!
//! 按优先级 env > file > defaults 加载、合并配置，支持运行时热更新。
//!
//! 设计参考：gbrain `src/core/config.ts:451-895` 加载器优先级模式，使用标准 crate
//! toml + dirs 实现，Rust 翻译实现，非 Cargo 依赖。

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::{broadcast, RwLock};

use crate::error::{ConfigError, ConfigResult};
use crate::event::{ConfigChangeEvent, CONFIG_BROADCAST_CAPACITY};
use crate::keys::{default_config_from_keys, validate_known_keys, warn_unknown_keys};
use crate::manager::ConfigManager;
use crate::plane::ConfigPlane;
use crate::provider::ConfigProviderRegistry;

/// 配置目录名称。
const CONFIG_DIR_NAME: &str = ".taiji-quant";
/// 配置文件名。
const CONFIG_FILE_NAME: &str = "config.toml";
/// 环境变量前缀。
const ENV_PREFIX: &str = "LVPA_";

/// LVPA 配置管理器实现。
pub struct LvpaConfigManager {
    /// 内部状态（RwLock 保护）。
    inner: Arc<RwLock<Inner>>,
    /// 配置变更广播发送端。
    tx: broadcast::Sender<ConfigChangeEvent>,
    /// 模块提供者注册表。
    providers: ConfigProviderRegistry,
    /// 配置文件路径。
    config_path: PathBuf,
}

struct Inner {
    /// 按平面存储的原始配置值。
    planes: std::collections::HashMap<ConfigPlane, Value>,
    /// 合并后的全量配置。
    merged: Value,
}

impl LvpaConfigManager {
    /// 创建新的配置管理器（使用默认路径）。
    pub fn new() -> Self {
        let config_dir = Self::default_config_dir();
        let config_path = config_dir.join(CONFIG_FILE_NAME);
        Self::with_path(config_path)
    }

    /// 创建指定配置文件路径的配置管理器。
    pub fn with_path(config_path: PathBuf) -> Self {
        let (tx, _) = broadcast::channel(CONFIG_BROADCAST_CAPACITY);
        let defaults = default_config_from_keys();
        let mut planes = std::collections::HashMap::new();
        planes.insert(ConfigPlane::Default, defaults.clone());

        Self {
            inner: Arc::new(RwLock::new(Inner {
                planes,
                merged: defaults,
            })),
            tx,
            providers: ConfigProviderRegistry::new(),
            config_path,
        }
    }

    /// 获取默认配置目录：`~/.taiji-quant/`。
    pub fn default_config_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(CONFIG_DIR_NAME)
    }

    /// 获取配置文件路径。
    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    /// 注册模块配置提供者。
    pub fn register_provider(&mut self, provider: Box<dyn crate::provider::ConfigProvider>) {
        self.providers.register(provider);
    }

    /// 获取提供者注册表引用。
    pub fn providers(&self) -> &ConfigProviderRegistry {
        &self.providers
    }

    /// 深度合并两个 JSON Value（override 覆盖 base）。
    fn deep_merge(base: &Value, override_val: &Value) -> Value {
        match (base, override_val) {
            (Value::Object(base_map), Value::Object(override_map)) => {
                let mut merged = base_map.clone();
                for (k, v) in override_map {
                    let new_val = if let Some(base_v) = merged.get(k) {
                        Self::deep_merge(base_v, v)
                    } else {
                        v.clone()
                    };
                    merged.insert(k.clone(), new_val);
                }
                Value::Object(merged)
            }
            _ => override_val.clone(),
        }
    }

    /// 从配置文件加载（TOML 格式）。
    fn load_file_config(path: &PathBuf) -> Value {
        if !path.exists() {
            return json!({});
        }
        match std::fs::read_to_string(path) {
            Ok(content) => {
                match content.parse::<toml::Value>() {
                    Ok(toml_val) => {
                        // toml::Value → serde_json::Value
                        serde_json::to_value(toml_val).unwrap_or_default()
                    }
                    Err(e) => {
                        // TOML 解析失败时返回空，不阻塞启动
                        eprintln!("[taiji-config] TOML 解析失败 ({}): {}", path.display(), e);
                        json!({})
                    }
                }
            }
            Err(_) => json!({}),
        }
    }

    /// 从环境变量加载配置。
    ///
    /// 将 `LVPA_MONITOR_LEVEL` → `monitor.level`，
    /// `LVPA_HARNESS_MODE` → `harness.mode`。
    /// 值类型推断：数字优先，然后 bool，然后字符串。
    fn load_env_config() -> Value {
        let mut map = serde_json::Map::new();
        for (key, value) in std::env::vars() {
            if let Some(path) = key.strip_prefix(ENV_PREFIX) {
                if path.is_empty() { continue; }
                let config_path = path.to_lowercase().replace('_', ".");
                let typed_value = Self::infer_value(&value);
                set_json_path(&mut map, &config_path, typed_value);
            }
        }
        Value::Object(map)
    }

    /// 推断环境变量值的类型。
    fn infer_value(s: &str) -> Value {
        // 空字符串
        if s.is_empty() {
            return json!("");
        }
        // 布尔值
        if let Ok(b) = s.parse::<bool>() {
            return json!(b);
        }
        // 整数
        if let Ok(i) = s.parse::<i64>() {
            return json!(i);
        }
        // 浮点数
        if let Ok(f) = s.parse::<f64>() {
            return json!(f);
        }
        // 字符串
        json!(s)
    }

    /// 执行三平面合并并更新 merged 值。
    async fn do_merge(&self) -> ConfigResult<()> {
        let mut inner = self.inner.write().await;

        // 从低到高逐层合并
        let base = inner.planes.get(&ConfigPlane::Default)
            .cloned()
            .unwrap_or_else(default_config_from_keys);

        let after_file = Self::deep_merge(
            &base,
            inner.planes.get(&ConfigPlane::File).unwrap_or(&json!({})),
        );

        let after_env = Self::deep_merge(
            &after_file,
            inner.planes.get(&ConfigPlane::Env).unwrap_or(&json!({})),
        );

        let old = std::mem::replace(&mut inner.merged, after_env);
        drop(inner); // 提前释放锁

        // 通知提供者
        self.providers.notify_all(&old, &self.inner.read().await.merged).await;

        Ok(())
    }

    /// 广播配置变更事件。
    fn broadcast_change(&self, path: String, old: Option<Value>, new: Value, plane: ConfigPlane) {
        let event = ConfigChangeEvent::new(path, old, new, plane);
        let _ = self.tx.send(event);
    }
}

impl Default for LvpaConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConfigManager for LvpaConfigManager {
    async fn load(&mut self) -> ConfigResult<()> {
        // 1. 加载默认值（已在 new 中完成）
        // 2. 加载文件配置
        let file_cfg = Self::load_file_config(&self.config_path);
        self.inner.write().await.planes.insert(ConfigPlane::File, file_cfg);

        // 3. 加载环境变量配置
        let env_cfg = Self::load_env_config();
        self.inner.write().await.planes.insert(ConfigPlane::Env, env_cfg);

        // 4. 合并
        self.do_merge().await
    }

    async fn get<T: DeserializeOwned + Send>(&self, path: &str) -> ConfigResult<T> {
        let inner = self.inner.read().await;
        let value = get_json_path(&inner.merged, path)
            .cloned()
            .ok_or_else(|| ConfigError::InvalidPath(format!("路径不存在: {}", path)))?;

        serde_json::from_value(value)
            .map_err(|e| ConfigError::TypeMismatch(format!("路径 '{}' 类型转换失败: {}", path, e)))
    }

    async fn set<T: Serialize + Send>(&mut self, path: &str, value: T) -> ConfigResult<()> {
        let new_val = serde_json::to_value(value)
            .map_err(|e| ConfigError::Serialization(format!("序列化失败: {}", e)))?;

        let old_val = {
            let inner = self.inner.read().await;
            get_json_path(&inner.merged, path).cloned()
        };

        {
            let mut inner = self.inner.write().await;
            set_json_path_from_root(&mut inner.merged, path, new_val.clone());
        }

        // 保存到配置文件
        self.save_file().await?;

        self.broadcast_change(path.to_string(), old_val, new_val, ConfigPlane::File);
        self.providers.notify_all(
            &self.inner.read().await.merged,
            &self.inner.read().await.merged,
        ).await;

        Ok(())
    }

    async fn reset(&mut self, path: Option<&str>) -> ConfigResult<()> {
        let defaults = default_config_from_keys();

        if let Some(p) = path {
            let default_val = get_json_path(&defaults, p)
                .cloned()
                .ok_or_else(|| ConfigError::InvalidPath(format!("无法重置未知路径: {}", p)))?;

            let old_val = {
                let inner = self.inner.read().await;
                get_json_path(&inner.merged, p).cloned()
            };

            {
                let mut inner = self.inner.write().await;
                set_json_path_from_root(&mut inner.merged, p, default_val.clone());
            }

            self.save_file().await?;
            self.broadcast_change(p.to_string(), old_val, default_val, ConfigPlane::Default);
        } else {
            // 重置全部
            let old_val = {
                let inner = self.inner.read().await;
                inner.merged.clone()
            };

            {
                let mut inner = self.inner.write().await;
                inner.merged = defaults.clone();
                inner.planes.insert(ConfigPlane::Default, defaults);
                inner.planes.remove(&ConfigPlane::File);
                inner.planes.remove(&ConfigPlane::Env);
            }

            self.save_file().await?;
            self.broadcast_change("*".to_string(), Some(old_val), json!("<reset>"), ConfigPlane::Default);
        }

        Ok(())
    }

    async fn validate(&self) -> ConfigResult<Vec<String>> {
        let inner = self.inner.read().await;
        let mut warnings = Vec::new();

        // 已知键存在性校验
        warnings.extend(validate_known_keys(&inner.merged));

        // 未知键提醒
        warnings.extend(warn_unknown_keys(&inner.merged));

        // 提供者校验
        let provider_warnings = self.providers.validate_all(&inner.merged).await;
        warnings.extend(provider_warnings);

        Ok(warnings)
    }

    fn subscribe(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.tx.subscribe()
    }
}

impl LvpaConfigManager {
    /// 保存当前 merged 配置到 TOML 文件。
    async fn save_file(&self) -> ConfigResult<()> {
        let inner = self.inner.read().await;

        // 确保配置目录存在
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ConfigError::Io(format!("创建配置目录失败: {}", e)))?;
        }

        // JSON → TOML 转换（仅序列化非 null 值）
        let toml_value = json_to_toml(&inner.merged);
        let toml_str = toml::to_string_pretty(&toml_value)
            .map_err(|e| ConfigError::Serialization(format!("TOML 序列化失败: {}", e)))?;

        // 原子写入
        let tmp_path = self.config_path.with_extension("tmp");
        std::fs::write(&tmp_path, &toml_str)
            .map_err(|e| ConfigError::Io(format!("写入配置文件失败: {}", e)))?;
        std::fs::rename(&tmp_path, &self.config_path)
            .map_err(|e| ConfigError::Io(format!("重命名配置文件失败: {}", e)))?;

        Ok(())
    }
}

// === JSON 路径工具 ===

/// 按点路径从 Value 中取值。
fn get_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

/// 按点路径从 Value 根节点设值（自动创建中间 Object）。
fn set_json_path_from_root(root: &mut Value, path: &str, value: Value) {
    if let Value::Object(ref mut map) = root {
        set_json_path(map, path, value);
    }
}

/// 按点路径在 Map 中设值（递归创建中间 Object）。
fn set_json_path(map: &mut serde_json::Map<String, Value>, path: &str, value: Value) {
    let segments: Vec<&str> = path.split('.').collect();
    let len = segments.len();
    if len <= 1 {
        if len == 1 {
            map.insert(segments[0].to_string(), value);
        }
        return;
    }

    let mut keys_owned: Vec<String> = segments.iter().map(|s| s.to_string()).collect();
    let last_key = keys_owned.pop().unwrap();

    let mut target = map;
    for key in &keys_owned {
        if !target.contains_key(key.as_str()) || !target[key.as_str()].is_object() {
            target.insert(key.clone(), Value::Object(serde_json::Map::new()));
        }
        let next = target.get_mut(key.as_str())
            .and_then(|v| v.as_object_mut());
        match next {
            Some(inner) => target = inner,
            None => return,
        }
    }

    target.insert(last_key, value);
}

/// serde_json::Value → toml::Value 转换。
fn json_to_toml(value: &Value) -> toml::Value {
    match value {
        Value::Null => toml::Value::String("".to_string()),
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                toml::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                toml::Value::Float(f)
            } else {
                toml::Value::String(n.to_string())
            }
        }
        Value::String(s) => toml::Value::String(s.clone()),
        Value::Array(arr) => {
            toml::Value::Array(arr.iter().map(json_to_toml).collect())
        }
        Value::Object(obj) => {
            let mut table = toml::map::Map::new();
            for (k, v) in obj {
                table.insert(k.clone(), json_to_toml(v));
            }
            toml::Value::Table(table)
        }
    }
}

// === 单元测试 ===

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::env;
    use tempfile;

    fn tmp_config_path() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let path = dir.path().join("config.toml");
        (dir, path)
    }

    #[tokio::test]
    async fn test_default_values() {
        // 清除可能的环境变量干扰（测试在单进程中运行，env 是全局的）
        env::remove_var("LVPA_MONITOR_LEVEL");
        env::remove_var("LVPA_HARNESS_MODE");
        let (_tmp, tmp_path) = tmp_config_path();
        let mut cm = LvpaConfigManager::with_path(tmp_path);
        cm.load().await.unwrap();
        let level: String = cm.get("monitor.level").await.unwrap();
        assert_eq!(level, "info");
    }

    #[tokio::test]
    async fn test_set_and_get() {
        let (_tmp, tmp_path) = tmp_config_path();
        let mut cm = LvpaConfigManager::with_path(tmp_path);
        cm.load().await.unwrap();
        cm.set("monitor.level", "debug").await.unwrap();
        let level: String = cm.get("monitor.level").await.unwrap();
        assert_eq!(level, "debug");
    }

    #[tokio::test]
    async fn test_dotted_path_support() {
        let (_tmp, tmp_path) = tmp_config_path();
        let mut cm = LvpaConfigManager::with_path(tmp_path);
        cm.load().await.unwrap();
        let rps: i64 = cm.get("harness.resource_quota.max_rps").await.unwrap();
        assert_eq!(rps, 100);
    }

    #[tokio::test]
    async fn test_reset_single_path() {
        let (_tmp, tmp_path) = tmp_config_path();
        let mut cm = LvpaConfigManager::with_path(tmp_path);
        cm.load().await.unwrap();
        cm.set("monitor.level", "debug").await.unwrap();
        cm.reset(Some("monitor.level")).await.unwrap();
        let level: String = cm.get("monitor.level").await.unwrap();
        assert_eq!(level, "info"); // 回到默认值
    }

    #[tokio::test]
    async fn test_subscribe_event() {
        let (_tmp, tmp_path) = tmp_config_path();
        let mut cm = LvpaConfigManager::with_path(tmp_path);
        cm.load().await.unwrap();
        let mut rx = cm.subscribe();
        cm.set("monitor.level", "warn").await.unwrap();
        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv()).await;
        assert!(event.is_ok(), "应在超时前收到事件");
        let event = event.unwrap().unwrap();
        assert_eq!(event.path, "monitor.level");
        assert_eq!(event.new_value, json!("warn"));
    }

    #[tokio::test]
    async fn test_env_override() {
        let (_tmp, tmp_path) = tmp_config_path();
        env::set_var("LVPA_MONITOR_LEVEL", "error");
        env::set_var("LVPA_HARNESS_MODE", "explore");
        let mut cm = LvpaConfigManager::with_path(tmp_path);
        cm.load().await.unwrap();
        let level: String = cm.get("monitor.level").await.unwrap();
        assert_eq!(level, "error", "env 应覆盖默认值");
        let mode: String = cm.get("harness.mode").await.unwrap();
        assert_eq!(mode, "explore", "env 应覆盖默认值");
        env::remove_var("LVPA_MONITOR_LEVEL");
        env::remove_var("LVPA_HARNESS_MODE");
    }

    #[tokio::test]
    async fn test_validate() {
        env::remove_var("LVPA_MONITOR_LEVEL");
        env::remove_var("LVPA_HARNESS_MODE");
        let (_tmp, tmp_path) = tmp_config_path();
        let mut cm = LvpaConfigManager::with_path(tmp_path);
        cm.load().await.unwrap();
        let warnings = cm.validate().await.unwrap();
        assert!(warnings.is_empty(), "默认配置应无警告: {:?}", warnings);
    }

    #[tokio::test]
    async fn test_get_nonexistent_path() {
        let cm = LvpaConfigManager::new();
        let result: ConfigResult<String> = cm.get("nonexistent.path").await;
        assert!(result.is_err(), "不存在的路径应返回错误");
    }

    #[test]
    fn test_deep_merge() {
        let base = json!({
            "a": 1,
            "b": { "c": 2, "d": 3 }
        });
        let override_val = json!({
            "b": { "c": 99, "e": 4 },
            "f": 5
        });
        let merged = LvpaConfigManager::deep_merge(&base, &override_val);
        assert_eq!(merged["a"], json!(1));
        assert_eq!(merged["b"]["c"], json!(99));
        assert_eq!(merged["b"]["d"], json!(3));
        assert_eq!(merged["b"]["e"], json!(4));
        assert_eq!(merged["f"], json!(5));
    }

    #[test]
    fn test_infer_value_types() {
        assert_eq!(LvpaConfigManager::infer_value("true"), json!(true));
        assert_eq!(LvpaConfigManager::infer_value("42"), json!(42));
        assert_eq!(LvpaConfigManager::infer_value("3.14"), json!(3.14));
        assert_eq!(LvpaConfigManager::infer_value("hello"), json!("hello"));
        assert_eq!(LvpaConfigManager::infer_value(""), json!(""));
    }
}
