use super::unified::{UnifiedResponse, UnifiedTokenUsage, UnifiedToolCall};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PromptTokensDetails {
    cached_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
    prompt_tokens_details: Option<PromptTokensDetails>,
}

impl From<OpenAIUsage> for UnifiedTokenUsage {
    fn from(usage: OpenAIUsage) -> Self {
        Self {
            prompt_token_count: usage.prompt_tokens,
            candidates_token_count: usage.completion_tokens,
            total_token_count: usage.total_tokens,
            cached_content_token_count: if let Some(prompt_tokens_details) =
                usage.prompt_tokens_details
            {
                Some(prompt_tokens_details.cached_tokens)
            } else {
                None
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct Choice {
    #[allow(dead_code)]
    index: usize,
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    #[allow(dead_code)]
    role: Option<String>,
    reasoning_content: Option<String>,
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Deserialize, Clone)]
struct OpenAIToolCall {
    #[allow(dead_code)]
    index: usize,
    #[allow(dead_code)]
    id: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    tool_type: Option<String>,
    function: Option<FunctionCall>,
}

impl From<OpenAIToolCall> for UnifiedToolCall {
    fn from(tool_call: OpenAIToolCall) -> Self {
        Self {
            id: tool_call.id,
            name: tool_call.function.as_ref().and_then(|f| f.name.clone()),
            arguments: tool_call
                .function
                .as_ref()
                .and_then(|f| f.arguments.clone()),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
struct FunctionCall {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAISSEData {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    created: u64,
    #[allow(dead_code)]
    model: String,
    choices: Vec<Choice>,
    usage: Option<OpenAIUsage>,
}

impl From<OpenAISSEData> for UnifiedResponse {
    fn from(data: OpenAISSEData) -> Self {
        let choices0 = data.choices.get(0).unwrap();
        let text = choices0.delta.content.clone();
        let reasoning_content = choices0.delta.reasoning_content.clone();
        let finish_reason = choices0.finish_reason.clone();
        let tool_call = choices0.delta.tool_calls.as_ref().and_then(|tool_calls| {
            tool_calls
                .get(0)
                .map(|tool_call| UnifiedToolCall::from(tool_call.clone()))
        });
        Self {
            text,
            reasoning_content,
            thinking_signature: None,
            tool_call,
            usage: data.usage.map(|usage| usage.into()),
            finish_reason,
        }
    }
}
