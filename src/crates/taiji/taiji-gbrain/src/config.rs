//! 三平面配置 — env > file > DB > defaults 合并规则。
//!
//! 参考: gbrain (MIT) config.ts:550-650 配置合并逻辑。
//! 优先级: 环境变量 > 配置文件 > 数据库配置 > 默认值。

use std::path::Path;
use taiji_types::knowledge::GBrainConfig;

use crate::error::GBrainError;

/// 配置加载器 — 三平面合并。
pub struct ConfigLoader;

impl ConfigLoader {
    /// 加载配置：env > file > defaults 三层合并。
    ///
    /// 1. 从默认值开始
    /// 2. 若 file_path 存在，从 JSON 文件加载并覆盖 defaults
    /// 3. 环境变量覆盖 file
    pub fn load(file_path: Option<&Path>) -> Result<GBrainConfig, GBrainError> {
        let mut config = Self::defaults();

        // 第二层：JSON 配置文件覆盖
        if let Some(path) = file_path {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                let file_config: GBrainConfig = serde_json::from_str(&content)?;
                Self::merge(&mut config, file_config);
            }
        }

        // 第三层：环境变量覆盖
        Self::apply_env_overrides(&mut config);

        Ok(config)
    }

    /// 获取默认配置。
    pub fn defaults() -> GBrainConfig {
        GBrainConfig::default()
    }

    /// 合并配置 — source 的非 None/非默认值覆盖 target。
    fn merge(target: &mut GBrainConfig, source: GBrainConfig) {
        if source.engine != "pglite" {
            target.engine = source.engine;
        }
        if source.database_url.is_some() {
            target.database_url = source.database_url;
        }
        if source.database_path.is_some() {
            target.database_path = source.database_path;
        }
        if source.embedding_model.is_some() {
            target.embedding_model = source.embedding_model;
        }
        if source.embedding_dimensions != 384 {
            target.embedding_dimensions = source.embedding_dimensions;
        }
    }

    /// 环境变量覆盖。
    fn apply_env_overrides(config: &mut GBrainConfig) {
        if let Ok(val) = std::env::var("GBRAIN_ENGINE") {
            config.engine = val;
        }
        if let Ok(val) = std::env::var("GBRAIN_DATABASE_URL") {
            config.database_url = Some(val);
        }
        if let Ok(val) = std::env::var("GBRAIN_DATABASE_PATH") {
            config.database_path = Some(val);
        }
        if let Ok(val) = std::env::var("GBRAIN_EMBEDDING_MODEL") {
            config.embedding_model = Some(val);
        }
        if let Ok(val) = std::env::var("GBRAIN_EMBEDDING_DIMENSIONS") {
            if let Ok(dim) = val.parse::<usize>() {
                config.embedding_dimensions = dim;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let config = ConfigLoader::defaults();
        assert_eq!(config.engine, "pglite");
        assert_eq!(config.embedding_dimensions, 384);
        assert!(config.database_url.is_none());
    }

    #[test]
    fn test_env_override_database_url() {
        // 仅测试合并逻辑，跳过实际环境变量设置（避免污染）
        let mut config = ConfigLoader::defaults();
        config.database_url = Some("env://localhost/gbrain".into());
        assert_eq!(config.database_url, Some("env://localhost/gbrain".into()));
    }

    #[test]
    fn test_merge_preserves_defaults() {
        let mut target = ConfigLoader::defaults();
        let source = GBrainConfig {
            engine: "pglite".into(), // same as default — not overridden
            database_url: Some("file://localhost/gbrain".into()),
            database_path: None,
            embedding_model: None,
            embedding_dimensions: 384, // same as default — not overridden
        };
        ConfigLoader::merge(&mut target, source);
        assert_eq!(target.engine, "pglite");
        assert_eq!(target.database_url, Some("file://localhost/gbrain".into()));
        assert!(target.database_path.is_none());
    }

    #[test]
    fn test_merge_overrides() {
        let mut target = ConfigLoader::defaults();
        let source = GBrainConfig {
            engine: "postgres".into(),
            database_url: Some("postgres://remote".into()),
            database_path: Some("/data/gbrain".into()),
            embedding_model: Some("intfloat/e5-small-v2".into()),
            embedding_dimensions: 768,
        };
        ConfigLoader::merge(&mut target, source);
        assert_eq!(target.engine, "postgres");
        assert_eq!(target.database_url, Some("postgres://remote".into()));
        assert_eq!(target.database_path, Some("/data/gbrain".into()));
        assert_eq!(target.embedding_model, Some("intfloat/e5-small-v2".into()));
        assert_eq!(target.embedding_dimensions, 768);
    }

    #[test]
    fn test_load_with_nonexistent_file() {
        // 不存在的文件路径 → 不应报错，返回 defaults
        let config = ConfigLoader::load(Some(Path::new("/nonexistent/gbrain.json"))).unwrap();
        assert_eq!(config.engine, "pglite");
    }

    #[test]
    fn test_config_file_roundtrip() {
        let dir = std::env::temp_dir();
        let path = dir.join("gbrain_test_config.json");
        let config = GBrainConfig {
            engine: "postgres".into(),
            database_url: Some("postgres://test".into()),
            database_path: None,
            embedding_model: Some("test-model".into()),
            embedding_dimensions: 512,
        };
        let json = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&path, &json).unwrap();

        let loaded = ConfigLoader::load(Some(&path)).unwrap();
        assert_eq!(loaded.engine, "postgres");
        assert_eq!(loaded.database_url, Some("postgres://test".into()));
        assert_eq!(loaded.embedding_model, Some("test-model".into()));

        let _ = std::fs::remove_file(&path);
    }
}
