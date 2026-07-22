//! taiji-llm — LLM 客户端抽象层。
//!
//! 提供统一的 [`LlmClient`] trait，以及 OpenAI / Claude / DeepSeek 三个 provider 实现。
//! 上层 Agent 通过 trait 调用，不依赖具体 provider。
//!
//! # 架构
//!
//! ```text
//! Agent (decision_agent / analysis agents)
//!   └── LlmClient trait (client.rs)
//!         ├── OpenAiClient  (provider/openai.rs)
//!         ├── ClaudeClient  (provider/claude.rs)
//!         ├── DeepSeekClient (provider/deepseek.rs)
//!         └── MockClient    (client.rs, 测试用)
//!   └── ChatMessage / ChatResponse / LlmConfig (client.rs)
//!   └── DecisionOutput (types.rs)
//! ```
//!
//! # 使用示例
//!
//! ```ignore
//! use taiji_llm::client::{ChatMessage, LlmClient, LlmConfig};
//! use taiji_llm::provider::openai::OpenAiClient;
//!
//! async fn example() {
//!     let client = OpenAiClient::new();
//!     let messages = vec![
//!         ChatMessage::system("你是一个交易分析助手"),
//!         ChatMessage::user("分析 rb9999 的趋势方向"),
//!     ];
//!     let config = LlmConfig {
//!         model: "gpt-4o".into(),
//!         ..Default::default()
//!     };
//!     let response = client.chat(&messages, &config).await.unwrap();
//!     println!("{}", response.content);
//! }
//! ```

pub mod client;
pub mod embedding;
pub mod provider;
pub mod types;

// 重新导出常用类型
pub use client::{ChatMessage, ChatResponse, LlmClient, LlmConfig, MockClient, Role, Usage};
pub use embedding::EmbeddingService;
pub use types::{ChatChunk, DecisionOutput};
