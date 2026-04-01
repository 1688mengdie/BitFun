//! MCP server configuration types.

use super::MCPServerType;
use crate::service::mcp::config::ConfigLocation;
use crate::util::errors::{BitFunError, BitFunResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerOAuthConfig {
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_metadata_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerXaaConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// MCP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerConfig {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub server_type: MCPServerType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Additional HTTP headers for remote MCP servers (Cursor-style `headers`).
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default = "default_true")]
    pub auto_start: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub location: ConfigLocation,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub settings: HashMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<MCPServerOAuthConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xaa: Option<MCPServerXaaConfig>,
}

fn default_true() -> bool {
    true
}

impl MCPServerConfig {
    /// Validates the configuration.
    pub fn validate(&self) -> BitFunResult<()> {
        if self.id.is_empty() {
            return Err(BitFunError::Configuration(
                "MCP server id cannot be empty".to_string(),
            ));
        }

        if self.name.is_empty() {
            return Err(BitFunError::Configuration(
                "MCP server name cannot be empty".to_string(),
            ));
        }

        match self.server_type {
            MCPServerType::Local => {
                if self.command.is_none() {
                    return Err(BitFunError::Configuration(format!(
                        "Local MCP server '{}' must have a command",
                        self.id
                    )));
                }
            }
            MCPServerType::Remote => {
                if self.url.is_none() {
                    return Err(BitFunError::Configuration(format!(
                        "Remote MCP server '{}' must have a URL",
                        self.id
                    )));
                }

                if let Some(oauth) = &self.oauth {
                    if let Some(port) = oauth.callback_port {
                        if port == 0 {
                            return Err(BitFunError::Configuration(format!(
                                "Remote MCP server '{}' OAuth callbackPort must be greater than 0",
                                self.id
                            )));
                        }
                    }
                }
            }
            MCPServerType::Container => {
                if self.command.is_none() {
                    return Err(BitFunError::Configuration(format!(
                        "Container MCP server '{}' must have a command",
                        self.id
                    )));
                }
            }
        }

        Ok(())
    }
}
