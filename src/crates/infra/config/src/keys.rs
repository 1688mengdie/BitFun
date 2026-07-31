//! 天书阁 — 已知配置键注册表。
//!
//! 编译期定义全系统已知配置键，提供默认值和校验支持。
//!
//! 设计参考：gbrain `src/core/config.ts:912-1075` KNOWN_CONFIG_KEYS 注册表模式 + BitFun ConfigStatistics，
//! LVPA 自定配置键集，Rust 翻译实现，非 Cargo 依赖。

use serde_json::{json, Value};
use std::sync::LazyLock;

/// 已知配置键——编译期注册表。
///
/// 所有点路径（dotted path）在使用前需在此注册。
/// 用于拼写检查和默认值生成。
pub struct KnownConfigKey {
    /// 点路径（如 `"monitor.level"`）。
    pub path: &'static str,
    /// 默认值。
    pub default: Value,
    /// 简短描述。
    pub description: &'static str,
}

/// 全系统已知配置键列表。
///
/// 按功能域分组：
/// - `engine`: 引擎配置
/// - `database`: 数据库配置
/// - `llm`: AI Provider 配置
/// - `monitor`: 监控配置
/// - `harness`: 权限门控配置
/// - `trading`: 量化交易配置
/// - `logging`: 日志配置
pub static KNOWN_CONFIG_KEYS: LazyLock<Vec<KnownConfigKey>> = LazyLock::new(|| {
    vec![
        // === engine ===
        KnownConfigKey { path: "engine", default: json!("pglite"), description: "引擎类型 (pglite/postgres)" },
        // === database ===
        KnownConfigKey { path: "database_url", default: json!(null), description: "数据库连接 URL" },
        KnownConfigKey { path: "database_path", default: json!("~/.taiji-quant/data/lvpa.db"), description: "数据库文件路径" },
        // === llm ===
        KnownConfigKey { path: "llm_api_key", default: json!(null), description: "LLM API Key" },
        KnownConfigKey { path: "embedding_model", default: json!("text-embedding-3-small"), description: "嵌入模型名称" },
        KnownConfigKey { path: "embedding_dimensions", default: json!(1536), description: "嵌入向量维度" },
        KnownConfigKey { path: "chat_model", default: json!("gpt-4o"), description: "聊天模型名称" },
        KnownConfigKey { path: "llm_base_url", default: json!(null), description: "LLM API Base URL（代理 / 兼容 API）" },
        KnownConfigKey { path: "llm_temperature", default: json!(0.7), description: "LLM 采样温度 [0.0, 2.0]" },
        KnownConfigKey { path: "llm_max_tokens", default: json!(4096), description: "LLM 最大输出 token 数" },
        // === monitor ===
        KnownConfigKey { path: "monitor.level", default: json!("info"), description: "监控日志级别" },
        KnownConfigKey { path: "monitor.alert_channel", default: json!("stdout"), description: "告警通知渠道" },
        KnownConfigKey { path: "monitor.health_check_interval_secs", default: json!(30), description: "健康检查间隔(秒)" },
        // === harness ===
        KnownConfigKey { path: "harness.mode", default: json!("default"), description: "护山大阵模式" },
        KnownConfigKey { path: "harness.working_directories", default: json!(["~/.taiji-quant"]), description: "允许的工作目录" },
        KnownConfigKey { path: "harness.resource_quota.max_rps", default: json!(100), description: "每秒请求上限" },
        KnownConfigKey { path: "harness.resource_quota.max_concurrency", default: json!(10), description: "最大并发数" },
        // === trading ===
        KnownConfigKey { path: "trading.broker", default: json!("simnow"), description: "交易 broker" },
        KnownConfigKey { path: "trading.account", default: json!(null), description: "交易账号" },
        KnownConfigKey { path: "trading.risk_limit", default: json!(0.1), description: "风险限额" },
        KnownConfigKey { path: "trading.ctp_addr", default: json!("tcp://180.168.146.187:10130"), description: "CTP 地址" },
        // === logging ===
        KnownConfigKey { path: "logging.level", default: json!("info"), description: "日志级别" },
        KnownConfigKey { path: "logging.include_sensitive_diagnostics", default: json!(false), description: "包含敏感诊断信息" },
        KnownConfigKey { path: "logging.path", default: json!("~/.taiji-quant/logs/"), description: "日志路径" },
    ]
});

/// 从已知键生成默认配置 Value（Object）。
pub fn default_config_from_keys() -> Value {
    let mut map = serde_json::Map::new();
    for key in KNOWN_CONFIG_KEYS.iter() {
        set_nested(&mut map, key.path, key.default.clone());
    }
    Value::Object(map)
}

/// 校验配置：检查所有已知键是否存在，返回警告列表。
pub fn validate_known_keys(config: &Value) -> Vec<String> {
    let mut warnings = Vec::new();
    for key in KNOWN_CONFIG_KEYS.iter() {
        if get_nested(config, key.path).is_none() {
            warnings.push(format!("已知键缺失: {} ({})", key.path, key.description));
        }
    }
    warnings
}

/// 校验未知键：返回配置中有但 KNOWN_CONFIG_KEYS 中没有的路径（排除中间父路径）。
pub fn warn_unknown_keys(config: &Value) -> Vec<String> {
    // 收集所有已知键 + 已知键的父路径（如 "harness.resource_quota" 的父路径 "harness"）
    let mut known: std::collections::HashSet<String> = KNOWN_CONFIG_KEYS.iter().map(|k| k.path.to_string()).collect();
    for full_path in KNOWN_CONFIG_KEYS.iter() {
        let parts: Vec<&str> = full_path.path.split('.').collect();
        // 添加所有父路径前缀
        for i in 1..parts.len() {
            known.insert(parts[..i].join("."));
        }
    }

    let mut unknown = Vec::new();
    collect_keys(config, "", &mut unknown);
    unknown.retain(|p| !known.contains(p));
    unknown.sort();
    unknown.dedup();
    if unknown.len() > 5 {
        unknown.truncate(5);
        unknown.push(format!("... 以及其他 {} 个未知键", unknown.len().saturating_sub(5)));
    }
    unknown
}

// === 内部工具函数 ===

/// 在嵌套 Value::Object 中按点路径取值。
fn get_nested<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

/// 在嵌套 Map 中按点路径设值。
fn set_nested(map: &mut serde_json::Map<String, Value>, path: &str, value: Value) {
    let segments: Vec<&str> = path.split('.').collect();
    let len = segments.len();
    if len == 0 {
        return;
    }
    if len == 1 {
        map.insert(segments[0].to_string(), value);
        return;
    }

    // Walk intermediate path segments using indexed access + clone/reinsert
    // to avoid entry() borrow conflicts with map reassignment.
    let mut keys_owned: Vec<String> = segments.iter().map(|s| s.to_string()).collect();
    let last_key = keys_owned.pop().unwrap();

    let mut target = map;
    for key in &keys_owned {
        if !target.contains_key(key.as_str()) || !target[key.as_str()].is_object() {
            target.insert(key.clone(), Value::Object(serde_json::Map::new()));
        }
        // Use split-borrow: clone the current inner map reference, reassign
        let next = target.get_mut(key.as_str())
            .and_then(|v| v.as_object_mut());
        match next {
            Some(inner) => target = inner,
            None => return, // unreachable: just inserted Object above
        }
    }

    target.insert(last_key, value);
}

/// 递归收集 Value::Object 中的全部点路径。
fn collect_keys(value: &Value, prefix: &str, keys: &mut Vec<String>) {
    if let Value::Object(map) = value {
        for (k, v) in map {
            let full = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
            keys.push(full.clone());
            collect_keys(v, &full, keys);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_default_config_has_keys() {
        let cfg = default_config_from_keys();
        for key in KNOWN_CONFIG_KEYS.iter() {
            assert!(get_nested(&cfg, key.path).is_some(), "键缺失: {}", key.path);
        }
    }

    #[test]
    fn test_validate_known_keys_valid() {
        let cfg = default_config_from_keys();
        let warns = validate_known_keys(&cfg);
        assert!(warns.is_empty(), "有警告: {:?}", warns);
    }

    #[test]
    fn test_validate_known_keys_missing() {
        let cfg = json!({});
        let warns = validate_known_keys(&cfg);
        assert!(!warns.is_empty(), "全缺失应产生警告");
    }

    #[test]
    fn test_warn_unknown_keys() {
        let cfg = json!({"unknown_key": "value", "nested": {"unknown": 1}});
        let warns = warn_unknown_keys(&cfg);
        assert!(warns.iter().any(|w| w.contains("unknown_key")), "应检测未知键");
    }

    #[test]
    fn test_set_and_get_nested() {
        let mut map = serde_json::Map::new();
        set_nested(&mut map, "monitor.level", json!("debug"));
        set_nested(&mut map, "a.b.c", json!(42));
        let val = Value::Object(map);
        assert_eq!(get_nested(&val, "monitor.level"), Some(&json!("debug")));
        assert_eq!(get_nested(&val, "a.b.c"), Some(&json!(42)));
        assert_eq!(get_nested(&val, "nonexistent"), None);
    }
}
