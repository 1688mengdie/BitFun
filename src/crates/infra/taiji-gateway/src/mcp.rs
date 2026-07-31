//! MCP 协议骨架 — Model Context Protocol。
//!
//! MCP 协议提供 AI 模型与外部工具/资源之间的标准化通信。
//! 当前为骨架实现，完整集成需 rmcp crate（依赖待确认来源）。
//!
//! 设计参考：modules/gateway/接口设计.md §2（ACP 协议流程，MCP 类似）

use serde::{Deserialize, Serialize};

/// MCP 工具定义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    /// 工具名称。
    pub name: String,
    /// 工具描述。
    pub description: String,
    /// 输入 schema（JSON Schema 格式）。
    pub input_schema: serde_json::Value,
}

/// MCP 资源定义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceDefinition {
    /// 资源 URI。
    pub uri: String,
    /// 资源名称。
    pub name: String,
    /// 资源描述。
    pub description: String,
    /// MIME 类型。
    pub mime_type: String,
}

/// MCP 客户端骨架。
///
/// 当前提供基础类型定义。完整实现需要：
/// - `rmcp` crate (v0.8+) 用于 MCP 协议传输
/// - 或基于 JSON-RPC 2.0 的自定义实现
///
/// # 待确认
///
/// `rmcp = "0.8"` 依赖来源需确认：
/// - crate.io 上的 `rmcp` crate 版本和许可证状态
/// - 或是否使用 BitFun 内置的 MCP 传输实现
#[derive(Debug, Default)]
pub struct McpClient {
    /// 已注册的工具列表。
    tools: Vec<McpToolDefinition>,
    /// 已注册的资源列表。
    resources: Vec<McpResourceDefinition>,
}

impl McpClient {
    /// 创建新的 MCP 客户端。
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            resources: Vec::new(),
        }
    }

    /// 注册工具。
    pub fn register_tool(&mut self, tool: McpToolDefinition) {
        self.tools.push(tool);
    }

    /// 注册资源。
    pub fn register_resource(&mut self, resource: McpResourceDefinition) {
        self.resources.push(resource);
    }

    /// 获取已注册的工具列表。
    pub fn tools(&self) -> &[McpToolDefinition] {
        &self.tools
    }

    /// 获取已注册的资源列表。
    pub fn resources(&self) -> &[McpResourceDefinition] {
        &self.resources
    }

    /// 列出所有工具（JSON-RPC "tools/list" 响应对应）。
    pub fn list_tools(&self) -> Vec<McpToolDefinition> {
        self.tools.clone()
    }

    /// 列出所有资源（JSON-RPC "resources/list" 响应对应）。
    pub fn list_resources(&self) -> Vec<McpResourceDefinition> {
        self.resources.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_client_empty() {
        let client = McpClient::new();
        assert!(client.tools().is_empty());
        assert!(client.resources().is_empty());
    }

    #[test]
    fn test_mcp_register_tool() {
        let mut client = McpClient::new();
        client.register_tool(McpToolDefinition {
            name: "get_quote".into(),
            description: "获取实时报价".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string"}
                }
            }),
        });
        assert_eq!(client.tools().len(), 1);
        assert_eq!(client.tools()[0].name, "get_quote");
    }

    #[test]
    fn test_mcp_register_resource() {
        let mut client = McpClient::new();
        client.register_resource(McpResourceDefinition {
            uri: "taiji://bars/rb2501/1m".into(),
            name: "rb2501 1分钟K线".into(),
            description: "螺纹钢 2501 合约 1 分钟 K 线数据".into(),
            mime_type: "application/json".into(),
        });
        assert_eq!(client.resources().len(), 1);
    }

    #[test]
    fn test_mcp_tool_definition_serde() {
        let tool = McpToolDefinition {
            name: "calculate".into(),
            description: "计算器工具".into(),
            input_schema: serde_json::json!({"type": "object"}),
        };
        let json = serde_json::to_string(&tool).unwrap();
        let back: McpToolDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(tool.name, back.name);
    }
}
