//! Agent 事件类型 — 流式回复产出的事件协议。

use serde::{Deserialize, Serialize};

/// Agent 流式回复过程中产出的事件。
///
/// 对应 AgentTrait::reply_stream() 的输出流。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    /// 文本片段。
    Text { content: String },
    /// 工具调用请求。
    ToolCall { name: String, args: serde_json::Value },
    /// 工具调用结果。
    ToolResult { name: String, result: serde_json::Value },
    /// 回复完成。
    Done { final_msg: String },
    /// 错误信息。
    Error { message: String },
}
