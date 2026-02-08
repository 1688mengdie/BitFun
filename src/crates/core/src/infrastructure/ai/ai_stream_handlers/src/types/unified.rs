use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedToolCall {
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: Option<String>,
}

/// Unified AI response format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedResponse {
    pub text: Option<String>,
    pub reasoning_content: Option<String>,
    /// Signature for Anthropic extended thinking (returned in multi-turn conversations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_signature: Option<String>,
    pub tool_call: Option<UnifiedToolCall>,
    pub usage: Option<UnifiedTokenUsage>,
    pub finish_reason: Option<String>,
}

impl Default for UnifiedResponse {
    fn default() -> Self {
        Self {
            text: None,
            reasoning_content: None,
            thinking_signature: None,
            tool_call: None,
            usage: None,
            finish_reason: None,
        }
    }
}

/// Unified token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedTokenUsage {
    pub prompt_token_count: u32,
    pub candidates_token_count: u32,
    pub total_token_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_content_token_count: Option<u32>,
}
