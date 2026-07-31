//! ToolRegistryItem trait — 工具注册项核心接口。
//!
//! 参考: BitFun (MIT) execution/tool-contracts/src/framework.rs:672-719 — R-4-301 — Rust trait 翻译实现

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// 工具可见性策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolExposure {
    /// 直接暴露给 LLM。
    Direct,
    /// 需要 GetToolSpec 懒加载。
    Deferred,
}

/// 工具引用包装（线程安全）。
pub type ToolRef<T> = Arc<T>;

/// 工具注册项 trait — 法宝台核心接口（R-4-301-01）。
///
/// 参考: BitFun (MIT) execution/tool-contracts/src/framework.rs:672-719 — R-4-301 — Rust trait 翻译
#[async_trait]
pub trait ToolRegistryItem: Send + Sync {
    /// 工具名称。
    fn name(&self) -> &str;

    /// 工具描述。
    async fn description(&self) -> Result<String, String>;

    /// 输入 JSON Schema。
    fn input_schema(&self) -> Value;

    /// 简短描述（默认取名称）。
    fn short_description(&self) -> String {
        self.name().to_string()
    }

    /// 默认可见性（默认 Direct）。
    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Direct
    }

    /// 是否只读（默认 false）。
    fn is_readonly(&self) -> bool {
        false
    }

    /// 是否并发安全（只读工具默认 true）。
    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        self.is_readonly()
    }

    /// 是否启用（默认 true）。
    async fn is_enabled(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockTool;

    #[async_trait]
    impl ToolRegistryItem for MockTool {
        fn name(&self) -> &str {
            "mock_tool"
        }

        async fn description(&self) -> Result<String, String> {
            Ok("Mock tool for testing".into())
        }

        fn input_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" }
                }
            })
        }
    }

    #[tokio::test]
    async fn test_tool_name() {
        let tool = MockTool;
        assert_eq!(tool.name(), "mock_tool");
    }

    #[tokio::test]
    async fn test_tool_description() {
        let tool = MockTool;
        let desc = tool.description().await.unwrap();
        assert_eq!(desc, "Mock tool for testing");
    }

    #[tokio::test]
    async fn test_default_exposure() {
        let tool = MockTool;
        assert_eq!(tool.default_exposure(), ToolExposure::Direct);
    }

    #[tokio::test]
    async fn test_default_readonly() {
        let tool = MockTool;
        assert!(!tool.is_readonly());
    }

    #[tokio::test]
    async fn test_default_enabled() {
        let tool = MockTool;
        assert!(tool.is_enabled().await);
    }

    #[tokio::test]
    async fn test_tool_exposure_serde() {
        for exp in &[ToolExposure::Direct, ToolExposure::Deferred] {
            let json = serde_json::to_string(exp).unwrap();
            let back: ToolExposure = serde_json::from_str(&json).unwrap();
            assert_eq!(*exp, back);
        }
    }
}
