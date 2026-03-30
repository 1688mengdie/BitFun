//! Copied from `bitfun_core::service::config::ProxyConfig` for standalone installer AI client.

use serde::{Deserialize, Serialize};

/// Proxy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProxyConfig {
    /// Whether the proxy is enabled.
    pub enabled: bool,

    /// Proxy URL (format: http://host:port or socks5://host:port).
    pub url: String,

    /// Proxy username (optional).
    pub username: Option<String>,

    /// Proxy password (optional).
    pub password: Option<String>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            username: None,
            password: None,
        }
    }
}
