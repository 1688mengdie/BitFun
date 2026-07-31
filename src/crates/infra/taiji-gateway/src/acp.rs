//! ACP 协议实现 — Agent Communication Protocol (JSON-RPC 2.0 over stdio NDJSON)。
//!
//! 包含：
//! - `AcpClient` — ACP 客户端，通过 stdio 与子进程通信
//! - `AcpServer` — ACP 服务端 trait
//! - `AcpToolBridge` — ACP 工具桥接定义
//!
//! 设计参考：buzz (Apache 2.0) acp/src/acp.rs:139-218 + BitFun (MIT) interfaces/acp/src/server.rs:18-83

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use uuid::Uuid;

use crate::error::{GatewayError, GatewayResult};

// ============================================================================
// JSON-RPC 2.0 消息类型
// ============================================================================

/// JSON-RPC 2.0 请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// JSON-RPC 2.0 响应。
#[derive(Debug, Clone, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    _id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 错误。
#[derive(Debug, Clone, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(dead_code)]
    _data: Option<Value>,
}

// ============================================================================
// ACP 协议类型
// ============================================================================

/// ACP 初始化请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeRequest {
    pub version: u32,
    pub name: String,
    pub capabilities: Vec<String>,
}

/// ACP 初始化响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResponse {
    pub version: u32,
    pub name: String,
    pub capabilities: Vec<String>,
}

/// ACP 认证请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpAuthenticateRequest {
    pub auth_type: String,
    pub credentials: Value,
}

/// ACP 认证响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpAuthenticateResponse {
    pub agent_id: String,
    pub session_id: String,
}

/// ACP 创建会话请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSessionRequest {
    pub channel_id: Uuid,
    pub mcp_servers: Vec<McpServerConfig>,
    pub model: Option<String>,
}

/// ACP 创建会话响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSessionResponse {
    pub session_id: String,
}

/// ACP 提示词请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptRequest {
    pub session_id: String,
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
}

/// ACP 提示词响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptResponse {
    pub text: String,
    pub stop_reason: StopReason,
    pub usage: Option<Value>,
}

/// ACP 取消通知。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelNotification {
    pub session_id: String,
}

/// ACP 停止原因。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    Error(String),
    Cancelled,
}

/// MCP 服务端配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// ACP 工具桥接定义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpToolBridge {
    /// 外部 Agent 客户端 ID。
    pub client_id: String,
    /// 工具名称（ACP 前缀 "acp__" + agent_name + "__prompt"）。
    pub tool_name: String,
    /// 显示名称。
    pub display_name: String,
    /// 工具描述。
    pub description: String,
    /// 简短描述。
    pub short_description: String,
    /// 是否只读。
    pub read_only: bool,
}

impl AcpToolBridge {
    /// ACP 工具前缀。
    pub const PREFIX: &'static str = "acp__";
    /// ACP 工具后缀。
    pub const SUFFIX: &'static str = "__prompt";

    /// 构造 ACP 工具名称。
    pub fn build_tool_name(client_id: &str) -> String {
        format!("{}{}{}", Self::PREFIX, client_id, Self::SUFFIX)
    }

    /// 从 client_id 解析出原始 Agent 名称。
    pub fn parse_client_id(tool_name: &str) -> Option<String> {
        let stripped = tool_name.strip_prefix(Self::PREFIX)?;
        stripped.strip_suffix(Self::SUFFIX).map(|s| s.to_string())
    }
}

// ============================================================================
// ACP 服务端 trait
// ============================================================================

/// ACP 协议服务端 trait。
///
/// 处理 ACP 客户端发来的 JSON-RPC 2.0 请求。
#[async_trait]
pub trait AcpServer: Send + Sync + 'static {
    /// 初始化：协议版本协商 + 能力声明。
    async fn initialize(&self, request: InitializeRequest) -> GatewayResult<InitializeResponse>;

    /// 认证：外部 Agent 身份验证。
    async fn authenticate(&self, request: AcpAuthenticateRequest) -> GatewayResult<AcpAuthenticateResponse>;

    /// 创建新会话（传入 MCP 配置）。
    async fn new_session(&self, request: NewSessionRequest) -> GatewayResult<NewSessionResponse>;

    /// 发送提示词，返回响应。
    async fn prompt(&self, request: PromptRequest) -> GatewayResult<PromptResponse>;

    /// 取消正在进行的会话。
    async fn cancel(&self, notification: CancelNotification) -> GatewayResult<()>;
}

// ============================================================================
// ACP 客户端
// ============================================================================

/// ACP 协议客户端 — JSON-RPC 2.0 over stdio NDJSON。
///
/// 通过子进程 stdin/stdout 与外部 Agent 进程通信。
pub struct AcpClient {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    reader: BufReader<ChildStdout>,
    next_id: u64,
}

impl AcpClient {
    /// 启动子进程并建立 ACP 连接。
    pub async fn spawn(command: &str, args: &[String]) -> GatewayResult<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(GatewayError::Io)?;

        let stdin = child.stdin.take()
            .ok_or_else(|| GatewayError::Protocol("无法获取子进程 stdin".into()))?;
        let stdout = child.stdout.take()
            .ok_or_else(|| GatewayError::Protocol("无法获取子进程 stdout".into()))?;

        Ok(Self {
            child,
            stdin: BufWriter::new(stdin),
            reader: BufReader::new(stdout),
            next_id: 1,
        })
    }

    /// 发送 JSON-RPC 2.0 请求并等待响应。
    async fn send_request(&mut self, method: &str, params: Option<Value>) -> GatewayResult<Value> {
        let id = self.next_id;
        self.next_id += 1;

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id,
            method: method.into(),
            params,
        };

        // 序列化并发送（NDJSON：每行一个 JSON）
        let line = serde_json::to_string(&request)
            .map_err(GatewayError::Json)?;
        self.stdin.write_all(line.as_bytes()).await
            .map_err(GatewayError::Io)?;
        self.stdin.write_all(b"\n").await
            .map_err(GatewayError::Io)?;
        self.stdin.flush().await
            .map_err(GatewayError::Io)?;

        // 读取响应行
        let mut line = String::new();
        self.reader.read_line(&mut line).await
            .map_err(GatewayError::Io)?;

        if line.trim().is_empty() {
            return Err(GatewayError::Protocol("收到空响应".into()));
        }

        let response: JsonRpcResponse = serde_json::from_str(&line)
            .map_err(GatewayError::Json)?;

        if let Some(err) = response.error {
            return Err(GatewayError::Protocol(format!("ACP 错误 [{}]: {}", err.code, err.message)));
        }

        response.result
            .ok_or_else(|| GatewayError::Protocol("ACP 响应缺少 result 字段".into()))
    }

    /// 初始化：协议版本协商。
    pub async fn initialize(&mut self) -> GatewayResult<(u32, String)> {
        let params = serde_json::json!({
            "version": 1,
            "name": "taiji-gateway",
            "capabilities": ["acp/1.0"],
        });
        let result = self.send_request("initialize", Some(params)).await?;
        let version = result.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let name = result.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        Ok((version, name))
    }

    /// 创建新会话。
    pub async fn session_new(&mut self, channel_id: Uuid) -> GatewayResult<String> {
        let params = serde_json::json!({
            "channel_id": channel_id.to_string(),
            "mcp_servers": [],
        });
        let result = self.send_request("session/new", Some(params)).await?;
        result.get("session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| GatewayError::Protocol("响应缺少 session_id".into()))
    }

    /// 发送提示词，返回停止原因和文本。
    pub async fn session_prompt(&mut self, session_id: &str, prompt: &str) -> GatewayResult<StopReason> {
        let params = serde_json::json!({
            "session_id": session_id,
            "prompt": prompt,
        });
        let result = self.send_request("session/prompt", Some(params)).await?;

        match result.get("stop_reason").and_then(|v| v.as_str()) {
            Some("end_turn") => Ok(StopReason::EndTurn),
            Some("max_tokens") => Ok(StopReason::MaxTokens),
            Some("cancelled") => Ok(StopReason::Cancelled),
            Some(e) => Ok(StopReason::Error(e.to_string())),
            None => Ok(StopReason::EndTurn),
        }
    }

    /// 取消正在进行的会话。
    pub async fn session_cancel(&mut self, session_id: &str) -> GatewayResult<()> {
        let params = serde_json::json!({
            "session_id": session_id,
        });
        self.send_request("session/cancel", Some(params)).await?;
        Ok(())
    }

    /// 获取子进程状态。
    pub fn is_running(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }

    /// 终止子进程。
    pub async fn kill(&mut self) -> GatewayResult<()> {
        self.child.kill().await
            .map_err(GatewayError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acp_tool_bridge_name() {
        let name = AcpToolBridge::build_tool_name("agent_1");
        assert_eq!(name, "acp__agent_1__prompt");
    }

    #[test]
    fn test_acp_tool_bridge_parse() {
        let client_id = AcpToolBridge::parse_client_id("acp__my_agent__prompt");
        assert_eq!(client_id, Some("my_agent".to_string()));
    }

    #[test]
    fn test_acp_tool_bridge_invalid_format() {
        assert!(AcpToolBridge::parse_client_id("invalid").is_none());
        assert!(AcpToolBridge::parse_client_id("acp__missing_suffix").is_none());
    }

    #[test]
    fn test_acp_tool_bridge_roundtrip() {
        let original = "test-agent";
        let tool_name = AcpToolBridge::build_tool_name(original);
        let parsed = AcpToolBridge::parse_client_id(&tool_name);
        assert_eq!(parsed, Some(original.to_string()));
    }

    #[test]
    fn test_stop_reason_variants() {
        match StopReason::EndTurn {
            StopReason::EndTurn => {}  // ok
            _ => panic!("expected EndTurn"),
        }
        match StopReason::MaxTokens {
            StopReason::MaxTokens => {}  // ok
            _ => panic!("expected MaxTokens"),
        }
        match StopReason::Error("test".into()) {
            StopReason::Error(ref s) => assert_eq!(s, "test"),
            _ => panic!("expected Error"),
        }
        match StopReason::Cancelled {
            StopReason::Cancelled => {}  // ok
            _ => panic!("expected Cancelled"),
        }
    }
}
