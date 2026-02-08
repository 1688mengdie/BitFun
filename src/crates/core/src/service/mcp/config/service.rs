use log::{info, warn};
use std::sync::Arc;

use crate::service::config::ConfigService;
use crate::service::mcp::server::MCPServerConfig;
use crate::util::errors::{BitFunError, BitFunResult};

use super::ConfigLocation;

/// MCP configuration service.
pub struct MCPConfigService {
    pub(super) config_service: Arc<ConfigService>,
}

impl MCPConfigService {
    /// Creates a new MCP configuration service.
    pub fn new(config_service: Arc<ConfigService>) -> BitFunResult<Self> {
        Ok(Self { config_service })
    }

    /// Loads all MCP server configurations.
    pub async fn load_all_configs(&self) -> BitFunResult<Vec<MCPServerConfig>> {
        let mut configs = Vec::new();

        let builtin = self.load_builtin_configs().await?;
        configs.extend(builtin);

        match self.load_user_configs().await {
            Ok(user_configs) => {
                configs.extend(user_configs);
            }
            Err(e) => {
                warn!("Failed to load user-level MCP configs: {}", e);
            }
        }

        match self.load_project_configs().await {
            Ok(project_configs) => {
                configs.extend(project_configs);
            }
            Err(e) => {
                warn!("Failed to load project-level MCP configs: {}", e);
            }
        }

        info!("Loaded {} MCP server config(s)", configs.len());
        Ok(configs)
    }

    /// Loads built-in configurations.
    async fn load_builtin_configs(&self) -> BitFunResult<Vec<MCPServerConfig>> {
        Ok(Vec::new())
    }

    /// Loads user-level configuration (supports Cursor format `{ "mcpServers": { "id": {..} } }`
    /// and array format `[{..}]`).
    async fn load_user_configs(&self) -> BitFunResult<Vec<MCPServerConfig>> {
        match self
            .config_service
            .get_config::<serde_json::Value>(Some("mcp_servers"))
            .await
        {
            Ok(config_value) => {
                if config_value
                    .get("mcpServers")
                    .and_then(|v| v.as_object())
                    .is_some()
                {
                    return super::cursor_format::parse_cursor_format(&config_value);
                }

                if let Some(servers) = config_value.as_array() {
                    let configs: Vec<MCPServerConfig> = servers
                        .iter()
                        .filter_map(|v| {
                            match serde_json::from_value::<MCPServerConfig>(v.clone()) {
                                Ok(config) => Some(config),
                                Err(e) => {
                                    warn!("Failed to parse MCP config item: {}", e);
                                    None
                                }
                            }
                        })
                        .collect();
                    return Ok(configs);
                }

                warn!("Invalid MCP config format, returning empty list");
                Ok(Vec::new())
            }
            Err(_) => Ok(Vec::new()),
        }
    }

    /// Loads project-level configuration.
    async fn load_project_configs(&self) -> BitFunResult<Vec<MCPServerConfig>> {
        match self
            .config_service
            .get_config::<serde_json::Value>(Some("project.mcp_servers"))
            .await
        {
            Ok(config_value) => {
                if let Some(servers) = config_value.as_array() {
                    let configs: Vec<MCPServerConfig> = servers
                        .iter()
                        .filter_map(|v| serde_json::from_value(v.clone()).ok())
                        .collect();
                    Ok(configs)
                } else {
                    Ok(Vec::new())
                }
            }
            Err(_) => Ok(Vec::new()),
        }
    }

    /// Gets a single server configuration.
    pub async fn get_server_config(
        &self,
        server_id: &str,
    ) -> BitFunResult<Option<MCPServerConfig>> {
        let all_configs = self.load_all_configs().await?;
        Ok(all_configs.into_iter().find(|c| c.id == server_id))
    }

    /// Saves a server configuration.
    pub async fn save_server_config(&self, config: &MCPServerConfig) -> BitFunResult<()> {
        match config.location {
            ConfigLocation::BuiltIn => Err(BitFunError::Configuration(
                "Cannot modify built-in MCP server configuration".to_string(),
            )),
            ConfigLocation::User => self.save_user_config(config).await,
            ConfigLocation::Project => self.save_project_config(config).await,
        }
    }

    /// Saves user-level configuration.
    async fn save_user_config(&self, config: &MCPServerConfig) -> BitFunResult<()> {
        let current_value = self
            .config_service
            .get_config::<serde_json::Value>(Some("mcp_servers"))
            .await
            .unwrap_or_else(|_| serde_json::json!({ "mcpServers": {} }));

        let mut mcp_servers =
            if let Some(obj) = current_value.get("mcpServers").and_then(|v| v.as_object()) {
                obj.clone()
            } else {
                serde_json::Map::new()
            };

        let cursor_format = super::cursor_format::config_to_cursor_format(config);

        mcp_servers.insert(config.id.clone(), cursor_format);

        let new_value = serde_json::json!({
            "mcpServers": mcp_servers
        });

        self.config_service
            .set_config("mcp_servers", new_value)
            .await?;
        info!(
            "Saved user-level MCP server config (Cursor format): {}",
            config.id
        );
        Ok(())
    }

    /// Saves project-level configuration.
    async fn save_project_config(&self, config: &MCPServerConfig) -> BitFunResult<()> {
        let mut configs = self.load_project_configs().await.unwrap_or_default();

        if let Some(existing) = configs.iter_mut().find(|c| c.id == config.id) {
            *existing = config.clone();
        } else {
            configs.push(config.clone());
        }

        let value = serde_json::to_value(&configs).map_err(|e| {
            BitFunError::serialization(format!("Failed to serialize MCP config: {}", e))
        })?;

        self.config_service
            .set_config("project.mcp_servers", value)
            .await?;
        Ok(())
    }

    /// Deletes a server configuration.
    pub async fn delete_server_config(&self, server_id: &str) -> BitFunResult<()> {
        let current_value = self
            .config_service
            .get_config::<serde_json::Value>(Some("mcp_servers"))
            .await
            .unwrap_or_else(|_| serde_json::json!({ "mcpServers": {} }));

        let mut mcp_servers =
            if let Some(obj) = current_value.get("mcpServers").and_then(|v| v.as_object()) {
                obj.clone()
            } else {
                return Err(BitFunError::NotFound(format!(
                    "MCP server config not found: {}",
                    server_id
                )));
            };

        if mcp_servers.remove(server_id).is_none() {
            return Err(BitFunError::NotFound(format!(
                "MCP server config not found: {}",
                server_id
            )));
        }

        let new_value = serde_json::json!({
            "mcpServers": mcp_servers
        });

        self.config_service
            .set_config("mcp_servers", new_value)
            .await?;
        info!("Deleted MCP server config: {}", server_id);
        Ok(())
    }
}
