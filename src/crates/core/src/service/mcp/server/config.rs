//! MCP server configuration types.

use super::MCPServerType;
use crate::service::mcp::config::ConfigLocation;
use crate::util::errors::{BitFunError, BitFunResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

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
