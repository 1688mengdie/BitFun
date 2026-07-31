//! 查询扩展 — 对接 taiji-llm LlmClient 将用户查询重写为 3 个变体。
//!
//! 参考: gbrain (MIT) core/search/expansion.ts — R-5-301 — Rust 翻译实现
//! L5-2: 查询扩展必须通过 taiji-llm LlmClient trait，不自行调用 LLM API

use taiji_llm::client::{ChatMessage, LlmClient};

use crate::error::GBrainError;

/// 查询扩展器（R-5-301）。
///
/// 将用户查询重写为多个语义变体，扩大检索覆盖。
pub struct QueryExpander {
    client: Box<dyn LlmClient>,
}

impl QueryExpander {
    /// 创建查询扩展器。
    pub fn new(client: Box<dyn LlmClient>) -> Self {
        Self { client }
    }

    /// 将用户查询扩展为 3 个变体。
    ///
    /// 使用 prompt 模板让 LLM 生成同义但不同表述的查询。
    pub async fn expand(&self, query: &str) -> Result<Vec<String>, GBrainError> {
        if query.trim().is_empty() {
            return Ok(vec![]);
        }

        let prompt = format!(
            "你是一个搜索查询扩展助手。请将以下用户查询重写为 3 个不同的表述变体，\
            每个变体用换行分隔（每行一个）。\
            保持原意不变，使用同义词或不同句式。\
            直接输出变体，不要添加序号、说明或其他文字。\n\n\
            用户查询：{}",
            query
        );

        let messages = vec![
            ChatMessage::system("你是一个查询扩展助手，输出简洁的查询变体。"),
            ChatMessage::user(&prompt),
        ];

        let config = taiji_llm::client::LlmConfig::default();
        let response = self
            .client
            .chat(&messages, &config)
            .await
            .map_err(|e| GBrainError::query(format!("query expansion failed: {}", e)))?;

        let text = response.content;
        let variants: Vec<String> = text
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .take(3)
            .collect();

        if variants.is_empty() {
            // Fallback: return original query
            Ok(vec![query.to_string()])
        } else {
            Ok(variants)
        }
    }

    /// 扩展查询并合并原始查询。
    pub async fn expand_with_original(&self, query: &str) -> Result<Vec<String>, GBrainError> {
        let mut variants = self.expand(query).await?;
        // 将原始查询放在第一个位置
        variants.insert(0, query.to_string());
        // 去重
        variants.sort();
        variants.dedup();
        Ok(variants)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taiji_llm::client::MockClient;

    fn make_expander() -> QueryExpander {
        let client = Box::new(MockClient::new("查询变体一\n查询变体二\n查询变体三"));
        QueryExpander::new(client)
    }

    #[tokio::test]
    async fn test_expand_empty() {
        let expander = make_expander();
        let variants = expander.expand("").await.unwrap();
        assert!(variants.is_empty());
    }

    #[tokio::test]
    async fn test_expand_basic() {
        let expander = make_expander();
        let variants = expander.expand("三层架构设计").await.unwrap();
        // MockClient returns the input directly, so we get one variant
        assert!(!variants.is_empty(), "should produce at least one variant");
    }

    #[tokio::test]
    async fn test_expand_with_original() {
        let expander = make_expander();
        let variants = expander.expand_with_original("量化交易").await.unwrap();
        assert!(!variants.is_empty(), "should include original query");
        assert!(variants.contains(&"量化交易".to_string()), "should contain original query");
    }

    #[tokio::test]
    async fn test_expand_with_original_deduplicates() {
        let expander = make_expander();
        let variants = expander.expand_with_original("测试").await.unwrap();
        let unique_len = {
            let mut v = variants.clone();
            v.sort();
            v.dedup();
            v.len()
        };
        assert_eq!(variants.len(), unique_len, "should deduplicate");
    }
}
