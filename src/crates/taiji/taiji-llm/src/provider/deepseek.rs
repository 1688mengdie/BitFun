use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::{ChatMessage, ChatResponse, ChatStream, LlmClient, LlmConfig, Role, Usage};
use crate::types::ChatChunk;

// ── Provider ────────────────────────────────────────────────────────────

/// DeepSeek provider — OpenAI 兼容 API。
///
/// Endpoint: `{base_url}/chat/completions`
/// 默认 base_url: `https://api.deepseek.com/v1`
pub struct DeepSeekClient {
    client: Client,
}

impl DeepSeekClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    fn base_url(config: &LlmConfig) -> &str {
        config
            .base_url
            .as_deref()
            .unwrap_or("https://api.deepseek.com/v1")
    }

    fn api_key(config: &LlmConfig) -> Result<String> {
        if let Some(key) = &config.api_key {
            return Ok(key.clone());
        }
        std::env::var("DEEPSEEK_API_KEY").map_err(|_| anyhow!("DEEPSEEK_API_KEY not set"))
    }
}

// ── 内部请求/响应类型（OpenAI 兼容格式）─────────────────────────────────

#[derive(Serialize)]
struct DeepSeekRequest<'a> {
    model: &'a str,
    messages: Vec<DeepSeekMessage<'a>>,
    temperature: f32,
    max_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Serialize)]
struct DeepSeekMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct DeepSeekResponse {
    choices: Vec<DeepSeekChoice>,
    usage: Option<DeepSeekUsage>,
}

#[derive(Deserialize)]
struct DeepSeekChoice {
    message: Option<DeepSeekMessageResp>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct DeepSeekMessageResp {
    content: Option<String>,
}

#[derive(Deserialize)]
struct DeepSeekUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

// ── LlmClient 实现 ─────────────────────────────────────────────────────

fn to_role_str(role: &Role) -> &str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

#[async_trait]
impl LlmClient for DeepSeekClient {
    async fn chat(&self, messages: &[ChatMessage], config: &LlmConfig) -> Result<ChatResponse> {
        let api_key = Self::api_key(config)?;
        let base_url = Self::base_url(config);
        let url = format!("{}/chat/completions", base_url);

        let ds_messages: Vec<DeepSeekMessage> = messages
            .iter()
            .map(|m| DeepSeekMessage {
                role: to_role_str(&m.role),
                content: &m.content,
            })
            .collect();

        let body = DeepSeekRequest {
            model: &config.model,
            messages: ds_messages,
            temperature: config.temperature,
            max_tokens: config.max_tokens,
            stream: None,
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("DeepSeek API error {}: {}", status, text));
        }

        let ds_resp: DeepSeekResponse = resp.json().await?;

        let choice = ds_resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("no choices in DeepSeek response"))?;

        let content = choice.message.and_then(|m| m.content).unwrap_or_default();

        let usage = ds_resp.usage.map_or(Usage::default(), |u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(ChatResponse {
            content,
            usage,
            finish_reason: choice.finish_reason.unwrap_or_else(|| "stop".into()),
        })
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        config: &LlmConfig,
    ) -> Result<ChatStream> {
        let api_key = Self::api_key(config)?;
        let base_url = Self::base_url(config).to_string();
        let url = format!("{}/chat/completions", base_url);

        let ds_messages: Vec<DeepSeekMessage> = messages
            .iter()
            .map(|m| DeepSeekMessage {
                role: to_role_str(&m.role),
                content: &m.content,
            })
            .collect();

        let body = DeepSeekRequest {
            model: &config.model,
            messages: ds_messages,
            temperature: config.temperature,
            max_tokens: config.max_tokens,
            stream: Some(true),
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("DeepSeek API error {}: {}", status, text));
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
                                finish_reason: Some("stop".into()),
                            }),
                            stream,
                        ));
                    }

                    match serde_json::from_str::<Value>(data) {
                        Ok(val) => {
                            let delta_text = val
                                .pointer("/choices/0/delta/content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let finish_reason = val
                                .pointer("/choices/0/finish_reason")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());

                            return Some((
                                Ok(ChatChunk {
                                    delta: delta_text,
                                    done: finish_reason.is_some(),
                                    finish_reason,
                                }),
                                stream,
                            ));
                        }
                        Err(_) => continue,
                    }
                }
            }
        });

        Ok(Box::pin(mapped))
    }
}

impl Default for DeepSeekClient {
    fn default() -> Self {
        Self::new()
    }
}
