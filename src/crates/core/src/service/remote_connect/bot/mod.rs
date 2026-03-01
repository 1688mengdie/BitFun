//! Bot integration for Remote Connect.
//!
//! Supports Feishu and Telegram bots as relay channels.

pub mod feishu;
pub mod telegram;

use serde::{Deserialize, Serialize};

/// Configuration for a bot-based connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "bot_type", rename_all = "snake_case")]
pub enum BotConfig {
    Feishu {
        app_id: String,
        app_secret: String,
    },
    Telegram {
        bot_token: String,
    },
}

/// Pairing state for bot-based connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotPairingInfo {
    pub pairing_code: String,
    pub bot_type: String,
    pub bot_link: String,
    pub expires_at: i64,
}
