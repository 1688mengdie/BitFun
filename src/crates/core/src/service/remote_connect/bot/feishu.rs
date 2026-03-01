//! Feishu (Lark) bot integration for Remote Connect.
//!
//! Users create their own Feishu bot on the Feishu Open Platform and provide
//! App ID + App Secret. Desktop listens for messages via the event subscription API.

use anyhow::{anyhow, Result};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuConfig {
    pub app_id: String,
    pub app_secret: String,
}

#[derive(Debug, Clone)]
struct FeishuToken {
    access_token: String,
    expires_at: i64,
}

pub struct FeishuBot {
    config: FeishuConfig,
    token: Arc<RwLock<Option<FeishuToken>>>,
    pending_pairings: Arc<RwLock<HashMap<String, PendingPairing>>>,
}

#[derive(Debug, Clone)]
struct PendingPairing {
    created_at: i64,
}

impl FeishuBot {
    pub fn new(config: FeishuConfig) -> Self {
        Self {
            config,
            token: Arc::new(RwLock::new(None)),
            pending_pairings: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or refresh the tenant access token.
    async fn get_access_token(&self) -> Result<String> {
        {
            let guard = self.token.read().await;
            if let Some(t) = guard.as_ref() {
                if t.expires_at > chrono::Utc::now().timestamp() + 60 {
                    return Ok(t.access_token.clone());
                }
            }
        }

        let client = reqwest::Client::new();
        let resp = client
            .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
            .json(&serde_json::json!({
                "app_id": self.config.app_id,
                "app_secret": self.config.app_secret,
            }))
            .send()
            .await
            .map_err(|e| anyhow!("feishu token request: {e}"))?;

        let body: serde_json::Value = resp.json().await?;
        let access_token = body["tenant_access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("missing tenant_access_token in response"))?
            .to_string();
        let expire = body["expire"].as_i64().unwrap_or(7200);

        *self.token.write().await = Some(FeishuToken {
            access_token: access_token.clone(),
            expires_at: chrono::Utc::now().timestamp() + expire,
        });

        info!("Feishu access token refreshed");
        Ok(access_token)
    }

    /// Send a text message to a Feishu chat.
    pub async fn send_message(&self, chat_id: &str, content: &str) -> Result<()> {
        let token = self.get_access_token().await?;
        let client = reqwest::Client::new();
        let resp = client
            .post("https://open.feishu.cn/open-apis/im/v1/messages")
            .query(&[("receive_id_type", "chat_id")])
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "receive_id": chat_id,
                "msg_type": "text",
                "content": serde_json::to_string(&serde_json::json!({"text": content}))?,
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("feishu send_message failed: {body}"));
        }

        debug!("Feishu message sent to {chat_id}");
        Ok(())
    }

    /// Register a pairing code and wait for the user to send it via bot.
    pub async fn register_pairing(&self, pairing_code: &str) -> Result<()> {
        self.pending_pairings.write().await.insert(
            pairing_code.to_string(),
            PendingPairing {
                created_at: chrono::Utc::now().timestamp(),
            },
        );
        Ok(())
    }

    /// Verify a pairing code received from a Feishu message.
    pub async fn verify_pairing_code(&self, code: &str) -> bool {
        let mut pairings = self.pending_pairings.write().await;
        if let Some(p) = pairings.remove(code) {
            let age = chrono::Utc::now().timestamp() - p.created_at;
            return age < 300;
        }
        false
    }

    /// Process an incoming Feishu event (webhook callback).
    pub async fn handle_event(&self, event: &serde_json::Value) -> Result<Option<String>> {
        let msg_type = event
            .pointer("/event/message/message_type")
            .and_then(|v| v.as_str());

        if msg_type != Some("text") {
            return Ok(None);
        }

        let content_str = event
            .pointer("/event/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("{}");

        let content: serde_json::Value = serde_json::from_str(content_str).unwrap_or_default();
        let text = content["text"].as_str().unwrap_or("").trim().to_string();

        if text.len() == 6 && text.chars().all(|c| c.is_ascii_digit()) {
            if self.verify_pairing_code(&text).await {
                let chat_id = event
                    .pointer("/event/message/chat_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                return Ok(Some(chat_id.to_string()));
            }
        }

        Ok(None)
    }
}
