//! 工坊配置模块 — WorkshopConfig 定义 + TOML 加载 + 4 条默认配置。
//!
//! 参考: 架构总纲 §7.1 — 工坊 DAG 定义
//!       Phase-工坊系统-规划.md §五 — 4 条工坊默认 TOML

use serde::{Deserialize, Serialize};

use taiji_types::agent::SpiritRoot;
use taiji_types::workshop_dungeon::{WorkshopDagNode, WorkshopType};

/// 工坊配置（TOML 驱动）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopConfig {
    pub workshop_type: WorkshopType,
    pub name: String,
    pub description: String,
    pub required_spirit_roots: Vec<SpiritRoot>,
    pub dag_nodes: Vec<WorkshopDagNode>,
    pub output_type: String,
}

/// 从 TOML 字符串加载 WorkshopConfig 列表。
pub fn load_workshop_configs(toml_str: &str) -> Result<Vec<WorkshopConfig>, String> {
    #[derive(Serialize, Deserialize)]
    struct ConfigFile {
        workshops: Vec<WorkshopConfig>,
    }
    let config: ConfigFile = toml::from_str(toml_str).map_err(|e| format!("TOML parse error: {}", e))?;
    Ok(config.workshops)
}

/// 4 条工坊默认 TOML 配置。
pub const DEFAULT_WORKSHOP_TOML: &str = r##"
[[workshops]]
workshop_type = "tianji"
name = "天机坊"
description = "代码开发与系统构建"
required_spirit_roots = ["metal", "earth"]
output_type = "code_artifact"
dag_nodes = [
    { name = "需求分析", description = "收集和分析开发需求", input_keys = [], output_keys = ["spec"] },
    { name = "编码实现", description = "根据需求实现代码", input_keys = ["spec"], output_keys = ["code"] },
    { name = "代码审查", description = "代码审查和质量检查", input_keys = ["code"], output_keys = ["reviewed_code"] },
    { name = "构建部署", description = "构建和部署到目标环境", input_keys = ["reviewed_code"], output_keys = ["deployment"] },
]

[[workshops]]
workshop_type = "jinsuan"
name = "金算坊"
description = "量化策略开发与交易执行"
required_spirit_roots = ["metal"]
output_type = "signal"
dag_nodes = [
    { name = "行情分析", description = "分析市场行情数据", input_keys = [], output_keys = ["market_data"] },
    { name = "信号计算", description = "根据行情计算交易信号", input_keys = ["market_data"], output_keys = ["signal"] },
    { name = "风控审核", description = "风控检查信号合规性", input_keys = ["signal"], output_keys = ["risk_checked_signal"] },
    { name = "交易执行", description = "执行交易指令", input_keys = ["risk_checked_signal"], output_keys = ["order"] },
]

[[workshops]]
workshop_type = "danqing"
name = "丹青坊"
description = "视觉设计与美术资源制作"
required_spirit_roots = ["wood", "fire"]
output_type = "design_asset"
dag_nodes = [
    { name = "需求沟通", description = "与需求方沟通设计需求", input_keys = [], output_keys = ["brief"] },
    { name = "草图设计", description = "绘制设计草稿", input_keys = ["brief"], output_keys = ["sketch"] },
    { name = "精稿制作", description = "根据草图制作精稿", input_keys = ["sketch"], output_keys = ["final_art"] },
    { name = "审核交付", description = "审核并交付最终设计", input_keys = ["final_art"], output_keys = ["delivery"] },
]

[[workshops]]
workshop_type = "liuying"
name = "留影坊"
description = "视频制作与内容发布"
required_spirit_roots = ["wood"]
output_type = "video_content"
dag_nodes = [
    { name = "选题策划", description = "策划视频选题", input_keys = [], output_keys = ["topic"] },
    { name = "素材采集", description = "采集视频素材", input_keys = ["topic"], output_keys = ["raw_footage"] },
    { name = "剪辑合成", description = "剪辑合成最终成片", input_keys = ["raw_footage"], output_keys = ["edited_video"] },
    { name = "发布上线", description = "发布视频到目标平台", input_keys = ["edited_video"], output_keys = ["published"] },
]
"##;

/// 返回 4 条工坊默认配置。
pub(crate) fn default_workshop_configs() -> Vec<WorkshopConfig> {
    load_workshop_configs(DEFAULT_WORKSHOP_TOML).expect("default workshop TOML must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_configs_load() {
        let configs = default_workshop_configs();
        assert_eq!(configs.len(), 4);
    }

    #[test]
    fn test_default_configs_have_dag_nodes() {
        let configs = default_workshop_configs();
        for cfg in &configs {
            assert!(!cfg.dag_nodes.is_empty(), "{} has no DAG nodes", cfg.name);
        }
    }

    #[test]
    fn test_default_configs_have_spirit_roots() {
        let configs = default_workshop_configs();
        for cfg in &configs {
            assert!(!cfg.required_spirit_roots.is_empty(), "{} has no spirit root requirements", cfg.name);
        }
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let configs = default_workshop_configs();
        let json = serde_json::to_string(&configs).unwrap();
        let back: Vec<WorkshopConfig> = serde_json::from_str(&json).unwrap();
        assert_eq!(configs.len(), back.len());
        assert_eq!(configs[0].name, back[0].name);
    }

    #[test]
    fn test_load_invalid_toml() {
        let result = load_workshop_configs("invalid toml [[[");
        assert!(result.is_err());
    }
}
