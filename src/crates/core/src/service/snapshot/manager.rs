use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::agentic::tools::registry::{get_global_tool_registry, ToolRegistry};
use crate::infrastructure::get_workspace_path;
use crate::service::snapshot::service::SnapshotService;
use crate::service::snapshot::types::{
    OperationType, SnapshotConfig, SnapshotError, SnapshotResult,
};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Snapshot manager
///
/// Manages all components of the snapshot system.
pub struct SnapshotManager {
    snapshot_service: Arc<RwLock<SnapshotService>>,
    original_tools: Vec<Arc<dyn Tool>>,
    file_modification_tools: HashSet<String>,
    initialized: bool,
}

impl SnapshotManager {
    /// Creates a new snapshot manager.
    pub async fn new(
        workspace_dir: PathBuf,
        config: Option<SnapshotConfig>,
    ) -> SnapshotResult<Self> {
        info!(
            "Creating snapshot manager: workspace={}",
            workspace_dir.display()
        );

        let mut snapshot_service = SnapshotService::new(workspace_dir, config);
        snapshot_service.initialize().await?;
        let snapshot_service = Arc::new(RwLock::new(snapshot_service));

        let original_tools = ToolRegistry::new().get_all_tools();

        let file_modification_tools = [
            "Write",
            "Edit",
            "Delete",
            "write_file",
            "edit_file",
            "create_file",
            "delete_file",
            "rename_file",
            "move_file",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Ok(Self {
            snapshot_service,
            original_tools,
            file_modification_tools,
            initialized: true,
        })
    }

    /// Returns whether the tool modifies files.
    fn is_file_modification_tool(&self, tool_name: &str) -> bool {
        self.file_modification_tools.contains(tool_name)
    }

    /// Returns wrapped tool list.
    pub fn get_wrapped_tools(&self) -> Vec<Arc<dyn Tool>> {
        if !self.initialized {
            error!("Snapshot manager not initialized");
            return vec![];
        }

        let mut wrapped_tools: Vec<Arc<dyn Tool>> = Vec::new();

        for tool in &self.original_tools {
            if self.is_file_modification_tool(tool.name()) {
                let wrapped_tool: Arc<dyn Tool> = Arc::new(WrappedTool::new(
                    tool.clone(),
                    self.snapshot_service.clone(),
                ));
                wrapped_tools.push(wrapped_tool);
            } else {
                wrapped_tools.push(tool.clone());
            }
        }

        wrapped_tools
    }

    /// Records a file change.
    pub async fn record_file_change(
        &self,
        session_id: &str,
        turn_index: usize,
        file_path: PathBuf,
        operation_type: OperationType,
        tool_name: String,
    ) -> SnapshotResult<String> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service
            .record_file_change(session_id, turn_index, file_path, operation_type, tool_name)
            .await
    }

    /// Rolls back a session.
    pub async fn rollback_session(&self, session_id: &str) -> SnapshotResult<Vec<PathBuf>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.rollback_session(session_id).await
    }

    /// Rolls back to a specific turn.
    pub async fn rollback_to_turn(
        &self,
        session_id: &str,
        turn_index: usize,
    ) -> SnapshotResult<Vec<PathBuf>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service
            .rollback_to_turn(session_id, turn_index)
            .await
    }

    /// Accepts all changes in a session.
    pub async fn accept_session(&self, session_id: &str) -> SnapshotResult<()> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.accept_session(session_id).await
    }

    /// Accepts changes for a single file.
    pub async fn accept_file(&self, session_id: &str, file_path: &str) -> SnapshotResult<()> {
        let snapshot_service = self.snapshot_service.read().await;
        let file_path = std::path::Path::new(file_path);
        snapshot_service.accept_file(session_id, file_path).await
    }

    /// Returns the list of files affected by a session.
    pub async fn get_session_files(&self, session_id: &str) -> SnapshotResult<Vec<PathBuf>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.get_session_files(session_id).await
    }

    /// Returns the list of turns for a session.
    pub async fn get_session_turns(&self, session_id: &str) -> SnapshotResult<Vec<usize>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.get_session_turns(session_id).await
    }

    /// Returns the list of files modified in a turn.
    pub async fn get_turn_files(
        &self,
        session_id: &str,
        turn_index: usize,
    ) -> SnapshotResult<Vec<PathBuf>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service
            .get_turn_files(session_id, turn_index)
            .await
    }

    /// Returns the diff content for a file.
    pub async fn get_file_diff(
        &self,
        session_id: &str,
        file_path: &str,
        anchor_operation_id: Option<&str>,
    ) -> SnapshotResult<serde_json::Value> {
        let snapshot_service = self.snapshot_service.read().await;
        let file_path = std::path::Path::new(file_path);
        let (original, modified, anchor_line) = snapshot_service
            .get_file_diff_with_anchor(session_id, file_path, anchor_operation_id)
            .await?;

        Ok(serde_json::json!({
            "file_path": file_path.to_string_lossy(),
            "original_content": original,
            "modified_content": modified,
            "anchor_line": anchor_line,
        }))
    }

    pub async fn get_operation_summary(
        &self,
        session_id: &str,
        operation_id: &str,
    ) -> SnapshotResult<serde_json::Value> {
        let snapshot_service = self.snapshot_service.read().await;
        let op = snapshot_service
            .get_operation_summary(session_id, operation_id)
            .await?;
        Ok(serde_json::json!({
            "operation_id": op.operation_id,
            "session_id": op.session_id,
            "turn_index": op.turn_index,
            "seq_in_turn": op.seq_in_turn,
            "file_path": op.file_path.to_string_lossy(),
            "operation_type": format!("{:?}", op.operation_type),
            "tool_name": op.tool_context.tool_name,
            "lines_added": op.diff_summary.lines_added,
            "lines_removed": op.diff_summary.lines_removed,
        }))
    }

    /// Returns session statistics.
    pub async fn get_session_stats(&self, session_id: &str) -> SnapshotResult<serde_json::Value> {
        let snapshot_service = self.snapshot_service.read().await;
        let stats = snapshot_service.get_session_stats(session_id).await?;

        serde_json::to_value(stats).map_err(|e| {
            SnapshotError::ConfigError(format!("Failed to serialize statistics: {}", e))
        })
    }

    /// Returns system statistics.
    pub async fn get_system_stats(&self) -> SnapshotResult<serde_json::Value> {
        let snapshot_service = self.snapshot_service.read().await;
        let stats = snapshot_service.get_system_stats().await?;

        serde_json::to_value(stats).map_err(|e| {
            SnapshotError::ConfigError(format!("Failed to serialize system statistics: {}", e))
        })
    }

    pub async fn list_sessions(&self) -> SnapshotResult<Vec<String>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.list_sessions().await
    }

    pub async fn cleanup_snapshot_data(&self, keep_recent_days: u64) -> SnapshotResult<()> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service
            .cleanup_snapshot_data(keep_recent_days)
            .await
    }

    /// Tries to acquire a file lock.
    pub async fn try_acquire_file_lock(
        &self,
        session_id: &str,
        file_path: &str,
        tool_name: &str,
    ) -> SnapshotResult<bool> {
        let snapshot_service = self.snapshot_service.read().await;
        let file_path = std::path::Path::new(file_path);
        snapshot_service
            .try_acquire_file_lock(session_id, file_path, tool_name)
            .await
    }

    /// Releases a file lock.
    pub async fn release_file_lock(&self, session_id: &str, file_path: &str) -> SnapshotResult<()> {
        let snapshot_service = self.snapshot_service.read().await;
        let file_path = std::path::Path::new(file_path);
        snapshot_service
            .release_file_lock(session_id, file_path)
            .await
    }

    /// Returns file lock status.
    pub async fn get_file_lock_status(&self, file_path: &str) -> SnapshotResult<serde_json::Value> {
        let snapshot_service = self.snapshot_service.read().await;
        let file_path = std::path::Path::new(file_path);

        let lock_status = snapshot_service.get_file_lock_status(file_path).await?;
        Ok(serde_json::json!({
            "locked": lock_status.is_some(),
            "lock_info": lock_status
        }))
    }

    /// Detects file conflicts.
    pub async fn detect_file_conflict(
        &self,
        session_id: &str,
        file_path: &str,
        tool_name: &str,
    ) -> SnapshotResult<serde_json::Value> {
        let snapshot_service = self.snapshot_service.read().await;
        let file_path = std::path::Path::new(file_path);

        let conflict = snapshot_service
            .detect_file_conflict(session_id, file_path, tool_name)
            .await?;

        Ok(serde_json::json!({
            "has_conflict": conflict.is_some(),
            "conflict_info": conflict
        }))
    }

    /// Checks Git isolation status.
    pub async fn check_git_isolation(&self) -> SnapshotResult<bool> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.check_git_isolation().await
    }

    /// Returns the change history for a file.
    pub async fn get_file_change_history(
        &self,
        file_path: &std::path::Path,
    ) -> SnapshotResult<Vec<crate::service::snapshot::snapshot_core::FileChangeEntry>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.get_file_change_history(file_path).await
    }

    /// Returns the list of all modified files.
    pub async fn get_all_modified_files(&self) -> SnapshotResult<Vec<PathBuf>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.get_all_modified_files().await
    }

    /// Returns a reference to the snapshot service (for advanced operations).
    pub fn get_snapshot_service(&self) -> Arc<RwLock<SnapshotService>> {
        self.snapshot_service.clone()
    }
}

/// Wrapped tool
///
/// Wraps file-modification tools with snapshot functionality.
struct WrappedTool {
    original_tool: Arc<dyn Tool>,
    snapshot_service: Arc<RwLock<SnapshotService>>,
}

impl WrappedTool {
    fn new(original_tool: Arc<dyn Tool>, snapshot_service: Arc<RwLock<SnapshotService>>) -> Self {
        Self {
            original_tool,
            snapshot_service,
        }
    }
}

#[async_trait]
impl Tool for WrappedTool {
    fn name(&self) -> &str {
        self.original_tool.name()
    }

    async fn description(&self) -> crate::util::errors::BitFunResult<String> {
        Ok(self.original_tool.description().await?)
    }

    fn input_schema(&self) -> Value {
        self.original_tool.input_schema()
    }

    fn input_json_schema(&self) -> Option<Value> {
        self.original_tool.input_json_schema()
    }

    fn user_facing_name(&self) -> String {
        format!("{}", self.original_tool.user_facing_name())
    }

    async fn is_enabled(&self) -> bool {
        self.original_tool.is_enabled().await
    }

    fn is_readonly(&self) -> bool {
        self.original_tool.is_readonly()
    }

    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        self.original_tool.is_concurrency_safe(input)
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> crate::agentic::tools::framework::ValidationResult {
        let original_validation = self.original_tool.validate_input(input, context).await;

        if !original_validation.result {
            return original_validation;
        }

        original_validation
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        let original_render = self.original_tool.render_result_for_assistant(output);
        format!(
            "{}\n\nModification recorded to snapshot system, can be reviewed and managed in the review panel.",
            original_render
        )
    }

    fn render_tool_use_message(
        &self,
        input: &Value,
        options: &crate::agentic::tools::framework::ToolRenderOptions,
    ) -> String {
        let original_message = self.original_tool.render_tool_use_message(input, options);
        format!("{}", original_message)
    }

    fn render_tool_use_rejected_message(&self) -> String {
        format!("{}", self.original_tool.render_tool_use_rejected_message())
    }

    fn render_tool_result_message(&self, output: &Value) -> String {
        let original_message = self.original_tool.render_tool_result_message(output);
        format!("{} recorded to snapshot", original_message)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> crate::util::errors::BitFunResult<Vec<ToolResult>> {
        let file_modification_tools = [
            "Write",
            "Edit",
            "Delete",
            "write_file",
            "edit_file",
            "create_file",
            "delete_file",
            "rename_file",
            "move_file",
            "search_replace",
        ];

        if file_modification_tools.contains(&self.name()) {
            debug!(
                "Intercepting file modification tool: tool_name={}",
                self.name()
            );

            match self.handle_file_modification_internal(input, context).await {
                Ok(results) => {
                    return Ok(results);
                }
                Err(e) => {
                    warn!("Snapshot processing failed, falling back to original tool: tool_name={} error={}", self.name(), e);
                    let error_msg = format!("{}", e);
                    if error_msg.contains("file not found") || error_msg.contains("not exist") {
                        warn!("Possible workspace path mismatch, check snapshot workspace and global workspace consistency");
                    }
                }
            }
        }

        self.original_tool.call(input, context).await
    }
}

impl WrappedTool {
    /// Handles a file-modification tool.
    async fn handle_file_modification_internal(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> crate::util::errors::BitFunResult<Vec<ToolResult>> {
        let session_id = context.session_id.clone().ok_or_else(|| {
            crate::util::errors::BitFunError::Tool(
                "session_id is required in ToolUseContext".to_string(),
            )
        })?;

        let raw_path = match self.extract_file_path_simple(input) {
            Ok(path) => path,
            Err(e) => return Err(crate::util::errors::BitFunError::Tool(e.to_string())),
        };

        let snapshot_workspace = {
            let snapshot_service = self.snapshot_service.read().await;
            snapshot_service.get_workspace_dir().to_path_buf()
        };

        let file_path = if raw_path.is_absolute() {
            raw_path.clone()
        } else {
            snapshot_workspace.join(&raw_path)
        };

        let is_create_tool = matches!(self.name(), "Write" | "write_file" | "create_file");

        if !file_path.exists() && !is_create_tool {
            error!(
                "File not found: file_path={} raw_path={} snapshot_workspace={}",
                file_path.display(),
                raw_path.display(),
                snapshot_workspace.display()
            );

            if let Some(global_workspace) = get_workspace_path() {
                let global_resolved = if raw_path.is_absolute() {
                    raw_path.clone()
                } else {
                    global_workspace.join(&raw_path)
                };

                if global_resolved.exists() && global_resolved != file_path {
                    error!(
                        "Workspace path mismatch detected: snapshot_path={} global_path={}",
                        file_path.display(),
                        global_resolved.display()
                    );
                }
            }

            return Err(crate::util::errors::BitFunError::Tool(format!(
                "File not found: {} (Snapshot workspace: {})",
                file_path.display(),
                snapshot_workspace.display()
            )));
        }

        if is_create_tool && !file_path.exists() {
            debug!("Creating new file: file_path={}", file_path.display());
        }

        let turn_index = self.extract_turn_index(context);

        let snapshot_service = self.snapshot_service.read().await;
        let operation_id = snapshot_service
            .intercept_file_modification(
                &session_id,
                turn_index,
                self.name(),
                input.clone(),
                &file_path,
                self.get_operation_type_internal(),
                context.tool_call_id.clone(),
            )
            .await
            .map_err(|e| crate::util::errors::BitFunError::Tool(e.to_string()))?;

        debug!(
            "Recorded file modification operation: operation_id={}",
            operation_id
        );

        let start_time = std::time::Instant::now();
        let results = self.original_tool.call(input, context).await?;
        let duration_ms = start_time.elapsed().as_millis() as u64;

        snapshot_service
            .complete_file_modification(&session_id, &operation_id, duration_ms)
            .await
            .map_err(|e| crate::util::errors::BitFunError::Tool(e.to_string()))?;

        debug!(
            "File modification tool completed: tool_name={}",
            self.name()
        );
        Ok(results)
    }

    /// Extracts the turn index.
    fn extract_turn_index(&self, context: &ToolUseContext) -> usize {
        context
            .options
            .as_ref()
            .and_then(|opts| opts.custom_data.as_ref())
            .and_then(|data| data.get("turn_index"))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(0)
    }

    /// Simplified file path extraction.
    fn extract_file_path_simple(&self, input: &Value) -> SnapshotResult<PathBuf> {
        let possible_fields = ["file_path", "path", "target_file", "filename"];

        for field in &possible_fields {
            if let Some(path_value) = input.get(field) {
                if let Some(path_str) = path_value.as_str() {
                    return Ok(PathBuf::from(path_str));
                }
            }
        }

        Err(SnapshotError::ConfigError(
            "Failed to extract file path from tool input".to_string(),
        ))
    }

    /// Returns the operation type.
    fn get_operation_type_internal(&self) -> OperationType {
        match self.name() {
            "create_file" => OperationType::Create,
            "delete_file" | "Delete" => OperationType::Delete,
            "rename_file" | "move_file" => OperationType::Rename,
            _ => OperationType::Modify,
        }
    }
}

/// Global snapshot manager instance
static mut GLOBAL_SNAPSHOT_MANAGER: Option<Arc<SnapshotManager>> = None;

/// Initializes the global snapshot manager.
pub async fn initialize_global_snapshot_manager(
    workspace_dir: PathBuf,
    config: Option<SnapshotConfig>,
) -> SnapshotResult<()> {
    let manager = SnapshotManager::new(workspace_dir, config).await?;
    let manager_arc = Arc::new(manager);

    unsafe {
        GLOBAL_SNAPSHOT_MANAGER = Some(manager_arc);
    }

    if let Some(manager) = get_global_snapshot_manager() {
        let wrapped_tools = manager.get_wrapped_tools();
        let registry = get_global_tool_registry();
        let mut registry_lock = registry.write().await;
        for tool in wrapped_tools {
            registry_lock.register_tool(tool);
        }
        info!("Refreshed global tool registry for snapshot interception");
    }

    info!("Global snapshot manager initialized");
    Ok(())
}

/// Gets the global snapshot manager.
#[allow(static_mut_refs)]
pub fn get_global_snapshot_manager() -> Option<Arc<SnapshotManager>> {
    unsafe { GLOBAL_SNAPSHOT_MANAGER.clone() }
}

/// Ensures the global snapshot manager has been initialized.
pub fn ensure_global_snapshot_manager() -> SnapshotResult<Arc<SnapshotManager>> {
    get_global_snapshot_manager().ok_or_else(|| {
        SnapshotError::ConfigError(
            "Global snapshot manager not initialized, please call initialize_global_snapshot_manager first".to_string(),
        )
    })
}
