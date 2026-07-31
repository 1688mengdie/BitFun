//! 参考: 量价时空/Phase-2-派发提示词.md:891 — R-2-506 — taiji-llm LLM 集成

use std::pin::Pin;

use async_trait::async_trait;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use taiji_infra_config::ConfigManager;

use crate::types::{ChatChunk, DecisionOutput};

// ── 核心类型 ──────────────────────────────────────────────────────────

/// 消息角色。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

/// 一条对话消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// LLM 调用配置。
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// 模型名称，如 "gpt-4o", "claude-sonnet-4-20250514", "deepseek-chat"
    pub model: String,
    /// 采样温度 [0.0, 2.0]
    pub temperature: f32,
    /// 最大输出 token 数
    pub max_tokens: usize,
    /// API key（可用环境变量替代）
    pub api_key: Option<String>,
    /// 自定义 base URL（代理 / 兼容 API）
    pub base_url: Option<String>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            model: String::new(),
            temperature: 0.7,
            max_tokens: 4096,
            api_key: None,
            base_url: None,
        }
    }
}

/// Token 用量统计。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

/// LLM 完成响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// 模型生成的完整文本
    pub content: String,
    /// Token 用量
    pub usage: Usage,
    /// 完成原因："stop" | "length" | "tool_calls" | ...
    pub finish_reason: String,
}

/// 流式响应的类型别名。
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<ChatChunk, anyhow::Error>> + Send>>;

/// 流式响应的 mpsc receiver 类型别名。
pub type ChatMpscReceiver = tokio::sync::mpsc::Receiver<Result<ChatChunk, anyhow::Error>>;

// ── LlmClient trait ────────────────────────────────────────────────────

/// LLM 客户端抽象。
///
/// 所有 provider（OpenAI / Claude / DeepSeek）实现此 trait，
/// 上层 Agent 通过此 trait 调用，不依赖具体 provider。
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// 发送非流式对话请求，返回完整响应。
    async fn chat(
        &self,
        messages: &[ChatMessage],
        config: &LlmConfig,
    ) -> Result<ChatResponse, anyhow::Error>;

    /// 发送流式对话请求，返回 SSE 增量流。
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        config: &LlmConfig,
    ) -> Result<ChatStream, anyhow::Error>;

    /// 发送流式对话请求，返回 tokio::sync::mpsc receiver（默认实现）。
    ///
    /// 基于 [`chat_stream`](Self::chat_stream) 的默认实现，
    /// provider 可覆盖此方法以提供原生 mpsc 支持。
    ///
    /// `buffer` 为 mpsc channel 缓冲区大小。
    async fn chat_stream_mpsc(
        &self,
        messages: &[ChatMessage],
        config: &LlmConfig,
        buffer: usize,
    ) -> Result<ChatMpscReceiver, anyhow::Error> {
        use futures::StreamExt;
        let stream = self.chat_stream(messages, config).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(buffer);
        tokio::spawn(async move {
            tokio::pin!(stream);
            while let Some(chunk) = stream.next().await {
                if tx.send(chunk).await.is_err() {
                    break;
                }
            }
        });
        Ok(rx)
    }
}

// ── 辅助函数 ───────────────────────────────────────────────────────────

/// 从 ChatResponse 解析 DecisionOutput。
///
/// 期望响应内容是合法的 DecisionOutput JSON；
/// 如果内容以 ```json 包裹，会自动剥离代码块标记。
pub fn parse_decision_output(response: &ChatResponse) -> Result<DecisionOutput, anyhow::Error> {
    let content = response.content.trim();

    // 剥离可能的 ```json ... ``` 包裹
    let json_str = if let Some(inner) = content.strip_prefix("```json") {
        inner.strip_suffix("```").unwrap_or(inner).trim()
    } else if let Some(inner) = content.strip_prefix("```") {
        inner.strip_suffix("```").unwrap_or(inner).trim()
    } else {
        content
    };

    let decision: DecisionOutput = serde_json::from_str(json_str)?;
    Ok(decision)
}

/// 从 Phase 1 config 读取 LLM 配置，构造 [`LlmConfig`]。
///
/// 读取的配置键：
/// - `chat_model` → `LlmConfig.model`
/// - `llm_api_key` → `LlmConfig.api_key`
/// - `llm_base_url` → `LlmConfig.base_url`
/// - `llm_temperature` → `LlmConfig.temperature`
/// - `llm_max_tokens` → `LlmConfig.max_tokens`
///
/// 不存在的配置键使用 [`LlmConfig::default`] 值。
///
/// # L1 合规
///
/// `config_manager.get()` 是 async 非阻塞调用（文件 I/O + env 读取），
/// 不持有 L1 线程。调用方需确保 ConfigManager 已加载。
pub async fn llm_config_from_config<C: ConfigManager>(config_manager: &C) -> LlmConfig {
    let model: String = config_manager
        .get("chat_model")
        .await
        .unwrap_or_else(|_| "gpt-4o".into());
    let api_key: String = config_manager
        .get("llm_api_key")
        .await
        .unwrap_or_default();
    let base_url: String = config_manager
        .get("llm_base_url")
        .await
        .unwrap_or_default();
    let temperature: f32 = config_manager
        .get("llm_temperature")
        .await
        .unwrap_or(0.7);
    let max_tokens: usize = config_manager
        .get("llm_max_tokens")
        .await
        .unwrap_or(4096);

    LlmConfig {
        model,
        temperature,
        max_tokens,
        api_key: if api_key.is_empty() { None } else { Some(api_key) },
        base_url: if base_url.is_empty() { None } else { Some(base_url) },
    }
}

/// 将 [`ChatStream`] 转换为 tokio::sync::mpsc receiver。
///
/// 内部 spawn 一个 tokio task 将 Stream 的每个 chunk 推入 mpsc channel。
/// `buffer` 为 mpsc channel 缓冲区大小。
///
/// # L1 合规
///
/// 数据搬移在独立的 tokio task 中执行，不阻塞调用方线程。
pub fn chat_stream_to_mpsc(
    stream: ChatStream,
    buffer: usize,
) -> ChatMpscReceiver {
    let (tx, rx) = tokio::sync::mpsc::channel(buffer);
    tokio::spawn(async move {
        use futures::StreamExt;
        let mut stream = stream;
        while let Some(chunk) = stream.next().await {
            if tx.send(chunk).await.is_err() {
                break;
            }
        }
    });
    rx
}

/// 在 tokio::spawn_blocking 中执行阻塞的 LLM 调用。
///
/// # L1 合规
///
/// 将 CPU 密集或阻塞 I/O 的 LLM 调用（如本地模型推理）移至
/// spawn_blocking 线程池，不持有 L1 实时计算线程。
///
/// # 示例
///
/// ```ignore
/// use taiji_llm::client::run_llm_blocking;
///
/// let result = run_llm_blocking(|| {
///     // 阻塞的 LLM 推理调用
///     Ok::<_, anyhow::Error>("推理结果".to_string())
/// }).await.unwrap();
/// ```
pub async fn run_llm_blocking<F, T>(f: F) -> anyhow::Result<T>
where
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking 失败: {}", e))?
}

// ── Mock client（测试用）────────────────────────────────────────────────

/// 测试用的 Mock LLM 客户端，返回预设 JSON。
pub struct MockClient {
    pub preset_response: String,
}

impl MockClient {
    pub fn new(preset_response: impl Into<String>) -> Self {
        Self {
            preset_response: preset_response.into(),
        }
    }

    /// 创建一个返回预设 DecisionOutput 的 MockClient。
    pub fn with_decision(direction: &str, confidence: f64, reasoning: &str) -> Self {
        let decision = DecisionOutput {
            direction: direction.into(),
            confidence,
            reasoning: reasoning.into(),
            key_signals: vec!["mock_signal".into()],
            risks: vec!["mock_risk".into()],
        };
        Self {
            preset_response: serde_json::to_string(&decision).unwrap(),
        }
    }
}

#[async_trait]
impl LlmClient for MockClient {
    async fn chat(
        &self,
        _messages: &[ChatMessage],
        _config: &LlmConfig,
    ) -> Result<ChatResponse, anyhow::Error> {
        Ok(ChatResponse {
            content: self.preset_response.clone(),
            usage: Usage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
            },
            finish_reason: "stop".into(),
        })
    }

    async fn chat_stream(
        &self,
        _messages: &[ChatMessage],
        _config: &LlmConfig,
    ) -> Result<ChatStream, anyhow::Error> {
        let content = self.preset_response.clone();
        let stream = futures::stream::once(async move {
            Ok(ChatChunk {
                delta: content,
                done: true,
                finish_reason: Some("stop".into()),
            })
        });
        Ok(Box::pin(stream))
    }
}

// ── 测试 ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_response_serde_roundtrip() {
        let original = ChatResponse {
            content: "Hello".into(),
            usage: Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
            finish_reason: "stop".into(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: ChatResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.content, "Hello");
        assert_eq!(parsed.usage.total_tokens, 15);
        assert_eq!(parsed.finish_reason, "stop");
    }

    #[tokio::test]
    async fn test_mock_client_chat() {
        let client = MockClient::with_decision("long", 0.85, "test reasoning");
        let messages = vec![ChatMessage::user("测试")];
        let config = LlmConfig::default();

        let response = client.chat(&messages, &config).await.unwrap();
        assert!(response.content.contains("long"));
        assert_eq!(response.finish_reason, "stop");

        let decision = parse_decision_output(&response).unwrap();
        assert_eq!(decision.direction, "long");
        assert_eq!(decision.confidence, 0.85);
        assert_eq!(decision.reasoning, "test reasoning");
    }

    #[tokio::test]
    async fn test_mock_client_chat_stream() {
        let client = MockClient::with_decision("short", 0.72, "stream test");

        let mut stream = client
            .chat_stream(&[ChatMessage::user("测试")], &LlmConfig::default())
            .await
            .unwrap();

        use futures::StreamExt;
        let chunk = stream.next().await.unwrap().unwrap();
        assert!(chunk.done);
        assert!(chunk.delta.contains("short"));
    }

    #[test]
    fn test_parse_decision_output_strips_code_block() {
        let response = ChatResponse {
            content:
                "```json\n{\"direction\":\"hold\",\"confidence\":0.5,\"reasoning\":\"wait\"}\n```"
                    .into(),
            usage: Usage::default(),
            finish_reason: "stop".into(),
        };
        let decision = parse_decision_output(&response).unwrap();
        assert_eq!(decision.direction, "hold");
    }

    #[test]
    fn test_parse_decision_output_plain_json() {
        let response = ChatResponse {
            content: r#"{"direction":"long","confidence":0.9,"reasoning":"strong signal"}"#.into(),
            usage: Usage::default(),
            finish_reason: "stop".into(),
        };
        let decision = parse_decision_output(&response).unwrap();
        assert_eq!(decision.direction, "long");
        assert_eq!(decision.confidence, 0.9);
    }

    #[test]
    fn test_llm_config_default() {
        let config = LlmConfig::default();
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.max_tokens, 4096);
        assert!(config.api_key.is_none());
        assert!(config.base_url.is_none());
    }

    // ── MockConfigManager（测试用）──────────────────────────────────────

    use std::collections::HashMap;
    use std::sync::Arc;
    use taiji_infra_config::{ConfigManager, ConfigResult};

    struct MockConfigManager {
        data: Arc<tokio::sync::RwLock<HashMap<String, serde_json::Value>>>,
    }

    impl MockConfigManager {
        fn new() -> Self {
            let mut map = HashMap::new();
            map.insert("chat_model".into(), serde_json::json!("gpt-4o"));
            map.insert("llm_api_key".into(), serde_json::json!("sk-test-key"));
            map.insert("llm_base_url".into(), serde_json::json!("https://api.example.com"));
            map.insert("llm_temperature".into(), serde_json::json!(0.5));
            map.insert("llm_max_tokens".into(), serde_json::json!(2048));
            Self {
                data: Arc::new(tokio::sync::RwLock::new(map)),
            }
        }
    }

    #[async_trait]
    impl ConfigManager for MockConfigManager {
        async fn load(&mut self) -> ConfigResult<()> {
            Ok(())
        }

        async fn get<T: serde::de::DeserializeOwned + Send>(&self, path: &str) -> ConfigResult<T> {
            let map = self.data.read().await;
            let value = map.get(path).ok_or_else(|| {
                taiji_infra_config::ConfigError::InvalidPath(path.into())
            })?;
            serde_json::from_value(value.clone()).map_err(|e| {
                taiji_infra_config::ConfigError::Serialization(e.to_string())
            })
        }

        async fn set<T: serde::Serialize + Send>(
            &mut self,
            path: &str,
            value: T,
        ) -> ConfigResult<()> {
            let json = serde_json::to_value(value).map_err(|e| {
                taiji_infra_config::ConfigError::Serialization(e.to_string())
            })?;
            self.data.write().await.insert(path.into(), json);
            Ok(())
        }

        async fn reset(&mut self, _path: Option<&str>) -> ConfigResult<()> {
            Ok(())
        }

        async fn validate(&self) -> ConfigResult<Vec<String>> {
            Ok(vec![])
        }

        fn subscribe(
            &self,
        ) -> tokio::sync::broadcast::Receiver<taiji_infra_config::ConfigChangeEvent> {
            let (tx, _) = tokio::sync::broadcast::channel(16);
            tx.subscribe()
        }
    }

    // ── 新增测试：llm_config_from_config ──────────────────────────────

    #[tokio::test]
    async fn test_llm_config_from_config_reads_all_fields() {
        let mgr = MockConfigManager::new();
        let config = llm_config_from_config(&mgr).await;

        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.api_key.as_deref(), Some("sk-test-key"));
        assert_eq!(config.base_url.as_deref(), Some("https://api.example.com"));
        assert_eq!(config.temperature, 0.5);
        assert_eq!(config.max_tokens, 2048);
    }

    #[tokio::test]
    async fn test_llm_config_from_config_fallback_on_missing_fields() {
        let mgr = MockConfigManager {
            data: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        };
        let config = llm_config_from_config(&mgr).await;

        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.api_key, None);
        assert_eq!(config.base_url, None);
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.max_tokens, 4096);
    }

    // ── 新增测试：chat_stream_mpsc ────────────────────────────────────

    #[tokio::test]
    async fn test_mock_client_chat_stream_mpsc() {
        let client = MockClient::with_decision("mpsc_test", 0.9, "mpsc stream");

        let mut rx = client
            .chat_stream_mpsc(&[ChatMessage::user("测试")], &LlmConfig::default(), 16)
            .await
            .unwrap();

        let chunk = rx.recv().await.unwrap().unwrap();
        assert!(chunk.done);
        assert!(chunk.delta.contains("mpsc_test"));
    }

    #[tokio::test]
    async fn test_chat_stream_to_mpsc_converts_stream() {
        use futures::stream;

        let chunks = vec![
            Ok(ChatChunk {
                delta: "chunk1".into(),
                done: false,
                finish_reason: None,
            }),
            Ok(ChatChunk {
                delta: "chunk2".into(),
                done: true,
                finish_reason: Some("stop".into()),
            }),
        ];
        let stream: ChatStream = Box::pin(stream::iter(chunks));

        let mut rx = chat_stream_to_mpsc(stream, 16);

        let first = rx.recv().await.unwrap().unwrap();
        assert_eq!(first.delta, "chunk1");
        assert!(!first.done);

        let second = rx.recv().await.unwrap().unwrap();
        assert_eq!(second.delta, "chunk2");
        assert!(second.done);
    }

    #[tokio::test]
    async fn test_chat_stream_to_mpsc_closes_when_stream_ends() {
        use futures::stream;

        let stream: ChatStream = Box::pin(stream::empty());
        let mut rx = chat_stream_to_mpsc(stream, 16);

        assert!(rx.recv().await.is_none());
    }

    // ── 新增测试：run_llm_blocking ────────────────────────────────────

    #[tokio::test]
    async fn test_run_llm_blocking_ok() {
        let result = run_llm_blocking(|| Ok::<_, anyhow::Error>(42)).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_run_llm_blocking_err() {
        let result: anyhow::Result<i32> =
            run_llm_blocking(|| Err(anyhow::anyhow!("blocking error"))).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("blocking error"));
    }
}
