//! MCP server manager
//!
//! Manages the lifecycle of all MCP servers.

use super::connection::{MCPConnection, MCPConnectionEvent, MCPConnectionPool};
use super::{MCPServerConfig, MCPServerRegistry, MCPServerStatus};
use crate::service::mcp::adapter::tool::MCPToolAdapter;
use crate::service::mcp::config::MCPConfigService;
use crate::service::mcp::protocol::{MCPError, MCPPrompt, MCPResource};
use crate::infrastructure::events::event_system::{get_global_event_system, BackendEvent};
use crate::service::runtime::{RuntimeManager, RuntimeSource};
use crate::service::workspace::get_global_workspace_service;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, error, info, warn};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use tokio::sync::oneshot;

/// Reconnect policy for unhealthy MCP servers.
#[derive(Debug, Clone, Copy)]
struct ReconnectPolicy {
    poll_interval: Duration,
    base_delay: Duration,
    max_delay: Duration,
    max_attempts: u32,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            base_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(60),
            max_attempts: 6,
        }
    }
}

#[derive(Debug, Clone)]
struct ReconnectAttemptState {
    attempts: u32,
    next_retry_at: Instant,
    exhausted_logged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListChangedKind {
    Tools,
    Prompts,
    Resources,
}

#[derive(Debug)]
enum MCPInteractionDecision {
    Accept {
        result: Value,
    },
    Reject {
        error: MCPError,
    },
}

#[derive(Debug)]
struct PendingMCPInteraction {
    sender: oneshot::Sender<MCPInteractionDecision>,
}

impl ReconnectAttemptState {
    fn new(now: Instant) -> Self {
        Self {
            attempts: 0,
            next_retry_at: now,
            exhausted_logged: false,
        }
    }
}

/// MCP server manager.
#[derive(Clone)]
pub struct MCPServerManager {
    registry: Arc<MCPServerRegistry>,
    connection_pool: Arc<MCPConnectionPool>,
    config_service: Arc<MCPConfigService>,
    reconnect_policy: ReconnectPolicy,
    reconnect_states: Arc<tokio::sync::RwLock<HashMap<String, ReconnectAttemptState>>>,
    reconnect_monitor_started: Arc<AtomicBool>,
    connection_event_tasks: Arc<tokio::sync::RwLock<HashMap<String, JoinHandle<()>>>>,
    resource_catalog_cache: Arc<tokio::sync::RwLock<HashMap<String, Vec<MCPResource>>>>,
    prompt_catalog_cache: Arc<tokio::sync::RwLock<HashMap<String, Vec<MCPPrompt>>>>,
    pending_interactions:
        Arc<tokio::sync::RwLock<HashMap<String, PendingMCPInteraction>>>,
}

impl MCPServerManager {
    /// Creates a new server manager.
    pub fn new(config_service: Arc<MCPConfigService>) -> Self {
        Self {
            registry: Arc::new(MCPServerRegistry::new()),
            connection_pool: Arc::new(MCPConnectionPool::new()),
            config_service,
            reconnect_policy: ReconnectPolicy::default(),
            reconnect_states: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            reconnect_monitor_started: Arc::new(AtomicBool::new(false)),
            connection_event_tasks: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            resource_catalog_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            prompt_catalog_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            pending_interactions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    fn start_reconnect_monitor_if_needed(&self) {
        if self.reconnect_monitor_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let manager = self.clone();
        tokio::spawn(async move {
            manager.run_reconnect_monitor().await;
        });
        info!("Started MCP reconnect monitor");
    }

    async fn run_reconnect_monitor(self) {
        let mut interval = tokio::time::interval(self.reconnect_policy.poll_interval);
        loop {
            interval.tick().await;
            if let Err(e) = self.reconnect_once().await {
                warn!("MCP reconnect monitor tick failed: {}", e);
            }
        }
    }

    async fn reconnect_once(&self) -> BitFunResult<()> {
        let configs = self.config_service.load_all_configs().await?;

        for config in configs {
            if !(config.enabled && config.auto_start) {
                self.clear_reconnect_state(&config.id).await;
                continue;
            }

            let status = self
                .get_server_status(&config.id)
                .await
                .unwrap_or(MCPServerStatus::Uninitialized);

            if matches!(
                status,
                MCPServerStatus::Connected | MCPServerStatus::Healthy | MCPServerStatus::Starting
            ) {
                self.clear_reconnect_state(&config.id).await;
                continue;
            }

            if matches!(status, MCPServerStatus::NeedsAuth) {
                self.clear_reconnect_state(&config.id).await;
                continue;
            }

            if !matches!(
                status,
                MCPServerStatus::Reconnecting | MCPServerStatus::Failed
            ) {
                continue;
            }

            self.try_reconnect_server(&config.id, &config.name, status)
                .await;
        }

        Ok(())
    }

    async fn try_reconnect_server(
        &self,
        server_id: &str,
        server_name: &str,
        status: MCPServerStatus,
    ) {
        let now = Instant::now();

        let (attempt_number, next_delay) = {
            let mut reconnect_states = self.reconnect_states.write().await;
            let state = reconnect_states
                .entry(server_id.to_string())
                .or_insert_with(|| ReconnectAttemptState::new(now));

            if state.attempts >= self.reconnect_policy.max_attempts {
                if !state.exhausted_logged {
                    warn!(
                        "MCP reconnect attempts exhausted: server_name={} server_id={} max_attempts={} status={:?}",
                        server_name, server_id, self.reconnect_policy.max_attempts, status
                    );
                    state.exhausted_logged = true;
                }
                return;
            }

            if now < state.next_retry_at {
                return;
            }

            state.attempts += 1;
            let delay = Self::compute_backoff_delay(
                self.reconnect_policy.base_delay,
                self.reconnect_policy.max_delay,
                state.attempts,
            );
            state.next_retry_at = now + delay;
            (state.attempts, delay)
        };

        info!(
            "Attempting MCP reconnect: server_name={} server_id={} attempt={}/{} status={:?}",
            server_name, server_id, attempt_number, self.reconnect_policy.max_attempts, status
        );

        let _ = self.stop_server(server_id).await;
        match self.start_server(server_id).await {
            Ok(_) => {
                self.clear_reconnect_state(server_id).await;
                info!(
                    "MCP reconnect succeeded: server_name={} server_id={} attempt={}",
                    server_name, server_id, attempt_number
                );
            }
            Err(e) => {
                warn!(
                    "MCP reconnect failed: server_name={} server_id={} attempt={}/{} next_retry_in={}s error={}",
                    server_name,
                    server_id,
                    attempt_number,
                    self.reconnect_policy.max_attempts,
                    next_delay.as_secs(),
                    e
                );
            }
        }
    }

    fn compute_backoff_delay(base: Duration, max: Duration, attempt: u32) -> Duration {
        let shift = attempt.saturating_sub(1).min(20);
        let factor = 1u64 << shift;
        let base_ms = base.as_millis() as u64;
        let max_ms = max.as_millis() as u64;
        let delay_ms = base_ms.saturating_mul(factor).min(max_ms);
        Duration::from_millis(delay_ms)
    }

    async fn clear_reconnect_state(&self, server_id: &str) {
        let mut reconnect_states = self.reconnect_states.write().await;
        reconnect_states.remove(server_id);
    }

    fn detect_list_changed_kind(method: &str) -> Option<ListChangedKind> {
        match method {
            "notifications/tools/list_changed"
            | "notifications/tools/listChanged"
            | "tools/list_changed" => Some(ListChangedKind::Tools),
            "notifications/prompts/list_changed"
            | "notifications/prompts/listChanged"
            | "prompts/list_changed" => Some(ListChangedKind::Prompts),
            "notifications/resources/list_changed"
            | "notifications/resources/listChanged"
            | "resources/list_changed" => Some(ListChangedKind::Resources),
            _ => None,
        }
    }

    async fn refresh_resources_catalog(
        &self,
        server_id: &str,
        connection: Arc<MCPConnection>,
    ) -> BitFunResult<usize> {
        let mut resources = Vec::new();
        let mut cursor = None::<String>;
        let mut visited = HashSet::new();

        loop {
            let result = connection.list_resources(cursor.clone()).await?;
            resources.extend(result.resources);

            match result.next_cursor {
                Some(next) => {
                    if !visited.insert(next.clone()) {
                        break;
                    }
                    cursor = Some(next);
                }
                None => break,
            }
        }

        let count = resources.len();
        let mut cache = self.resource_catalog_cache.write().await;
        cache.insert(server_id.to_string(), resources);
        Ok(count)
    }

    async fn refresh_prompts_catalog(
        &self,
        server_id: &str,
        connection: Arc<MCPConnection>,
    ) -> BitFunResult<usize> {
        let mut prompts = Vec::new();
        let mut cursor = None::<String>;
        let mut visited = HashSet::new();

        loop {
            let result = connection.list_prompts(cursor.clone()).await?;
            prompts.extend(result.prompts);

            match result.next_cursor {
                Some(next) => {
                    if !visited.insert(next.clone()) {
                        break;
                    }
                    cursor = Some(next);
                }
                None => break,
            }
        }

        let count = prompts.len();
        let mut cache = self.prompt_catalog_cache.write().await;
        cache.insert(server_id.to_string(), prompts);
        Ok(count)
    }

    async fn warm_catalog_caches(&self, server_id: &str, connection: Arc<MCPConnection>) {
        if let Err(e) = self
            .refresh_resources_catalog(server_id, connection.clone())
            .await
        {
            debug!(
                "Skipping MCP resources catalog warmup: server_id={} error={}",
                server_id, e
            );
        }

        if let Err(e) = self.refresh_prompts_catalog(server_id, connection).await {
            debug!(
                "Skipping MCP prompts catalog warmup: server_id={} error={}",
                server_id, e
            );
        }
    }

    fn path_to_file_uri(path: &Path) -> Option<String> {
        reqwest::Url::from_directory_path(path)
            .ok()
            .map(|u| u.to_string())
    }

    fn build_roots_list_result() -> Value {
        let mut candidate_roots = Vec::new();

        if let Some(workspace_service) = get_global_workspace_service() {
            if let Some(workspace_root) = workspace_service.try_get_current_workspace_path() {
                candidate_roots.push(workspace_root);
            }
        }

        if candidate_roots.is_empty() {
            if let Ok(current_dir) = std::env::current_dir() {
                candidate_roots.push(current_dir);
            }
        }

        let mut seen_uris = HashSet::new();
        let mut roots = Vec::new();
        for root in candidate_roots {
            let Some(uri) = Self::path_to_file_uri(&root) else {
                continue;
            };
            if !seen_uris.insert(uri.clone()) {
                continue;
            }
            let name = root
                .file_name()
                .and_then(|v| v.to_str())
                .filter(|v| !v.is_empty())
                .unwrap_or("BitFun Workspace")
                .to_string();
            roots.push(json!({
                "uri": uri,
                "name": name,
            }));
        }

        json!({ "roots": roots })
    }

    async fn handle_server_request(
        &self,
        server_id: &str,
        server_name: &str,
        connection: Arc<MCPConnection>,
        request_id: Value,
        method: String,
        params: Option<Value>,
    ) {
        match method.as_str() {
            "ping" => {
                if let Err(e) = connection.send_response(request_id, json!({})).await {
                    warn!(
                        "Failed to respond to MCP ping request: server_name={} server_id={} error={}",
                        server_name, server_id, e
                    );
                }
            }
            "roots/list" => {
                let result = Self::build_roots_list_result();
                if let Err(e) = connection.send_response(request_id, result).await {
                    warn!(
                        "Failed to respond to MCP roots/list request: server_name={} server_id={} error={}",
                        server_name, server_id, e
                    );
                } else {
                    info!(
                        "Handled MCP roots/list request: server_name={} server_id={}",
                        server_name, server_id
                    );
                }
            }
            "elicitation/create" | "sampling/createMessage" => {
                self
                    .handle_interactive_server_request(
                        server_id,
                        server_name,
                        connection,
                        request_id,
                        method,
                        params,
                    )
                    .await;
            }
            _ => {
                let error = MCPError::method_not_found(method.clone());
                if let Err(e) = connection.send_error(request_id, error).await {
                    warn!(
                        "Failed to respond with method_not_found for MCP request: server_name={} server_id={} method={} error={}",
                        server_name, server_id, method, e
                    );
                } else {
                    warn!(
                        "Rejected unsupported MCP server request: server_name={} server_id={} method={}",
                        server_name, server_id, method
                    );
                }
            }
        }
    }

    async fn handle_interactive_server_request(
        &self,
        server_id: &str,
        server_name: &str,
        connection: Arc<MCPConnection>,
        request_id: Value,
        method: String,
        params: Option<Value>,
    ) {
        let interaction_id = format!("mcp_interaction_{}", uuid::Uuid::new_v4());
        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending_interactions.write().await;
            pending.insert(
                interaction_id.clone(),
                PendingMCPInteraction { sender: tx },
            );
        }

        let event_payload = json!({
            "interactionId": interaction_id,
            "serverId": server_id,
            "serverName": server_name,
            "method": method.clone(),
            "params": params,
        });

        let event_system = get_global_event_system();
        if let Err(e) = event_system
            .emit(BackendEvent::Custom {
                event_name: "backend-event-mcpinteractionrequest".to_string(),
                payload: event_payload,
            })
            .await
        {
            warn!(
                "Failed to emit MCP interaction request event: server_name={} server_id={} method={} error={}",
                server_name, server_id, method, e
            );
        }

        let wait_timeout = Duration::from_secs(600);
        let decision = tokio::time::timeout(wait_timeout, rx).await;
        {
            let mut pending = self.pending_interactions.write().await;
            pending.remove(&interaction_id);
        }

        match decision {
            Ok(Ok(MCPInteractionDecision::Accept { result })) => {
                if let Err(e) = connection.send_response(request_id, result).await {
                    warn!(
                        "Failed to send interactive MCP response: server_name={} server_id={} method={} error={}",
                        server_name, server_id, method, e
                    );
                } else {
                    info!(
                        "Handled interactive MCP request: server_name={} server_id={} method={}",
                        server_name, server_id, method
                    );
                }
            }
            Ok(Ok(MCPInteractionDecision::Reject { error })) => {
                if let Err(e) = connection.send_error(request_id, error).await {
                    warn!(
                        "Failed to send interactive MCP rejection: server_name={} server_id={} method={} error={}",
                        server_name, server_id, method, e
                    );
                } else {
                    info!(
                        "Rejected interactive MCP request: server_name={} server_id={} method={}",
                        server_name, server_id, method
                    );
                }
            }
            Ok(Err(_)) => {
                let error = MCPError::internal_error(format!(
                    "MCP interaction channel closed before response: {}",
                    method
                ));
                if let Err(e) = connection.send_error(request_id, error).await {
                    warn!(
                        "Failed to send interaction channel-closed error: server_name={} server_id={} method={} error={}",
                        server_name, server_id, method, e
                    );
                }
            }
            Err(_) => {
                let error = MCPError::internal_error(format!(
                    "Timed out waiting for user interaction response for method: {}",
                    method
                ));
                if let Err(e) = connection.send_error(request_id, error).await {
                    warn!(
                        "Failed to send interaction timeout error: server_name={} server_id={} method={} error={}",
                        server_name, server_id, method, e
                    );
                } else {
                    warn!(
                        "Timed out waiting for interactive MCP request: server_name={} server_id={} method={} timeout={}s",
                        server_name, server_id, method, wait_timeout.as_secs()
                    );
                }
            }
        }
    }

    pub async fn submit_interaction_response(
        &self,
        interaction_id: &str,
        approve: bool,
        result: Option<Value>,
        error_message: Option<String>,
        error_code: Option<i32>,
        error_data: Option<Value>,
    ) -> BitFunResult<()> {
        let pending = {
            let mut interactions = self.pending_interactions.write().await;
            interactions.remove(interaction_id)
        };

        let Some(pending) = pending else {
            return Err(BitFunError::NotFound(format!(
                "MCP interaction not found: {}",
                interaction_id
            )));
        };

        let decision = if approve {
            MCPInteractionDecision::Accept {
                result: result.unwrap_or_else(|| json!({})),
            }
        } else {
            MCPInteractionDecision::Reject {
                error: MCPError {
                    code: error_code.unwrap_or(MCPError::INVALID_REQUEST),
                    message: error_message
                        .unwrap_or_else(|| "User rejected MCP interaction request".to_string()),
                    data: error_data,
                },
            }
        };

        pending.sender.send(decision).map_err(|_| {
            BitFunError::MCPError(format!(
                "Failed to deliver MCP interaction response (receiver dropped): {}",
                interaction_id
            ))
        })?;

        Ok(())
    }

    async fn start_connection_event_listener(
        &self,
        server_id: &str,
        server_name: &str,
        connection: Arc<MCPConnection>,
    ) {
        self.stop_connection_event_listener(server_id).await;

        let manager = self.clone();
        let server_id_owned = server_id.to_string();
        let server_name_owned = server_name.to_string();
        let mut rx = connection.subscribe_events();
        let connection_for_refresh = connection.clone();

        let handle = tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(MCPConnectionEvent::Notification { method, .. }) => {
                        match Self::detect_list_changed_kind(&method) {
                            Some(ListChangedKind::Tools) => {
                                info!(
                                    "Received MCP tools list-changed notification: server_name={} server_id={}",
                                    server_name_owned, server_id_owned
                                );
                                if let Err(e) = manager
                                    .refresh_mcp_tools(
                                        &server_id_owned,
                                        &server_name_owned,
                                        connection_for_refresh.clone(),
                                    )
                                    .await
                                {
                                    warn!(
                                        "Failed to refresh MCP tools after list-changed notification: server_name={} server_id={} error={}",
                                        server_name_owned, server_id_owned, e
                                    );
                                }
                            }
                            Some(ListChangedKind::Prompts) => {
                                info!(
                                    "Received MCP prompts list-changed notification: server_name={} server_id={}",
                                    server_name_owned, server_id_owned
                                );
                                if let Err(e) = manager
                                    .refresh_prompts_catalog(
                                        &server_id_owned,
                                        connection_for_refresh.clone(),
                                    )
                                    .await
                                {
                                    warn!(
                                        "Failed to refresh MCP prompts catalog after list-changed notification: server_name={} server_id={} error={}",
                                        server_name_owned, server_id_owned, e
                                    );
                                }
                            }
                            Some(ListChangedKind::Resources) => {
                                info!(
                                    "Received MCP resources list-changed notification: server_name={} server_id={}",
                                    server_name_owned, server_id_owned
                                );
                                if let Err(e) = manager
                                    .refresh_resources_catalog(
                                        &server_id_owned,
                                        connection_for_refresh.clone(),
                                    )
                                    .await
                                {
                                    warn!(
                                        "Failed to refresh MCP resources catalog after list-changed notification: server_name={} server_id={} error={}",
                                        server_name_owned, server_id_owned, e
                                    );
                                }
                            }
                            None => {
                                debug!(
                                    "Ignoring MCP notification from server: server_name={} server_id={} method={}",
                                    server_name_owned, server_id_owned, method
                                );
                            }
                        }
                    }
                    Ok(MCPConnectionEvent::Request {
                        request_id,
                        method,
                        params,
                    }) => {
                        manager
                            .handle_server_request(
                                &server_id_owned,
                                &server_name_owned,
                                connection_for_refresh.clone(),
                                request_id,
                                method,
                                params,
                            )
                            .await;
                    }
                    Ok(MCPConnectionEvent::Closed) => {
                        warn!(
                            "MCP connection event stream closed: server_name={} server_id={}",
                            server_name_owned, server_id_owned
                        );
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                        warn!(
                            "Dropped MCP connection events due to lag: server_name={} server_id={} dropped={}",
                            server_name_owned, server_id_owned, count
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });

        let mut tasks = self.connection_event_tasks.write().await;
        tasks.insert(server_id.to_string(), handle);
    }

    async fn stop_connection_event_listener(&self, server_id: &str) {
        let mut tasks = self.connection_event_tasks.write().await;
        if let Some(handle) = tasks.remove(server_id) {
            handle.abort();
        }
    }

    async fn refresh_mcp_tools(
        &self,
        server_id: &str,
        server_name: &str,
        connection: Arc<MCPConnection>,
    ) -> BitFunResult<usize> {
        Self::unregister_mcp_tools(server_id).await;
        Self::register_mcp_tools(server_id, server_name, connection).await
    }

    /// Initializes all servers.
    pub async fn initialize_all(&self) -> BitFunResult<()> {
        self.start_reconnect_monitor_if_needed();
        info!("Initializing all MCP servers");

        let existing_server_ids = self.registry.get_all_server_ids().await;
        if !existing_server_ids.is_empty() {
            info!(
                "Refreshing MCP servers: shutting down existing servers before applying config: count={}",
                existing_server_ids.len()
            );
            self.shutdown().await?;
        }

        let configs = self.config_service.load_all_configs().await?;
        info!("Loaded {} MCP server configs", configs.len());

        if configs.is_empty() {
            warn!("No MCP server configurations found");
            return Ok(());
        }

        let mut registered_count = 0;
        for config in &configs {
            if config.enabled {
                match self.registry.register(config).await {
                    Ok(_) => {
                        registered_count += 1;
                        debug!(
                            "Registered MCP server: name={} id={}",
                            config.name, config.id
                        );
                    }
                    Err(e) => {
                        error!(
                            "Failed to register MCP server: name={} id={} error={}",
                            config.name, config.id, e
                        );
                        return Err(e);
                    }
                }
            }
        }
        info!("Registered {} MCP servers", registered_count);

        let mut started_count = 0;
        let mut failed_count = 0;
        for config in configs {
            if config.enabled && config.auto_start {
                info!(
                    "Auto-starting MCP server: name={} id={}",
                    config.name, config.id
                );
                match self.start_server(&config.id).await {
                    Ok(_) => {
                        started_count += 1;
                        info!("MCP server started successfully: name={}", config.name);
                    }
                    Err(e) => {
                        failed_count += 1;
                        error!(
                            "Failed to auto-start MCP server: name={} id={} error={}",
                            config.name, config.id, e
                        );
                    }
                }
            }
        }

        info!(
            "MCP server initialization completed: started={} failed={}",
            started_count, failed_count
        );
        Ok(())
    }

    /// Initializes servers without shutting down existing ones.
    ///
    /// This is safe to call multiple times (e.g., from multiple frontend windows).
    pub async fn initialize_non_destructive(&self) -> BitFunResult<()> {
        self.start_reconnect_monitor_if_needed();
        info!("Initializing MCP servers (non-destructive)");

        let configs = self.config_service.load_all_configs().await?;
        if configs.is_empty() {
            return Ok(());
        }

        for config in &configs {
            if !config.enabled {
                continue;
            }
            if !self.registry.contains(&config.id).await {
                if let Err(e) = self.registry.register(config).await {
                    warn!(
                        "Failed to register MCP server during non-destructive init: name={} id={} error={}",
                        config.name, config.id, e
                    );
                }
            }
        }

        for config in configs {
            if !(config.enabled && config.auto_start) {
                continue;
            }

            // Start only when not already running.
            if let Ok(status) = self.get_server_status(&config.id).await {
                if matches!(
                    status,
                    MCPServerStatus::Connected | MCPServerStatus::Healthy
                ) {
                    continue;
                }
            }

            let _ = self.start_server(&config.id).await;
        }

        Ok(())
    }

    /// Ensures a server is registered in the registry if it exists in config.
    ///
    /// This is useful after config changes (e.g. importing MCP servers) where the registry
    /// hasn't been re-initialized yet.
    pub async fn ensure_registered(&self, server_id: &str) -> BitFunResult<()> {
        if self.registry.contains(server_id).await {
            return Ok(());
        }

        let Some(config) = self.config_service.get_server_config(server_id).await? else {
            return Err(BitFunError::NotFound(format!(
                "MCP server config not found: {}",
                server_id
            )));
        };

        if !config.enabled {
            return Ok(());
        }

        self.registry.register(&config).await?;
        Ok(())
    }

    /// Starts a server.
    pub async fn start_server(&self, server_id: &str) -> BitFunResult<()> {
        self.start_reconnect_monitor_if_needed();
        info!("Starting MCP server: id={}", server_id);

        let config = self
            .config_service
            .get_server_config(server_id)
            .await?
            .ok_or_else(|| {
                error!("MCP server config not found: id={}", server_id);
                BitFunError::NotFound(format!("MCP server config not found: {}", server_id))
            })?;

        if !config.enabled {
            warn!("MCP server is disabled: id={}", server_id);
            return Err(BitFunError::Configuration(format!(
                "MCP server is disabled: {}",
                server_id
            )));
        }

        if !self.registry.contains(server_id).await {
            self.registry.register(&config).await?;
        }

        let process = self.registry.get_process(server_id).await.ok_or_else(|| {
            error!("MCP server not registered: id={}", server_id);
            BitFunError::NotFound(format!("MCP server not registered: {}", server_id))
        })?;

        let mut proc = process.write().await;

        let status = proc.status().await;
        if matches!(
            status,
            MCPServerStatus::Connected | MCPServerStatus::Healthy
        ) {
            warn!("MCP server already running: id={}", server_id);
            return Ok(());
        }

        match config.server_type {
            super::MCPServerType::Local => {
                let command = config.command.as_ref().ok_or_else(|| {
                    error!("Missing command for local MCP server: id={}", server_id);
                    BitFunError::Configuration("Missing command for local MCP server".to_string())
                })?;

                let runtime_manager = RuntimeManager::new()?;
                let resolved = runtime_manager.resolve_command(command).ok_or_else(|| {
                    BitFunError::ProcessError(format!(
                        "MCP server command '{}' not found in system PATH or BitFun managed runtimes at {}",
                        command,
                        runtime_manager.runtime_root_display()
                    ))
                })?;

                let source_label = match resolved.source {
                    RuntimeSource::System => "system",
                    RuntimeSource::Managed => "managed",
                };

                info!(
                    "Starting local MCP server: command={} source={} id={}",
                    resolved.command, source_label, server_id
                );

                proc.start(&resolved.command, &config.args, &config.env)
                    .await
                    .map_err(|e| {
                        error!(
                            "Failed to start local MCP server process: id={} command={} source={} error={}",
                            server_id, resolved.command, source_label, e
                        );
                        e
                    })?;
            }
            super::MCPServerType::Remote => {
                let url = config.url.as_ref().ok_or_else(|| {
                    error!("Missing URL for remote MCP server: id={}", server_id);
                    BitFunError::Configuration("Missing URL for remote MCP server".to_string())
                })?;

                info!(
                    "Connecting to remote MCP server: url={} id={}",
                    url, server_id
                );

                proc.start_remote(url, &config.env, &config.headers)
                    .await
                    .map_err(|e| {
                        error!(
                            "Failed to connect to remote MCP server: url={} id={} error={}",
                            url, server_id, e
                        );
                        e
                    })?;
            }
            super::MCPServerType::Container => {
                error!("Container MCP servers not supported: id={}", server_id);
                return Err(BitFunError::NotImplemented(
                    "Container MCP servers not yet supported".to_string(),
                ));
            }
        }

        if let Some(connection) = proc.connection() {
            self.connection_pool
                .add_connection(server_id.to_string(), connection.clone())
                .await;

            match Self::register_mcp_tools(server_id, &config.name, connection.clone()).await {
                Ok(count) => {
                    info!(
                        "Registered {} MCP tools: server_name={} server_id={}",
                        count, config.name, server_id
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to register MCP tools: server_name={} server_id={} error={}",
                        config.name, server_id, e
                    );
                }
            }

            self.start_connection_event_listener(server_id, &config.name, connection.clone())
                .await;
            self.warm_catalog_caches(server_id, connection).await;
        } else {
            warn!(
                "Connection not available, server may not have started correctly: id={}",
                server_id
            );
        }

        info!("MCP server started successfully: id={}", server_id);
        self.clear_reconnect_state(server_id).await;
        Ok(())
    }

    /// Stops a server.
    pub async fn stop_server(&self, server_id: &str) -> BitFunResult<()> {
        info!("Stopping MCP server: id={}", server_id);

        self.stop_connection_event_listener(server_id).await;

        let process =
            self.registry.get_process(server_id).await.ok_or_else(|| {
                BitFunError::NotFound(format!("MCP server not found: {}", server_id))
            })?;

        let mut proc = process.write().await;
        let stop_result = proc.stop().await;

        self.connection_pool.remove_connection(server_id).await;
        self.resource_catalog_cache.write().await.remove(server_id);
        self.prompt_catalog_cache.write().await.remove(server_id);

        Self::unregister_mcp_tools(server_id).await;

        stop_result
    }

    /// Restarts a server.
    pub async fn restart_server(&self, server_id: &str) -> BitFunResult<()> {
        info!("Restarting MCP server: id={}", server_id);

        let config = self
            .config_service
            .get_server_config(server_id)
            .await?
            .ok_or_else(|| {
                BitFunError::NotFound(format!("MCP server config not found: {}", server_id))
            })?;

        match config.server_type {
            super::MCPServerType::Local => {
                self.ensure_registered(server_id).await?;

                let process = self.registry.get_process(server_id).await.ok_or_else(|| {
                    BitFunError::NotFound(format!("MCP server not found: {}", server_id))
                })?;
                let mut proc = process.write().await;

                let command = config
                    .command
                    .as_ref()
                    .ok_or_else(|| BitFunError::Configuration("Missing command".to_string()))?;
                proc.restart(command, &config.args, &config.env).await?;
            }
            super::MCPServerType::Remote => {
                // Treat restart as reconnect for remote servers.
                self.ensure_registered(server_id).await?;
                let _ = self.stop_server(server_id).await;
                self.start_server(server_id).await?;
            }
            _ => {
                return Err(BitFunError::NotImplemented(
                    "Restart not supported for this server type".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Returns server status.
    pub async fn get_server_status(&self, server_id: &str) -> BitFunResult<MCPServerStatus> {
        if !self.registry.contains(server_id).await {
            // If the server exists in config but isn't registered yet, register it so status
            // reflects reality (Uninitialized) instead of heuristics in the UI.
            let _ = self.ensure_registered(server_id).await;
        }

        let process =
            self.registry.get_process(server_id).await.ok_or_else(|| {
                BitFunError::NotFound(format!("MCP server not found: {}", server_id))
            })?;

        let proc = process.read().await;
        Ok(proc.status().await)
    }

    /// Returns the current status detail/message for one server.
    pub async fn get_server_status_message(&self, server_id: &str) -> BitFunResult<Option<String>> {
        if !self.registry.contains(server_id).await {
            let _ = self.ensure_registered(server_id).await;
        }

        let process =
            self.registry.get_process(server_id).await.ok_or_else(|| {
                BitFunError::NotFound(format!("MCP server not found: {}", server_id))
            })?;

        let proc = process.read().await;
        Ok(proc.status_message().await)
    }

    /// Returns statuses of all servers.
    pub async fn get_all_server_statuses(&self) -> Vec<(String, MCPServerStatus)> {
        let processes = self.registry.get_all_processes().await;
        let mut statuses = Vec::new();

        for process in processes {
            let proc = process.read().await;
            let id = proc.id().to_string();
            let status = proc.status().await;
            statuses.push((id, status));
        }

        statuses
    }

    /// Returns a connection.
    pub async fn get_connection(&self, server_id: &str) -> Option<Arc<MCPConnection>> {
        self.connection_pool.get_connection(server_id).await
    }

    /// Returns cached MCP resources for a server.
    pub async fn get_cached_resources(&self, server_id: &str) -> Vec<MCPResource> {
        self.resource_catalog_cache
            .read()
            .await
            .get(server_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Returns cached MCP prompts for a server.
    pub async fn get_cached_prompts(&self, server_id: &str) -> Vec<MCPPrompt> {
        self.prompt_catalog_cache
            .read()
            .await
            .get(server_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Refreshes resources catalog cache for one server.
    pub async fn refresh_server_resource_catalog(&self, server_id: &str) -> BitFunResult<usize> {
        let connection = self.get_connection(server_id).await.ok_or_else(|| {
            BitFunError::NotFound(format!("MCP server connection not found: {}", server_id))
        })?;
        self.refresh_resources_catalog(server_id, connection).await
    }

    /// Refreshes prompts catalog cache for one server.
    pub async fn refresh_server_prompt_catalog(&self, server_id: &str) -> BitFunResult<usize> {
        let connection = self.get_connection(server_id).await.ok_or_else(|| {
            BitFunError::NotFound(format!("MCP server connection not found: {}", server_id))
        })?;
        self.refresh_prompts_catalog(server_id, connection).await
    }

    /// Returns all server IDs.
    pub async fn get_all_server_ids(&self) -> Vec<String> {
        self.registry.get_all_server_ids().await
    }

    /// Adds a server.
    pub async fn add_server(&self, config: MCPServerConfig) -> BitFunResult<()> {
        config.validate()?;

        self.config_service.save_server_config(&config).await?;

        self.registry.register(&config).await?;

        if config.enabled && config.auto_start {
            self.start_server(&config.id).await?;
        }

        Ok(())
    }

    /// Removes a server.
    pub async fn remove_server(&self, server_id: &str) -> BitFunResult<()> {
        info!("Removing MCP server: id={}", server_id);

        self.stop_connection_event_listener(server_id).await;

        match self.registry.unregister(server_id).await {
            Ok(_) => {
                info!("Unregistered MCP server: id={}", server_id);
            }
            Err(e) => {
                warn!(
                    "Server not running, skipping unregister: id={} error={}",
                    server_id, e
                );
            }
        }

        self.config_service.delete_server_config(server_id).await?;
        self.clear_reconnect_state(server_id).await;
        self.resource_catalog_cache.write().await.remove(server_id);
        self.prompt_catalog_cache.write().await.remove(server_id);
        info!("Deleted MCP server config: id={}", server_id);

        Ok(())
    }

    /// Updates server configuration.
    pub async fn update_server_config(&self, config: MCPServerConfig) -> BitFunResult<()> {
        config.validate()?;

        self.config_service.save_server_config(&config).await?;

        let status = self.get_server_status(&config.id).await?;
        if matches!(
            status,
            MCPServerStatus::Connected | MCPServerStatus::Healthy
        ) {
            info!(
                "Restarting MCP server to apply new configuration: id={}",
                config.id
            );
            self.restart_server(&config.id).await?;
        } else if config.enabled
            && config.auto_start
            && matches!(
                status,
                MCPServerStatus::NeedsAuth
                    | MCPServerStatus::Failed
                    | MCPServerStatus::Reconnecting
                    | MCPServerStatus::Stopped
                    | MCPServerStatus::Uninitialized
            )
        {
            info!(
                "Starting MCP server after configuration update: id={} previous_status={:?}",
                config.id, status
            );
            let _ = self.start_server(&config.id).await;
        }

        Ok(())
    }

    /// Shuts down all servers.
    pub async fn shutdown(&self) -> BitFunResult<()> {
        info!("Shutting down all MCP servers");

        let server_ids = self.registry.get_all_server_ids().await;
        for server_id in server_ids {
            if let Err(e) = self.stop_server(&server_id).await {
                error!("Failed to stop MCP server: id={} error={}", server_id, e);
            }
        }

        self.registry.clear().await?;
        self.reconnect_states.write().await.clear();
        self.resource_catalog_cache.write().await.clear();
        self.prompt_catalog_cache.write().await.clear();
        self.pending_interactions.write().await.clear();
        let mut event_tasks = self.connection_event_tasks.write().await;
        for (_, handle) in event_tasks.drain() {
            handle.abort();
        }

        info!("All MCP servers shut down");
        Ok(())
    }

    /// Registers MCP tools into the global tool registry.
    async fn register_mcp_tools(
        server_id: &str,
        server_name: &str,
        connection: Arc<MCPConnection>,
    ) -> BitFunResult<usize> {
        info!(
            "Registering MCP tools: server_name={} server_id={}",
            server_name, server_id
        );

        let mut adapter = MCPToolAdapter::new();

        adapter
            .load_tools_from_server(server_id, server_name, connection)
            .await
            .map_err(|e| {
                error!(
                    "Failed to load tools from MCP server: server_name={} server_id={} error={}",
                    server_name, server_id, e
                );
                e
            })?;

        let tools = adapter.get_tools();
        let tool_count = tools.len();

        for tool in tools {
            debug!(
                "Loaded MCP tool: name={} server={}",
                tool.name(),
                server_name
            );
        }

        let registry = crate::agentic::tools::registry::get_global_tool_registry();
        let mut registry_lock = registry.write().await;

        let tools_to_register = adapter.get_tools().to_vec();
        registry_lock.register_mcp_tools(tools_to_register);
        drop(registry_lock);

        info!(
            "Registered {} MCP tools: server_name={} server_id={}",
            tool_count, server_name, server_id
        );

        Ok(tool_count)
    }

    /// Unregisters MCP tools from the global tool registry.
    async fn unregister_mcp_tools(server_id: &str) {
        let registry = crate::agentic::tools::registry::get_global_tool_registry();
        let mut registry_lock = registry.write().await;
        registry_lock.unregister_mcp_server_tools(server_id);
        info!("Unregistered MCP tools: server_id={}", server_id);
    }
}

#[cfg(test)]
mod tests {
    use super::{ListChangedKind, MCPServerManager};
    use std::time::Duration;

    #[test]
    fn backoff_delay_grows_exponentially_and_caps() {
        let base = Duration::from_secs(2);
        let max = Duration::from_secs(60);

        assert_eq!(
            MCPServerManager::compute_backoff_delay(base, max, 1),
            Duration::from_secs(2)
        );
        assert_eq!(
            MCPServerManager::compute_backoff_delay(base, max, 2),
            Duration::from_secs(4)
        );
        assert_eq!(
            MCPServerManager::compute_backoff_delay(base, max, 5),
            Duration::from_secs(32)
        );
        assert_eq!(
            MCPServerManager::compute_backoff_delay(base, max, 10),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn detect_list_changed_kind_supports_three_catalogs() {
        assert_eq!(
            MCPServerManager::detect_list_changed_kind("notifications/tools/list_changed"),
            Some(ListChangedKind::Tools)
        );
        assert_eq!(
            MCPServerManager::detect_list_changed_kind("notifications/prompts/list_changed"),
            Some(ListChangedKind::Prompts)
        );
        assert_eq!(
            MCPServerManager::detect_list_changed_kind("notifications/resources/list_changed"),
            Some(ListChangedKind::Resources)
        );
        assert_eq!(
            MCPServerManager::detect_list_changed_kind("notifications/unknown"),
            None
        );
    }
}
