use super::tool::ToolCall;
use serde::{Deserialize, Serialize};

/// Internal message representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String, // "user", "assistant", "tool", "system"
    pub content: Option<String>,
    /// Reasoning content (for interleaved thinking mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Signature for Anthropic extended thinking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    pub fn user(content: String) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(content),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant(content: String) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Some(content),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant_with_tools(tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: None,
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
            name: None,
        }
    }

    pub fn system(content: String) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(content),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
}
