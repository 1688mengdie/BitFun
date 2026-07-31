//! DefaultHarness — Harness trait 的默认实现
//!
//! 实现 6 阶段流水线权限检查（参考 AgentScope PermissionEngine DEFAULT 模式）。
//!
//! 来源: modules/harness/接口设计.md §1-4 — Harness trait + 6 阶段流水线

use std::sync::RwLock;

use taiji_types::harness::{Harness, HarnessConfig};
use taiji_types::permission::{
    GateCommand, GuardLevel, PermissionBehavior, PermissionRule,
};

/// DefaultHarness — 护山大阵默认实现
///
/// 6 阶段流水线：
/// 1. Deny rules → 最高优先级
/// 2. Ask rules
/// 3. Read-only fast path
/// 4. tool.check_permissions (本实现简化: 跳过)
/// 5. Allow rules
/// 6. 模式降级
pub struct DefaultHarness {
    mode: RwLock<GuardLevel>,
    deny_rules: RwLock<Vec<PermissionRule>>,
    ask_rules: RwLock<Vec<PermissionRule>>,
    allow_rules: RwLock<Vec<PermissionRule>>,
}

impl DefaultHarness {
    /// 创建 DefaultHarness（默认 Default 模式，空规则集）
    pub fn new() -> Self {
        Self::from_config(&HarnessConfig::default())
    }

    /// 从配置创建 DefaultHarness
    pub fn from_config(config: &HarnessConfig) -> Self {
        let mut deny = Vec::new();
        let mut ask = Vec::new();
        let mut allow = Vec::new();

        for rule in &config.rules {
            match rule.behavior {
                PermissionBehavior::Deny => deny.push(rule.clone()),
                PermissionBehavior::Ask => ask.push(rule.clone()),
                PermissionBehavior::Allow => allow.push(rule.clone()),
                PermissionBehavior::Passthrough => {} // 透传规则不处理
            }
        }

        Self {
            mode: RwLock::new(config.guard_level),
            deny_rules: RwLock::new(deny),
            ask_rules: RwLock::new(ask),
            allow_rules: RwLock::new(allow),
        }
    }
}

impl Default for DefaultHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness for DefaultHarness {
    /// 执行 6 阶段流水线权限检查
    ///
    /// 1. Deny rules — 最高优先级，匹配即 DENY
    /// 2. Ask rules — 匹配即 ASK
    /// 3. Read-only fast path — is_readonly()=true 直接 ALLOW
    /// 4. tool.check_permissions — 本实现简化，跳过
    /// 5. Allow rules — 匹配即 ALLOW
    /// 6. 模式降级 — Default→ASK, Explore→DENY, Bypass→ALLOW, DontAsk→DENY
    async fn check(&self, tool_name: &str, _input: &serde_json::Value) -> GateCommand {
        // ── Phase 1: Deny rules ──
        if let Some(rule) = self.deny_rules.read().unwrap_or_else(|e| e.into_inner()).iter().find(|r| r.tool_name == tool_name) {
            if Self::rule_matches(rule, tool_name, _input) {
                return GateCommand::Deny;
            }
        }

        // ── Phase 2: Ask rules ──
        if let Some(rule) = self.ask_rules.read().unwrap_or_else(|e| e.into_inner()).iter().find(|r| r.tool_name == tool_name) {
            if Self::rule_matches(rule, tool_name, _input) {
                return GateCommand::Ask {
                    suggested_rules: vec![rule.clone()],
                };
            }
        }

        // ── Phase 3: Read-only fast path ──
        if Self::is_readonly(tool_name) {
            return GateCommand::Allow;
        }

        // ── Phase 4: tool.check_permissions ──
        // 本实现简化：跳过（完整实现在 Phase 5 中集成）

        // ── Phase 5: Allow rules ──
        if let Some(rule) = self.allow_rules.read().unwrap_or_else(|e| e.into_inner()).iter().find(|r| r.tool_name == tool_name) {
            if Self::rule_matches(rule, tool_name, _input) {
                return GateCommand::Allow;
            }
        }

        // ── Phase 6: 模式降级 ──
        match *self.mode.read().unwrap_or_else(|e| e.into_inner()) {
            GuardLevel::Default => GateCommand::Ask {
                suggested_rules: vec![],
            },
            GuardLevel::AcceptEdits => GateCommand::Ask {
                suggested_rules: vec![],
            },
            GuardLevel::Explore => GateCommand::Deny,
            GuardLevel::Bypass => GateCommand::Allow,
            GuardLevel::DontAsk => GateCommand::Deny,
        }
    }

    fn add_rule(&mut self, rule: PermissionRule) {
        let rules = match rule.behavior {
            PermissionBehavior::Deny => &mut *self.deny_rules.write().unwrap_or_else(|e| e.into_inner()),
            PermissionBehavior::Ask => &mut *self.ask_rules.write().unwrap_or_else(|e| e.into_inner()),
            PermissionBehavior::Allow => &mut *self.allow_rules.write().unwrap_or_else(|e| e.into_inner()),
            PermissionBehavior::Passthrough => return,
        };
        rules.push(rule);
    }

    fn guard_level(&self) -> GuardLevel {
        *self.mode.read().unwrap_or_else(|e| e.into_inner())
    }

    fn set_guard_level(&mut self, mode: GuardLevel) {
        *self.mode.write().unwrap_or_else(|e| e.into_inner()) = mode;
    }
}

impl DefaultHarness {
    /// 判断是否只读工具
    fn is_readonly(tool_name: &str) -> bool {
        matches!(
            tool_name,
            "read" | "Read" | "search" | "Search" | "list" | "List"
                | "grep" | "Grep" | "glob" | "Glob"
        )
    }

    /// 检查规则是否匹配（简化实现：仅 tool_name 精确匹配 + rule_content 子串匹配）
    fn rule_matches(rule: &PermissionRule, tool_name: &str, input: &serde_json::Value) -> bool {
        if rule.tool_name != tool_name {
            return false;
        }
        if let Some(ref content) = rule.rule_content {
            // 对 tool input 做简单的子串匹配
            let input_str = serde_json::to_string(input).unwrap_or_default();
            input_str.contains(content.as_str())
        } else {
            true // 无条件匹配
        }
    }
}

// ── 只读工具辅助函数 ──

/// 只读工具列表（供外部使用）
pub const READONLY_TOOLS: &[&str] = &[
    "read", "Read", "search", "Search", "list", "List",
    "grep", "Grep", "glob", "Glob",
];

/// 判断工具名是否为只读
pub fn is_readonly_tool(tool_name: &str) -> bool {
    READONLY_TOOLS.contains(&tool_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use taiji_types::permission::PermissionBehavior;

    fn make_rule(tool_name: &str, behavior: PermissionBehavior) -> PermissionRule {
        PermissionRule {
            tool_name: tool_name.into(),
            rule_content: None,
            behavior,
            source: "test".into(),
        }
    }

    #[tokio::test]
    async fn test_deny_highest_priority() {
        let mut harness = DefaultHarness::new();
        harness.add_rule(make_rule("read", PermissionBehavior::Deny));
        harness.add_rule(make_rule("read", PermissionBehavior::Allow));

        let input = serde_json::json!({});
        let cmd = harness.check("read", &input).await;
        assert_eq!(cmd, GateCommand::Deny, "Deny 规则应优先于 Allow");
    }

    #[tokio::test]
    async fn test_readonly_fast_path() {
        let harness = DefaultHarness::new();
        let input = serde_json::json!({});
        let cmd = harness.check("read", &input).await;
        assert_eq!(cmd, GateCommand::Allow, "Read-only 工具应直接 ALLOW");
    }

    #[tokio::test]
    async fn test_non_readonly_default_asks() {
        let harness = DefaultHarness::new();
        let input = serde_json::json!({});
        let cmd = harness.check("write", &input).await;
        assert_eq!(
            cmd,
            GateCommand::Ask {
                suggested_rules: vec![]
            },
            "Default 模式下非只读工具应 ASK"
        );
    }

    #[tokio::test]
    async fn test_explore_mode_denies() {
        let mut harness = DefaultHarness::new();
        harness.set_guard_level(GuardLevel::Explore);

        let input = serde_json::json!({});
        let cmd = harness.check("write", &input).await;
        assert_eq!(cmd, GateCommand::Deny, "Explore 模式应 DENY");
    }

    #[tokio::test]
    async fn test_bypass_mode_allows() {
        let mut harness = DefaultHarness::new();
        harness.set_guard_level(GuardLevel::Bypass);

        let input = serde_json::json!({});
        let cmd = harness.check("write", &input).await;
        assert_eq!(cmd, GateCommand::Allow, "Bypass 模式应 ALLOW");
    }

    #[tokio::test]
    async fn test_dont_ask_denies() {
        let mut harness = DefaultHarness::new();
        harness.set_guard_level(GuardLevel::DontAsk);

        let input = serde_json::json!({});
        let cmd = harness.check("write", &input).await;
        assert_eq!(cmd, GateCommand::Deny, "DontAsk 模式应 DENY");
    }

    #[tokio::test]
    async fn test_allow_rule_matches() {
        let mut harness = DefaultHarness::new();
        harness.add_rule(make_rule("write_file", PermissionBehavior::Allow));

        let input = serde_json::json!({"path": "test.txt"});
        let cmd = harness.check("write_file", &input).await;
        assert_eq!(cmd, GateCommand::Allow, "Allow 规则应生效");
    }

    #[tokio::test]
    async fn test_ask_rule_matches() {
        let mut harness = DefaultHarness::new();
        harness.add_rule(make_rule("execute", PermissionBehavior::Ask));

        let input = serde_json::json!({"cmd": "rm -rf /"});
        let cmd = harness.check("execute", &input).await;
        assert!(matches!(cmd, GateCommand::Ask { .. }), "Ask 规则应返回 ASK");
    }

    #[tokio::test]
    async fn test_guard_level_get_set() {
        let mut harness = DefaultHarness::new();
        assert_eq!(harness.guard_level(), GuardLevel::Default);

        harness.set_guard_level(GuardLevel::Explore);
        assert_eq!(harness.guard_level(), GuardLevel::Explore);
    }

    #[tokio::test]
    async fn test_rule_matching_with_content() {
        let rule = PermissionRule {
            tool_name: "write".into(),
            rule_content: Some("dangerous".into()),
            behavior: PermissionBehavior::Deny,
            source: "test".into(),
        };
        let input = serde_json::json!({"content": "this is dangerous content"});
        assert!(
            DefaultHarness::rule_matches(&rule, "write", &input),
            "input 包含 'dangerous' 应匹配"
        );

        let input2 = serde_json::json!({"content": "safe content"});
        assert!(
            !DefaultHarness::rule_matches(&rule, "write", &input2),
            "input 不包含 'dangerous' 不应匹配"
        );
    }

    #[test]
    fn test_empty_harness_has_default_mode() {
        let harness = DefaultHarness::new();
        assert_eq!(harness.guard_level(), GuardLevel::Default);
    }

    #[test]
    fn test_readonly_tools_list() {
        assert!(is_readonly_tool("read"));
        assert!(is_readonly_tool("search"));
        assert!(is_readonly_tool("Grep"));
        assert!(!is_readonly_tool("write"));
        assert!(!is_readonly_tool("execute"));
    }

    #[tokio::test]
    async fn test_accept_edits_mode_asks() {
        let mut harness = DefaultHarness::new();
        harness.set_guard_level(GuardLevel::AcceptEdits);

        let input = serde_json::json!({});
        let cmd = harness.check("write", &input).await;
        assert_eq!(
            cmd,
            GateCommand::Ask {
                suggested_rules: vec![]
            },
            "AcceptEdits 模式非只读工具应 ASK"
        );
    }
}
