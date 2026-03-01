//! Telegram bot integration for Remote Connect.
//!
//! Users create their own bot via @BotFather, obtain a token, and enter it in BitFun settings.
//! Desktop polls for updates via the Telegram Bot API (long polling).

use anyhow::{anyhow, Result};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
}

pub struct TelegramBot {
    config: TelegramConfig,
    pending_pairings: Arc<RwLock<HashMap<String, PendingPairing>>>,
    last_update_id: Arc<RwLock<i64>>,
}

#[derive(Debug, Clone)]
struct PendingPairing {
    created_at: i64,
}

impl TelegramBot {
    pub fn new(config: TelegramConfig) -> Self {
        Self {
            config,
            pending_pairings: Arc::new(RwLock::new(HashMap::new())),
            last_update_id: Arc::new(RwLock::new(0)),
        }
    }

    fn api_url(&self, method: &str) -> String {
        format!(
            "https://api.telegram.org/bot{}/{}",
            self.config.bot_token, method
        )
    }

    /// Send a text message to a Telegram chat.
    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let resp = client
            .post(&self.api_url("sendMessage"))
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": text,
                "parse_mode": "Markdown",
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("telegram sendMessage failed: {body}"));
        }

        debug!("Telegram message sent to chat {chat_id}");
        Ok(())
    }

    /// Register a pairing code for verification.
    pub async fn register_pairing(&self, pairing_code: &str) -> Result<()> {
        self.pending_pairings.write().await.insert(
            pairing_code.to_string(),
            PendingPairing {
                created_at: chrono::Utc::now().timestamp(),
            },
        );
        Ok(())
    }

    /// Verify a pairing code. Returns true and removes it if valid and not expired.
    pub async fn verify_pairing_code(&self, code: &str) -> bool {
        let mut pairings = self.pending_pairings.write().await;
        if let Some(p) = pairings.remove(code) {
            let age = chrono::Utc::now().timestamp() - p.created_at;
            return age < 300;
        }
        false
    }

    /// Long-poll for new messages. Returns (chat_id, text) pairs.
    pub async fn poll_updates(&self) -> Result<Vec<(i64, String)>> {
        let offset = *self.last_update_id.read().await;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(35))
            .build()?;

        let resp = client
            .get(&self.api_url("getUpdates"))
            .query(&[
                ("offset", (offset + 1).to_string()),
                ("timeout", "30".to_string()),
            ])
            .send()
            .await?;

        let body: serde_json::Value = resp.json().await?;
        let results = body["result"].as_array().cloned().unwrap_or_default();

        let mut messages = Vec::new();
        for update in results {
            if let Some(update_id) = update["update_id"].as_i64() {
                let mut last = self.last_update_id.write().await;
                if update_id > *last {
                    *last = update_id;
                }
            }

            if let (Some(chat_id), Some(text)) = (
                update.pointer("/message/chat/id").and_then(|v| v.as_i64()),
                update
                    .pointer("/message/text")
                    .and_then(|v| v.as_str()),
            ) {
                messages.push((chat_id, text.trim().to_string()));
            }
        }

        Ok(messages)
    }

    /// Start a polling loop that checks for pairing codes.
    /// Returns the chat_id when a valid pairing code is received.
    pub async fn wait_for_pairing(&self) -> Result<i64> {
        info!("Telegram bot waiting for pairing code...");
        loop {
            match self.poll_updates().await {
                Ok(messages) => {
                    for (chat_id, text) in messages {
                        if text.len() == 6 && text.chars().all(|c| c.is_ascii_digit()) {
                            if self.verify_pairing_code(&text).await {
                                info!("Telegram pairing successful, chat_id={chat_id}");
                                self.send_message(chat_id, "Pairing successful! BitFun is now connected.")
                                    .await
                                    .ok();
                                return Ok(chat_id);
                            } else {
                                self.send_message(chat_id, "Invalid or expired pairing code. Please try again.")
                                    .await
                                    .ok();
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Telegram poll error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    }
}
