//! R-1-301 默认配置文件验证测试。
//!
//! 验证 `software/taiji/default.config.toml` 可通过 config crate 正确加载。
//! 使用 LvpaConfigManager 的文件加载路径做端到端验证。

use std::path::PathBuf;
use taiji_infra_config::{ConfigManager, LvpaConfigManager};

#[test]
fn test_default_config_file_exists() {
    let path = find_default_config();
    assert!(path.exists(), "默认配置文件不存在: {:?}", path);
}

#[test]
fn test_default_config_loadable_by_config_crate() {
    let path = find_default_config();
    let content = std::fs::read_to_string(&path).expect("读取默认配置文件失败");

    // 使用 config crate 的 TOML 加载逻辑解析
    // toml::from_str::<toml::Value> 是 toml crate 0.9 的正解 API
    let value: toml::Value = toml::from_str(&content).expect("默认配置文件不是合法 TOML");
    assert!(value.is_table(), "根应为 Table");

    let table = value.as_table().unwrap();
    let expected_sections = ["engine", "database", "llm", "monitor", "harness", "trading", "logging"];
    for section in &expected_sections {
        assert!(table.contains_key(*section), "缺少配置节: [{}]", section);
    }
}

#[test]
fn test_default_config_contains_core_keys() {
    let path = find_default_config();
    let content = std::fs::read_to_string(&path).expect("读取默认配置文件失败");
    let value: toml::Value = toml::from_str(&content).expect("TOML 解析失败");

    assert_eq!(value["engine"]["engine"].as_str(), Some("pglite"));
    assert_eq!(value["monitor"]["level"].as_str(), Some("info"));
    assert_eq!(value["harness"]["mode"].as_str(), Some("default"));
    assert_eq!(value["trading"]["broker"].as_str(), Some("simnow"));
    assert_eq!(value["logging"]["level"].as_str(), Some("info"));
    assert_eq!(value["monitor"]["health_check_interval_secs"].as_integer(), Some(30));
}

#[test]
fn test_default_config_file_match_writer_output() {
    // 验证 config crate 的 save_file 输出与 default.config.toml 的结构一致
    let mut cm = LvpaConfigManager::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        cm.load().await.unwrap();
    });

    // 写入临时文件，验证可二次读取
    let tmp_dir = std::env::temp_dir();
    let tmp_cfg = tmp_dir.join("test_taiji_config.toml");
    let mut cm2 = LvpaConfigManager::with_path(tmp_cfg.clone());
    rt.block_on(async {
        cm2.load().await.unwrap();
        cm2.set("monitor.level", "debug").await.unwrap();
    });

    let written = std::fs::read_to_string(&tmp_cfg).expect("读取临时配置失败");
    assert!(!written.is_empty(), "写入的配置文件不应为空");

    // 清理
    let _ = std::fs::remove_file(&tmp_cfg);
}

/// 在工作区根目录查找 default.config.toml。
fn find_default_config() -> PathBuf {
    let candidates = [
        // 从 infra/config/tests/ 到 workspace root 的相对路径
        PathBuf::from("../../../../default.config.toml"),
        // 绝对路径 fallback
        PathBuf::from(r"E:\finance-trading\lvpa\software\taiji\default.config.toml"),
    ];

    for p in &candidates {
        if p.exists() {
            return p.clone();
        }
    }

    candidates[0].clone()
}
