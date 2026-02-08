//! MCP (Model Context Protocol) service module
//!
//! Provides standardized MCP protocol support for connecting external context providers and
//! services.
//!
//! ## Module structure
//! - `protocol`: MCP protocol layer (JSON-RPC 2.0 communication)
//! - `server`: MCP server management (processes, connections, registry)
//! - `adapter`: Adapter layer (Resource/Prompt/Tool adapters)
//! - `config`: MCP configuration management

pub mod adapter;
pub mod config;
pub mod protocol;
pub mod server;

// Re-export main components.
pub use protocol::{
    MCPCapability, MCPMessage, MCPNotification, MCPProtocolVersion, MCPRequest, MCPResponse,
    MCPServerInfo,
};

pub use server::{
    MCPConnection, MCPConnectionPool, MCPServerConfig, MCPServerManager, MCPServerStatus,
    MCPServerType,
};

pub use adapter::{
    ContextEnhancer, MCPContextProvider, MCPToolAdapter, PromptAdapter, ResourceAdapter,
};

pub use config::{ConfigLocation, MCPConfigService};

/// MCP service interface.
pub struct MCPService {
    server_manager: std::sync::Arc<MCPServerManager>,
    config_service: std::sync::Arc<MCPConfigService>,
    context_provider: std::sync::Arc<MCPContextProvider>,
}

impl MCPService {
    /// Creates a new MCP service instance.
    pub fn new(
        config_service: std::sync::Arc<crate::service::config::ConfigService>,
    ) -> crate::util::errors::BitFunResult<Self> {
        let mcp_config_service = std::sync::Arc::new(MCPConfigService::new(config_service)?);
        let server_manager = std::sync::Arc::new(MCPServerManager::new(mcp_config_service.clone()));
        let context_provider = std::sync::Arc::new(MCPContextProvider::new(server_manager.clone()));

        Ok(Self {
            server_manager,
            config_service: mcp_config_service,
            context_provider,
        })
    }

    /// Returns the server manager.
    pub fn server_manager(&self) -> std::sync::Arc<MCPServerManager> {
        self.server_manager.clone()
    }

    /// Returns the context provider.
    pub fn context_provider(&self) -> std::sync::Arc<MCPContextProvider> {
        self.context_provider.clone()
    }

    /// Returns the configuration service.
    pub fn config_service(&self) -> std::sync::Arc<MCPConfigService> {
        self.config_service.clone()
    }
}
