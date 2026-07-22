use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::{ChatMessage, ChatResponse, ChatStream, LlmClient, LlmConfig, Role, Usage};
use crate::types::ChatChunk;

// ── Provider ────────────────────────────────────────────────────────────

pub struct ClaudeClient {
    client: Client,
}

impl ClaudeClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    fn base_url(config: &LlmConfig) -> &str {
        config
            .base_url
            .as_deref()
            .unwrap_or("https://api.anthropic.com/v1")
    }

    fn api_key(config: &LlmConfig) -> Result<String> {
        if let Some(key) = &config.api_key {
            return Ok(key.clone());
        }
        std::env::var("ANTHROPIC_API_KEY").map_err(|_| anyhow!("ANTHROPIC_API_KEY not set"))
    }

    /// 从消息列表中提取 system 消息（Claude 的 system 是顶层字段）。
    fn extract_system_and_messages<'a>(
        messages: &'a [ChatMessage],
    ) -> (Option<String>, Vec<ClaudeMessage<'a>>) {
        let mut system = None;
        let mut conversation = Vec::new();

        for msg in messages {
            if msg.role == Role::System {
                system = Some(msg.content.clone());
            } else {
                conversation.push(ClaudeMessage {
                    role: claude_role_str(&msg.role),
                    content: &msg.content,
                });
            }
        }

        (system, conversation)
    }
}

fn claude_role_str(role: &Role) -> &str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "user", // 不回退到这里
    }
}

// ── 内部请求/响应类型 ──────────────────────────────────────────────────

#[derive(Serialize)]
struct ClaudeRequest<'a> {
    model: &'a str,
    messages: Vec<ClaudeMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: usize,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Serialize)]
struct ClaudeMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
    usage: ClaudeUsage,
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct ClaudeContent {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    content_type: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct ClaudeUsage {
    input_tokens: usize,
    output_tokens: usize,
}

// ── LlmClient 实现 ─────────────────────────────────────────────────────

#[async_trait]
impl LlmClient for ClaudeClient {
    async fn chat(&self, messages: &[ChatMessage], config: &LlmConfig) -> Result<ChatResponse> {
        let api_key = Self::api_key(config)?;
        let base_url = Self::base_url(config);
        let url = format!("{}/messages", base_url);

        let (system, claude_messages) = Self::extract_system_and_messages(messages);

        let body = ClaudeRequest {
            model: &config.model,
            messages: claude_messages,
            system,
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            stream: None,
        };

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Claude API error {}: {}", status, text));
        }

        let claude_resp: ClaudeResponse = resp.json().await?;

        let content = claude_resp
            .content
            .iter()
            .filter_map(|c| c.text.as_deref())
            .collect::<Vec<_>>()
            .join("");

        Ok(ChatResponse {
            content,
            usage: Usage {
                prompt_tokens: claude_resp.usage.input_tokens,
                completion_tokens: claude_resp.usage.output_tokens,
                total_tokens: claude_resp.usage.input_tokens + claude_resp.usage.output_tokens,
            },
            finish_reason: claude_resp.stop_reason.unwrap_or_else(|| "end_turn".into()),
        })
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        config: &LlmConfig,
    ) -> Result<ChatStream> {
        let api_key = Self::api_key(config)?;
        let base_url = Self::base_url(config).to_string();
        let url = format!("{}/messages", base_url);

        let (system, claude_messages) = Self::extract_system_and_messages(messages);

        let body = ClaudeRequest {
            model: &config.model,
            messages: claude_messages,
            system,
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            stream: Some(true),
        };

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Claude API error {}: {}", status, text));
        }

        let stream = resp.bytes_stream();
        let mapped = futures::stream::unfold(stream, move |mut stream| async move {
            use futures::StreamExt;
            loop {
                let chunk = stream.next().await?;
                let bytes = match chunk {
                    Ok(b) => b,
                    Err(e) => {
                        return Some((Err(anyhow!("stream error: {}", e)), stream));
                    }
                };
                let text = String::from_utf8_lossy(&bytes);

                for line in text.lines() {
                    let line = line.trim();
                    if line.is_empty() || !line.starts_with("data: ") {
                        continue;
                    }
                    let data = &line[6..];
                    if data == "[DONE]" {
                        return Some((
                            Ok(ChatChunk {
                                delta: String::new(),
                                done: true,
                                finish_reason: Some("end_turn".into()),
                            }),
                            stream,
                        ));
                    }

                    match serde_json::from_str::<Value>(data) {
                        Ok(val) => {
                            let event_type =
                                val.pointer("/type").and_then(|v| v.as_str()).unwrap_or("");

                            match event_type {
                                "content_block_delta" => {
                                    let delta_text = val
                                        .pointer("/delta/text")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    return Some((
                                        Ok(ChatChunk {
                                            delta: delta_text,
                                            done: false,
                                            finish_reason: None,
                                        }),
                                        stream,
                                    ));
                                }
                                "message_stop" => {
                                    return Some((
                                        Ok(ChatChunk {
                                            delta: String::new(),
                                            done: true,
                                            finish_reason: Some("end_turn".into()),
                                        }),
                                        stream,
                                    ));
                                }
                                _ => continue,
                            }
                        }
                        Err(_) => continue,
                    }
                }
            }
        });

        Ok(Box::pin(mapped))
    }
}

impl Default for ClaudeClient {
    fn default() -> Self {
        Self::new()
    }
}
