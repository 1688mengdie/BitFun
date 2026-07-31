//! R-4-603: ToolBus → Harness 门控集成测试
//!
//! 验证 ToolBus::execute 强制 Harness 门控的全链路：
//!   1. 注册工具到 ToolRegistry
//!   2. 配置 Harness（自定义 Allow/Deny/Ask harness）
//!   3. 执行工具 → Harness.check() 裁决 → execute 返回对应结果
//!   4. 集成 DefaultHarness + 规则配置
//!
//! 参考: modules/tool-bus/接口设计.md §5 — R-4-603 — v2.4 安全加固

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use taiji_harness::DefaultHarness;
use taiji_tool_bus::{ToolRef, ToolRegistry, ToolRegistryItem};
use taiji_types::harness::Harness;
use taiji_types::permission::{GateCommand, GuardLevel, PermissionBehavior, PermissionRule};

// ── Mock tools ──

struct ReadTool;
struct WriteTool;

#[async_trait]
impl ToolRegistryItem for ReadTool {
    fn name(&self) -> &str { "read_file" }
    async fn description(&self) -> Result<String, String> { Ok("Read a file".into()) }
    fn input_schema(&self) -> Value { serde_json::json!({"type": "object"}) }
    fn is_readonly(&self) -> bool { true }
}

#[async_trait]
impl ToolRegistryItem for WriteTool {
    fn name(&self) -> &str { "write_file" }
    async fn description(&self) -> Result<String, String> { Ok("Write a file".into()) }
    fn input_schema(&self) -> Value { serde_json::json!({"type": "object"}) }
    fn is_readonly(&self) -> bool { false }
}

// ── Custom harnesses for controlled testing ──

struct AllowHarness;
struct DenyHarness;
struct AskHarness;

impl Harness for AllowHarness {
    async fn check(&self, _tool_name: &str, _input: &Value) -> GateCommand { GateCommand::Allow }
    fn add_rule(&mut self, _rule: PermissionRule) {}
    fn guard_level(&self) -> GuardLevel { GuardLevel::Default }
    fn set_guard_level(&mut self, _mode: GuardLevel) {}
}

impl Harness for DenyHarness {
    async fn check(&self, _tool_name: &str, _input: &Value) -> GateCommand { GateCommand::Deny }
    fn add_rule(&mut self, _rule: PermissionRule) {}
    fn guard_level(&self) -> GuardLevel { GuardLevel::Default }
    fn set_guard_level(&mut self, _mode: GuardLevel) {}
}

impl Harness for AskHarness {
    async fn check(&self, _tool_name: &str, _input: &Value) -> GateCommand { GateCommand::Ask { suggested_rules: vec![] } }
    fn add_rule(&mut self, _rule: PermissionRule) {}
    fn guard_level(&self) -> GuardLevel { GuardLevel::Default }
    fn set_guard_level(&mut self, _mode: GuardLevel) {}
}

// ══════════════════════════════════════════════════════════════
// Tests: ToolBus → Custom Harness
// ══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_allow_harness_execute_success() {
    let mut registry = ToolRegistry::<dyn ToolRegistryItem>::new();
    registry.register_tool(Arc::new(WriteTool) as ToolRef<dyn ToolRegistryItem>);
    let tool = registry.get_tool("write_file").unwrap();

    let result = registry
        .execute("write_file", serde_json::json!({}), &tool, &AllowHarness)
        .await;

    assert!(result.is_ok(), "AllowHarness should permit execution: {:?}", result);
    assert!(result.unwrap().success);
}

#[tokio::test]
async fn test_deny_harness_returns_permission_denied() {
    let mut registry = ToolRegistry::<dyn ToolRegistryItem>::new();
    registry.register_tool(Arc::new(WriteTool) as ToolRef<dyn ToolRegistryItem>);
    let tool = registry.get_tool("write_file").unwrap();

    let result = registry
        .execute("write_file", serde_json::json!({}), &tool, &DenyHarness)
        .await;

    match result {
        Err(taiji_tool_bus::ToolBusError::PermissionDenied(_)) => {}
        other => panic!("expected PermissionDenied, got {:?}", other),
    }
}

#[tokio::test]
async fn test_ask_harness_returns_needs_confirmation() {
    let mut registry = ToolRegistry::<dyn ToolRegistryItem>::new();
    registry.register_tool(Arc::new(WriteTool) as ToolRef<dyn ToolRegistryItem>);
    let tool = registry.get_tool("write_file").unwrap();

    let result = registry
        .execute("write_file", serde_json::json!({}), &tool, &AskHarness)
        .await;

    match result {
        Err(taiji_tool_bus::ToolBusError::NeedsConfirmation(_)) => {}
        other => panic!("expected NeedsConfirmation, got {:?}", other),
    }
}

#[tokio::test]
async fn test_tool_not_found_before_harness() {
    let mut registry = ToolRegistry::<dyn ToolRegistryItem>::new();
    registry.register_tool(Arc::new(ReadTool) as ToolRef<dyn ToolRegistryItem>);
    let tool = registry.get_tool("read_file").unwrap();

    let result = registry
        .execute("nonexistent", serde_json::json!({}), &tool, &AllowHarness)
        .await;

    match result {
        Err(taiji_tool_bus::ToolBusError::ToolNotFound(name)) => {
            assert_eq!(name, "nonexistent");
        }
        other => panic!("expected ToolNotFound, got {:?}", other),
    }
}

// ══════════════════════════════════════════════════════════════
// Tests: ToolBus → DefaultHarness (6-stage pipeline)
// ══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_default_harness_deny_rule_blocks_write() {
    let mut registry = ToolRegistry::<dyn ToolRegistryItem>::new();
    registry.register_tool(Arc::new(WriteTool) as ToolRef<dyn ToolRegistryItem>);
    let tool = registry.get_tool("write_file").unwrap();

    let mut harness = DefaultHarness::new();
    harness.add_rule(PermissionRule {
        tool_name: "write_file".into(),
        rule_content: None,
        behavior: PermissionBehavior::Deny,
        source: "test".into(),
    });

    let result = registry
        .execute("write_file", serde_json::json!({}), &tool, &harness)
        .await;

    match result {
        Err(taiji_tool_bus::ToolBusError::PermissionDenied(msg)) => {
            assert!(msg.contains("write_file"));
        }
        other => panic!("expected PermissionDenied, got {:?}", other),
    }
}

#[tokio::test]
async fn test_default_harness_allow_rule_permits_write() {
    let mut registry = ToolRegistry::<dyn ToolRegistryItem>::new();
    registry.register_tool(Arc::new(WriteTool) as ToolRef<dyn ToolRegistryItem>);
    let tool = registry.get_tool("write_file").unwrap();

    let mut harness = DefaultHarness::new();
    // Allow rule for write_file
    harness.add_rule(PermissionRule {
        tool_name: "write_file".into(),
        rule_content: None,
        behavior: PermissionBehavior::Allow,
        source: "test".into(),
    });

    let result = registry
        .execute("write_file", serde_json::json!({}), &tool, &harness)
        .await;

    assert!(result.is_ok(), "Allow rule should permit write: {:?}", result);
}

#[tokio::test]
async fn test_default_harness_no_rule_default_mode_asks() {
    // With no matching rules in Default mode, harness should Ask (mode fallback)
    let mut registry = ToolRegistry::<dyn ToolRegistryItem>::new();
    registry.register_tool(Arc::new(WriteTool) as ToolRef<dyn ToolRegistryItem>);
    let tool = registry.get_tool("write_file").unwrap();

    let harness = DefaultHarness::new(); // Default mode, no rules

    let result = registry
        .execute("write_file", serde_json::json!({}), &tool, &harness)
        .await;

    // In Default mode with no rules matching → Ask fallback
    match result {
        Err(taiji_tool_bus::ToolBusError::NeedsConfirmation(_)) => {}
        other => panic!("expected NeedsConfirmation for default mode fallback, got {:?}", other),
    }
}

// ══════════════════════════════════════════════════════════════
// Tests: ToolRegistry operations
// ══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_register_override_does_not_duplicate() {
    let mut registry = ToolRegistry::<dyn ToolRegistryItem>::new();
    registry.register_tool(Arc::new(ReadTool) as ToolRef<dyn ToolRegistryItem>);

    struct ReadToolV2;
    #[async_trait]
    impl ToolRegistryItem for ReadToolV2 {
        fn name(&self) -> &str { "read_file" }
        async fn description(&self) -> Result<String, String> { Ok("v2".into()) }
        fn input_schema(&self) -> Value { serde_json::json!({}) }
    }
    registry.register_tool(Arc::new(ReadToolV2) as ToolRef<dyn ToolRegistryItem>);

    let names = registry.get_tool_names();
    let count = names.iter().filter(|n| *n == "read_file").count();
    assert_eq!(count, 1, "override should not create duplicates");
}

#[tokio::test]
async fn test_unregister_then_re_register() {
    let mut registry = ToolRegistry::<dyn ToolRegistryItem>::new();
    registry.register_tool(Arc::new(ReadTool) as ToolRef<dyn ToolRegistryItem>);
    registry.unregister_tool("read_file");
    assert!(registry.get_tool_names().is_empty());

    registry.register_tool(Arc::new(ReadTool) as ToolRef<dyn ToolRegistryItem>);
    assert_eq!(registry.get_tool_names().len(), 1);
}
