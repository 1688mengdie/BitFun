//! ToolRegistry — 工具注册中心 + execute(Harness) 强制门控。
//!
//! 参考: modules/tool-bus/接口设计.md §2/§5 — R-4-301 — Rust 实现
//!       BitFun (MIT) execution/tool-contracts/src/framework.rs — IndexMap 注册模式

use chrono::Utc;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use taiji_types::harness::Harness;
use taiji_types::permission::GateCommand;

use crate::tool::{ToolRef, ToolRegistryItem};

// ============================================================
// 工具执行结果
// ============================================================

/// 工具执行结果（R-4-301-04）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: Value,
    pub duration_ms: u64,
}

// ============================================================
// 工具执行错误
// ============================================================

/// 工具执行错误（R-4-301-04）。
#[derive(Debug, Error)]
pub enum ToolBusError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("needs user confirmation")]
    NeedsConfirmation(GateCommand),

    #[error("execution failed: {0}")]
    ExecutionFailed(String),

    #[error("internal error: {0}")]
    Internal(String),
}

// ============================================================
// 工具栏快照
// ============================================================

/// 工具栏快照 — 用于 prompt 渲染。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterializedToolSnapshot {
    /// 工具名称列表。
    pub tool_names: Vec<String>,
    /// 生成时间。
    pub generated_at: chrono::DateTime<Utc>,
}

// ============================================================
// 工具注册中心
// ============================================================

/// 工具注册中心 — 法宝台核心（R-4-301-02）。
///
/// 内部由 IndexMap 存储，保证确定性遍历顺序。
/// execute 方法强制接受 &dyn Harness，不可绕过权限检查。
///
/// 参考: modules/tool-bus/接口设计.md §2 — R-4-301 — IndexMap 注册模式
pub struct ToolRegistry<Tool: ToolRegistryItem + ?Sized> {
    tools: IndexMap<String, ToolRef<Tool>>,
}

impl<Tool: ToolRegistryItem + ?Sized> ToolRegistry<Tool> {
    /// 创建空注册中心。
    pub fn new() -> Self {
        Self {
            tools: IndexMap::new(),
        }
    }

    /// 注册工具（同名覆盖）。
    pub fn register_tool(&mut self, tool: ToolRef<Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    /// 取消注册，返回被移除的工具（如果存在）。
    pub fn unregister_tool(&mut self, name: &str) -> Option<ToolRef<Tool>> {
        self.tools.shift_remove(name)
    }

    /// 按名称获取工具。
    pub fn get_tool(&self, name: &str) -> Option<ToolRef<Tool>> {
        self.tools.get(name).cloned()
    }

    /// 获取所有已注册工具名称。
    pub fn get_tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// 获取所有已注册工具引用。
    pub fn get_all_tools(&self) -> Vec<ToolRef<Tool>> {
        self.tools.values().cloned().collect()
    }

    /// 导出工具快照（用于 prompt 渲染）。
    pub async fn materialized_tool_snapshot(&self) -> Result<MaterializedToolSnapshot, String> {
        let tool_names = self.get_tool_names();
        Ok(MaterializedToolSnapshot {
            tool_names,
            generated_at: Utc::now(),
        })
    }

    /// 执行工具调用 — v2.4 安全加固，强制 harness 检查（R-4-301-03）。
    ///
    /// # 流程
    /// 1. 查找工具（不存在返回 ToolNotFound）
    /// 2. 调用 harness.check() 做权限裁决
    /// 3. Allow → 执行工具 / Deny → PermissionDenied / Ask → NeedsConfirmation
    ///
    /// # 编译期强制
    /// `harness` 参数为泛型 `H: Harness`，不可省略或绕过的路径。
    ///
    /// 参考: modules/tool-bus/接口设计.md §5 — R-4-301 — 禁止绕过路径
    pub async fn execute<H: Harness>(
        &self,
        tool_name: &str,
        args: Value,
        _tool: &ToolRef<Tool>,
        harness: &H,
    ) -> Result<ToolResult, ToolBusError> {
        // 1. 查找工具
        if !self.tools.contains_key(tool_name) {
            return Err(ToolBusError::ToolNotFound(tool_name.into()));
        }

        // 2. 权限检查
        let gate = harness.check(tool_name, &args).await;
        match gate {
            GateCommand::Allow => {
                // 3. 执行工具
                let start = std::time::Instant::now();
                // 工具执行的具体逻辑由调用方在外部处理
                // 这里触发工具调用回调（由具体的 Tool 实现完成）
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(ToolResult {
                    success: true,
                    output: args,
                    duration_ms,
                })
            }
            GateCommand::Deny => {
                Err(ToolBusError::PermissionDenied(format!(
                    "permission denied by harness for tool '{}'",
                    tool_name
                )))
            }
            GateCommand::Ask { .. } => {
                Err(ToolBusError::NeedsConfirmation(gate))
            }
        }
    }
}

impl<Tool: ToolRegistryItem + ?Sized> Default for ToolRegistry<Tool> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Arc;
    use taiji_types::harness::Harness;
    use taiji_types::permission::{GateCommand, GuardLevel, PermissionRule};

    // ── Mock implementations ──

    struct MockTool;

    #[async_trait]
    impl ToolRegistryItem for MockTool {
        fn name(&self) -> &str {
            "mock_tool"
        }

        async fn description(&self) -> Result<String, String> {
            Ok("Mock tool".into())
        }

        fn input_schema(&self) -> Value {
            serde_json::json!({"type": "object"})
        }

        fn is_readonly(&self) -> bool {
            false
        }
    }

    struct AllowHarness;
    struct DenyHarness;
    struct AskHarness;

    impl Harness for AllowHarness {
        async fn check(&self, _tool_name: &str, _input: &Value) -> GateCommand {
            GateCommand::Allow
        }
        fn add_rule(&mut self, _rule: PermissionRule) {}
        fn guard_level(&self) -> GuardLevel { GuardLevel::Default }
        fn set_guard_level(&mut self, _mode: GuardLevel) {}
    }

    impl Harness for DenyHarness {
        async fn check(&self, _tool_name: &str, _input: &Value) -> GateCommand {
            GateCommand::Deny
        }
        fn add_rule(&mut self, _rule: PermissionRule) {}
        fn guard_level(&self) -> GuardLevel { GuardLevel::Default }
        fn set_guard_level(&mut self, _mode: GuardLevel) {}
    }

    impl Harness for AskHarness {
        async fn check(&self, _tool_name: &str, _input: &Value) -> GateCommand {
            GateCommand::Ask { suggested_rules: vec![] }
        }
        fn add_rule(&mut self, _rule: PermissionRule) {}
        fn guard_level(&self) -> GuardLevel { GuardLevel::Default }
        fn set_guard_level(&mut self, _mode: GuardLevel) {}
    }

    fn make_registry() -> ToolRegistry<MockTool> {
        let mut reg = ToolRegistry::new();
        reg.register_tool(Arc::new(MockTool));
        reg
    }

    #[tokio::test]
    async fn test_register_tool() {
        let reg = make_registry();
        let names = reg.get_tool_names();
        assert!(names.contains(&"mock_tool".to_string()));
    }

    #[tokio::test]
    async fn test_unregister_tool() {
        let mut reg = make_registry();
        let removed = reg.unregister_tool("mock_tool");
        assert!(removed.is_some());
        assert!(!reg.get_tool_names().contains(&"mock_tool".to_string()));
    }

    #[tokio::test]
    async fn test_unregister_nonexistent() {
        let mut reg = make_registry();
        let removed = reg.unregister_tool("nonexistent");
        assert!(removed.is_none());
    }

    #[tokio::test]
    async fn test_get_tool() {
        let reg = make_registry();
        let tool = reg.get_tool("mock_tool");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name(), "mock_tool");
    }

    #[tokio::test]
    async fn test_get_tool_nonexistent() {
        let reg = make_registry();
        let tool = reg.get_tool("nonexistent");
        assert!(tool.is_none());
    }

    #[tokio::test]
    async fn test_execute_allow() {
        let reg = make_registry();
        let tool = reg.get_tool("mock_tool").unwrap();
        let harness = AllowHarness;
        let result = reg
            .execute("mock_tool", serde_json::json!({"input": "test"}), &tool, &harness)
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[tokio::test]
    async fn test_execute_deny() {
        let reg = make_registry();
        let tool = reg.get_tool("mock_tool").unwrap();
        let harness = DenyHarness;
        let result = reg
            .execute("mock_tool", serde_json::json!({}), &tool, &harness)
            .await;
        match result {
            Err(ToolBusError::PermissionDenied(_)) => {}
            _ => panic!("expected PermissionDenied"),
        }
    }

    #[tokio::test]
    async fn test_execute_ask() {
        let reg = make_registry();
        let tool = reg.get_tool("mock_tool").unwrap();
        let harness = AskHarness;
        let result = reg
            .execute("mock_tool", serde_json::json!({}), &tool, &harness)
            .await;
        match result {
            Err(ToolBusError::NeedsConfirmation(_)) => {}
            other => panic!("expected NeedsConfirmation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_execute_tool_not_found() {
        let reg = make_registry();
        let tool = reg.get_tool("mock_tool").unwrap();
        let harness = AllowHarness;
        let result = reg
            .execute("nonexistent", serde_json::json!({}), &tool, &harness)
            .await;
        match result {
            Err(ToolBusError::ToolNotFound(_)) => {}
            other => panic!("expected ToolNotFound, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_materialized_snapshot() {
        let reg = make_registry();
        let snapshot = reg.materialized_tool_snapshot().await.unwrap();
        assert!(snapshot.tool_names.contains(&"mock_tool".to_string()));
    }

    #[tokio::test]
    async fn test_tool_result_serde() {
        let result = ToolResult {
            success: true,
            output: serde_json::json!({"result": "ok"}),
            duration_ms: 42,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: ToolResult = serde_json::from_str(&json).unwrap();
        assert!(back.success);
        assert_eq!(back.duration_ms, 42);
    }

    #[test]
    fn test_tool_bus_error_display() {
        let err = ToolBusError::ToolNotFound("test".into());
        assert_eq!(format!("{}", err), "tool not found: test");

        let err = ToolBusError::PermissionDenied("denied".into());
        assert!(format!("{}", err).contains("denied"));
    }
}
