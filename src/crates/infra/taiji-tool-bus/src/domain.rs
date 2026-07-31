//! TaijiToolDomain — 灵根→工具组绑定。
//!
//! 参考: modules/tool-bus/接口设计.md §4 — R-4-301 — LVPA 特有

use serde::{Deserialize, Serialize};

/// 工具域 — 灵根→工具组绑定（R-4-301-02）。
///
/// 将工具按功能域分组，与 Agent 灵根（职业）对应：
/// - Basic: 基础功法（所有 Agent 可用）
/// - Agent: 宗门管理（Agent 控制工具）
/// - Trading: 金算坊（交易工具）
/// - Teaching: 木灵根（教学工具）
/// - Analysis: 火灵根（分析工具）
/// - Development: 土灵根（开发工具）
/// - Integration: 坊市（外部集成/MCP/ACP）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaijiToolDomain {
    Basic,
    Agent,
    Trading,
    Teaching,
    Analysis,
    Development,
    Integration,
}

impl TaijiToolDomain {
    /// 返回所有域的列表。
    pub fn all() -> Vec<Self> {
        vec![
            Self::Basic,
            Self::Agent,
            Self::Trading,
            Self::Teaching,
            Self::Analysis,
            Self::Development,
            Self::Integration,
        ]
    }
}

impl std::fmt::Display for TaijiToolDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Basic => write!(f, "Basic"),
            Self::Agent => write!(f, "Agent"),
            Self::Trading => write!(f, "Trading"),
            Self::Teaching => write!(f, "Teaching"),
            Self::Analysis => write!(f, "Analysis"),
            Self::Development => write!(f, "Development"),
            Self::Integration => write!(f, "Integration"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_serde_roundtrip() {
        for domain in TaijiToolDomain::all() {
            let json = serde_json::to_string(&domain).unwrap();
            let back: TaijiToolDomain = serde_json::from_str(&json).unwrap();
            assert_eq!(domain, back);
        }
    }

    #[test]
    fn test_domain_all_returns_all() {
        let all = TaijiToolDomain::all();
        assert_eq!(all.len(), 7);
    }

    #[test]
    fn test_domain_display() {
        assert_eq!(format!("{}", TaijiToolDomain::Basic), "Basic");
        assert_eq!(format!("{}", TaijiToolDomain::Trading), "Trading");
    }
}
