//! Tool configuration sync module
//!
//! Automatically syncs the tool registry with the tool list in configuration:
//! newly added tools are added to the appropriate modes, and removed tools are
//! removed from configuration.

use crate::agentic::agents::get_agent_registry;
use crate::agentic::tools::registry::get_all_registered_tools;
use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Sync report.
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncReport {
    pub new_tools: Vec<String>,
    pub deleted_tools: Vec<String>,
    pub updated_modes: Vec<ModeSyncInfo>,
}

/// Mode sync information.
#[derive(Debug, Serialize, Deserialize)]
pub struct ModeSyncInfo {
    pub mode_id: String,
    pub added_tools: Vec<String>,
    pub removed_tools: Vec<String>,
}

/// Syncs tool configuration with the registry.
///
/// Logic:
/// 1. Get the current tool registry (excluding MCP tools)
/// 2. Read `known_tools` from configuration (historical record)
/// 3. Detect added and removed tools by diffing the sets
/// 4. For newly added tools, if they are in a mode's default list, add them to `available_tools`
/// 5. Remove deleted tools from all `available_tools`
/// 6. Update `known_tools` to the current set
/// 7. Persist configuration
pub async fn sync_tool_configs() -> BitFunResult<SyncReport> {
    let all_tools = get_all_registered_tools().await;
    let current_tools: HashSet<String> = all_tools
        .iter()
        .map(|t| t.name().to_string())
        .filter(|name| !name.starts_with("mcp_"))
        .collect();

    let config_service = GlobalConfigManager::get_service().await?;
    let mut config: crate::service::config::types::GlobalConfig =
        config_service.get_config(None).await?;
    let known_tools: HashSet<_> = config.ai.known_tools.into_iter().collect();

    let new_tools: Vec<String> = current_tools.difference(&known_tools).cloned().collect();

    let deleted_tools: Vec<String> = known_tools.difference(&current_tools).cloned().collect();

    let agent_registry = get_agent_registry();
    let mut updated_modes = Vec::new();

    for (mode_id, mode_config) in config.ai.mode_configs.iter_mut() {
        let mut added = Vec::new();

        if let Some(agent) = agent_registry.get_mode_agent(mode_id) {
            let default_tools = agent.default_tools();

            for new_tool in &new_tools {
                if default_tools.contains(new_tool) {
                    if !mode_config.available_tools.contains(new_tool) {
                        mode_config.available_tools.push(new_tool.clone());
                        added.push(new_tool.clone());
                    }
                }
            }
        }

        let (kept, removed): (Vec<String>, Vec<String>) = mode_config
            .available_tools
            .drain(..)
            .partition(|tool| !deleted_tools.contains(tool));

        mode_config.available_tools = kept;

        if !added.is_empty() || !removed.is_empty() {
            updated_modes.push(ModeSyncInfo {
                mode_id: mode_id.clone(),
                added_tools: added,
                removed_tools: removed,
            });
        }
    }

    config.ai.known_tools = current_tools.into_iter().collect();

    config_service.set_config("ai", &config.ai).await?;

    Ok(SyncReport {
        new_tools,
        deleted_tools,
        updated_modes,
    })
}
