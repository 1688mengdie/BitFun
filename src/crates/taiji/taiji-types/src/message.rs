//! 消息类型 — 传音符（message-bus）通信协议。
//!
//! 参考源：
//! - ContentBlock → agentscope message/_block.py:11-215
//! - Message<T> → agentscope message/_base.py:67-114 Msg 类
//! - TransportEvent → BitFun emit_generic(event_name, payload) 模式

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── ContentBlock 体系（参考 agentscope _block.py:11-215） ──

/// 文本块。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextBlock {
    pub text: String,
}

/// 思考块（模型推理过程）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThinkingBlock {
    pub thinking: String,
}

/// 提示块（给 LLM 的指令或提示）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HintBlock {
    pub hint: String,
}

/// 多媒体数据块（图片/音频/视频）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataBlock {
    pub mime_type: String,
    pub data: String,
}

/// 工具调用请求。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallBlock {
    pub tool_name: String,
    pub args: serde_json::Value,
}

/// 工具调用结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResultBlock {
    pub tool_name: String,
    pub success: bool,
    pub output: serde_json::Value,
}

/// 内容块类型联合（参考 agentscope ContentBlock type alias, _block.py:219-226）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text(TextBlock),
    #[serde(rename = "thinking")]
    Thinking(ThinkingBlock),
    #[serde(rename = "hint")]
    Hint(HintBlock),
    #[serde(rename = "data")]
    Data(DataBlock),
    #[serde(rename = "tool_call")]
    ToolCall(ToolCallBlock),
    #[serde(rename = "tool_result")]
    ToolResult(ToolResultBlock),
}

// ── 消息优先级 ──

/// 消息优先级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum Priority {
    Low = 0,
    #[default]
    Normal = 1,
    High = 2,
    Critical = 3,
}

// ── 消息（参考 agentscope Msg, _base.py:67-114） ──

/// 消息 — 传音符传输单元。
///
/// 参考 agentscope Msg 类（_base.py:67-114），保留核心字段
/// name/content/role/id/metadata/created_at。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// 发送者名称。
    pub name: String,
    /// 消息内容（ContentBlock 列表）。
    pub content: Vec<ContentBlock>,
    /// 角色（user/assistant/system）。
    pub role: String,
    /// 消息唯一标识。
    pub id: Uuid,
    /// 所属主题/频道。
    pub topic: String,
    /// 消息优先级。
    pub priority: Priority,
    /// 元数据。
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
    /// 消息创建时间（UTC）。
    pub created_at: DateTime<Utc>,
}
