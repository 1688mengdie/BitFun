//! SessionControl manages persisted workspace-scoped sessions.
//!
//! The `cancel` action only cancels the target session's current running dialog turn.
//! It does not permanently stop the session itself, and it does not clear queued
//! messages that may still run later through the scheduler.

use super::util::normalize_path;
use crate::agentic::coordination::{get_global_coordinator, get_global_scheduler};
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_agent_runtime::sdk::AgentRuntime;
use bitfun_agent_runtime::session_control::{
    render_session_control_tool_use_message, resolve_session_control_cancel_route,
    session_control_agent_type_or_default, session_control_cancel_result_message,
    session_control_cancel_status, session_control_created_result_message,
    session_control_creator_marker, session_control_deleted_result_message,
    session_control_renamed_result_message, session_control_session_name_or_default,
    validate_session_control_input, validate_session_id, SessionControlAction,
    SessionControlCancelRoute, SessionControlInput, SessionControlValidationContext,
    SessionControlValidationResult,
};
use bitfun_core_types::SessionExecutionTarget;
use bitfun_runtime_ports::{
    AgentSessionCreateRequest, AgentSessionDeleteRequest, AgentSessionListRequest,
    AgentSessionRenameRequest, AgentSessionSummary, AgentSessionWorkspaceBinding,
    AgentSessionWorkspaceRequest, AgentSubmissionSource, AgentTurnCancellationRequest,
};
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// SessionControl tool - create, cancel, delete, rename, or list persisted sessions
pub struct SessionControlTool;

const CANCEL_WAIT_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
struct SessionControlWorkspaceTarget {
    display_workspace: String,
    project_workspace: String,
    execution_target: Option<SessionExecutionTarget>,
    workspace_id: Option<String>,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
}

impl Default for SessionControlTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionControlTool {
    pub fn new() -> Self {
        Self
    }

    fn current_workspace_session<'a>(
        &self,
        context: &'a ToolUseContext,
        workspace: &str,
    ) -> Option<&'a str> {
        let current_session_id = context.session_id.as_deref()?;
        let current_workspace = context.workspace_root()?;
        let normalized_current_workspace =
            normalize_path(current_workspace.to_string_lossy().as_ref());

        if normalized_current_workspace == workspace {
            Some(current_session_id)
        } else {
            None
        }
    }

    fn escape_markdown_table_cell(value: &str) -> String {
        value
            .replace('\\', "\\\\")
            .replace('|', "\\|")
            .replace('\n', "<br>")
    }

    fn format_system_time(time: SystemTime) -> String {
        let datetime: chrono::DateTime<chrono::Local> = time.into();
        datetime.format("%Y-%m-%dT%H:%M:%S").to_string()
    }

    fn creator_session_marker(&self, context: &ToolUseContext) -> BitFunResult<String> {
        let creator_session_id = context.session_id.as_ref().ok_or_else(|| {
            BitFunError::tool("create requires a creator session in tool context".to_string())
        })?;
        Ok(session_control_creator_marker(creator_session_id))
    }

    async fn resolve_effective_workspace(
        &self,
        action: SessionControlAction,
        session_id: Option<&str>,
        context: &ToolUseContext,
        runtime: &AgentRuntime,
    ) -> BitFunResult<SessionControlWorkspaceTarget> {
        match action {
            SessionControlAction::Cancel
            | SessionControlAction::Delete
            | SessionControlAction::Rename => {
                let session_id = session_id.ok_or_else(|| {
                    BitFunError::tool(format!("session_id is required for {}", action.as_str()))
                })?;
                if let Some(binding) = runtime
                    .resolve_session_workspace_binding(AgentSessionWorkspaceRequest {
                        session_id: session_id.to_string(),
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?
                {
                    return Ok(Self::workspace_target_from_binding(binding));
                }
                Err(BitFunError::NotFound(format!(
                    "Workspace for session '{}' could not be resolved",
                    session_id
                )))
            }
            SessionControlAction::Create | SessionControlAction::List => {
                let workspace = context.workspace.as_ref().ok_or_else(|| {
                    BitFunError::tool(format!(
                        "workspace is required for {} when the current workspace is unavailable",
                        action.as_str()
                    ))
                })?;
                Ok(Self::workspace_target_from_context(workspace))
            }
        }
    }

    fn workspace_target_from_context(
        workspace: &crate::agentic::WorkspaceBinding,
    ) -> SessionControlWorkspaceTarget {
        SessionControlWorkspaceTarget {
            display_workspace: normalize_path(&workspace.root_path_string()),
            project_workspace: normalize_path(&workspace.project_root_path_string()),
            execution_target: workspace.execution_target.clone(),
            workspace_id: workspace.workspace_id.clone(),
            remote_connection_id: workspace.connection_id().map(ToOwned::to_owned),
            remote_ssh_host: if workspace.is_remote() {
                Some(workspace.session_identity.hostname.clone())
                    .filter(|value| !value.trim().is_empty())
            } else {
                None
            },
        }
    }

    fn workspace_target_from_binding(
        binding: AgentSessionWorkspaceBinding,
    ) -> SessionControlWorkspaceTarget {
        let project_workspace = binding
            .project_workspace_path
            .clone()
            .unwrap_or_else(|| binding.workspace_path.clone());
        SessionControlWorkspaceTarget {
            display_workspace: binding.workspace_path,
            project_workspace,
            execution_target: binding.execution_target,
            workspace_id: binding.workspace_id,
            remote_connection_id: binding.remote_connection_id,
            remote_ssh_host: binding.remote_ssh_host,
        }
    }

    fn rename_request(
        workspace: &SessionControlWorkspaceTarget,
        session_id: &str,
        session_name: &str,
    ) -> AgentSessionRenameRequest {
        AgentSessionRenameRequest {
            workspace_path: workspace.project_workspace.clone(),
            session_id: session_id.to_string(),
            session_name: session_name.to_string(),
            remote_connection_id: workspace.remote_connection_id.clone(),
            remote_ssh_host: workspace.remote_ssh_host.clone(),
        }
    }

    fn validation_context(context: Option<&ToolUseContext>) -> SessionControlValidationContext<'_> {
        SessionControlValidationContext {
            current_session_id: context.and_then(|value| value.session_id.as_deref()),
            has_workspace_root: context.and_then(|value| value.workspace_root()).is_some(),
        }
    }

    fn into_validation_result(result: SessionControlValidationResult) -> ValidationResult {
        ValidationResult {
            result: result.result,
            message: result.message,
            error_code: result.error_code,
            meta: result.meta,
        }
    }

    async fn ensure_session_exists(
        &self,
        runtime: &AgentRuntime,
        workspace: &SessionControlWorkspaceTarget,
        session_id: &str,
    ) -> BitFunResult<()> {
        let existing_sessions = runtime
            .list_sessions(AgentSessionListRequest {
                workspace_path: workspace.project_workspace.clone(),
                remote_connection_id: workspace.remote_connection_id.clone(),
                remote_ssh_host: workspace.remote_ssh_host.clone(),
            })
            .await
            .map_err(|error| {
                BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
            })?;
        if existing_sessions
            .iter()
            .any(|session| session.session_id == session_id)
        {
            Ok(())
        } else {
            Err(BitFunError::NotFound(format!(
                "Session '{}' not found in workspace '{}'",
                session_id, workspace.display_workspace
            )))
        }
    }

    fn system_time_from_epoch_ms(epoch_ms: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(epoch_ms)
    }

    fn build_list_result_for_assistant(
        &self,
        workspace: &str,
        sessions: &[AgentSessionSummary],
        current_session_id: Option<&str>,
    ) -> String {
        if sessions.is_empty() {
            return format!("No sessions found in workspace '{}'.", workspace);
        }

        let mut lines = vec![format!(
            "Found {} session(s) in workspace '{}'",
            sessions.len(),
            workspace
        )];
        lines.push(String::new());
        if let Some(current_session_id) = current_session_id {
            lines.push(format!("Note: '{}' is your session_id", current_session_id));
            lines.push(String::new());
        }
        lines.push(
            "| session_id | session_name | agent_type | created_at | last_active_at |".to_string(),
        );
        lines.push("| --- | --- | --- | --- | --- |".to_string());
        for session in sessions {
            lines.push(format!(
                "| {} | {} | {} | {} | {} |",
                Self::escape_markdown_table_cell(&session.session_id),
                Self::escape_markdown_table_cell(&session.session_name),
                Self::escape_markdown_table_cell(&session.agent_type),
                Self::format_system_time(Self::system_time_from_epoch_ms(session.created_at_ms)),
                Self::format_system_time(Self::system_time_from_epoch_ms(
                    session.last_active_at_ms
                )),
            ));
        }
        lines.join("\n")
    }
}

#[async_trait]
impl Tool for SessionControlTool {
    fn name(&self) -> &str {
        "SessionControl"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Manage persisted workspace-scoped agent sessions.

Actions:
- "create": Create a new session. You may optionally provide session_name and agent_type.
- "cancel": Cancel the target session's currently running dialog turn. This does not delete the session or clear any queued messages that may still run later.
- "delete": Delete an existing session by session_id.
- "rename": Rename an existing session by session_id using session_name as the new title.
- "list": List all sessions.

Arguments:
- "workspace": Absolute workspace path. Required for create and list. Ignored for cancel, delete, and rename.
- "session_name": Used by create (defaults to "New Session") and required as the new title for rename.
- "agent_type": Only used by create. Defaults to "agentic".
  - "agentic": Coding-focused agent for implementation, debugging, and code changes.
  - "Plan": Planning agent for clarifying requirements and producing an implementation plan before coding.
  - "Cowork": Collaborative agent for office-style work such as research, documentation, presentations, etc.
  - "DeepResearch": Research agent for systematic investigation and evidence-driven reports.
- "session_id": Required for cancel, delete, and rename."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Create, list, rename, cancel, and delete persisted agent sessions.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "cancel", "delete", "rename", "list"],
                    "description": "The session action to perform."
                },
                "workspace": {
                    "type": "string",
                    "description": "Required absolute workspace path for create and list. Ignored for cancel, delete, and rename."
                },
                "session_id": {
                    "type": "string",
                    "description": "Required for cancel, delete, and rename."
                },
                "session_name": {
                    "type": "string",
                    "description": "Optional display name when creating a session; required as the new title when renaming."
                },
                "agent_type": {
                    "type": "string",
                    "enum": ["agentic", "Plan", "Cowork", "DeepResearch"],
                    "description": "Optional agent type when creating a session. Defaults to agentic."
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: SessionControlInput = match serde_json::from_value(input.clone()) {
            Ok(value) => value,
            Err(err) => {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid input: {}", err)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        Self::into_validation_result(validate_session_control_input(
            &parsed,
            Self::validation_context(context),
        ))
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        render_session_control_tool_use_message(input)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: SessionControlInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;
        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
        let runtime = CoreServiceAgentRuntime::agent_runtime(coordinator.clone())
            .map_err(BitFunError::tool)?;

        match params.action {
            SessionControlAction::Create => {
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::Create,
                        None,
                        context,
                        &runtime,
                    )
                    .await?;
                let session_name =
                    session_control_session_name_or_default(params.session_name.as_deref());
                let agent_type = session_control_agent_type_or_default(params.agent_type.as_ref());
                let created_by = self.creator_session_marker(context)?;
                let mut metadata = serde_json::Map::new();
                metadata.insert("createdBy".to_string(), json!(created_by));
                let session = runtime
                    .create_session(AgentSessionCreateRequest {
                        session_name,
                        agent_type,
                        workspace_path: Some(workspace.display_workspace.clone()),
                        project_workspace_path: Some(workspace.project_workspace.clone()),
                        execution_target: workspace.execution_target.clone(),
                        workspace_id: workspace.workspace_id.clone(),
                        remote_connection_id: workspace.remote_connection_id.clone(),
                        remote_ssh_host: workspace.remote_ssh_host.clone(),
                        model_id: None,
                        metadata,
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;
                let created_session_id = session.session_id.clone();
                let created_session_name = session.session_name.clone();
                let created_agent_type = session.agent_type.clone();
                let result_for_assistant = session_control_created_result_message(
                    &created_session_id,
                    &workspace.display_workspace,
                    &created_agent_type,
                );

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "create",
                        "workspace": workspace.display_workspace.clone(),
                        "session": {
                            "session_id": created_session_id,
                            "session_name": created_session_name,
                            "agent_type": created_agent_type,
                        }
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            SessionControlAction::Cancel => {
                let session_id = params.session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("session_id is required for cancel".to_string())
                })?;
                validate_session_id(session_id).map_err(BitFunError::tool)?;
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::Cancel,
                        Some(session_id),
                        context,
                        &runtime,
                    )
                    .await?;
                if self.current_workspace_session(context, &workspace.display_workspace)
                    == Some(session_id)
                {
                    return Err(BitFunError::tool(
                        "cannot cancel the current session from SessionControl".to_string(),
                    ));
                }

                self.ensure_session_exists(&runtime, &workspace, session_id)
                    .await?;

                let scheduler = get_global_scheduler();
                let cancel_route = resolve_session_control_cancel_route(
                    context.session_id.as_deref(),
                    scheduler.is_some(),
                );
                let (runtime, requester_session_id) = match (cancel_route, scheduler) {
                    (
                        SessionControlCancelRoute::RequesterViaScheduler {
                            requester_session_id,
                        },
                        Some(scheduler),
                    ) => {
                        let runtime = CoreServiceAgentRuntime::agent_runtime_with_scheduler_ports(
                            coordinator.clone(),
                            scheduler,
                        )
                        .map_err(BitFunError::tool)?;
                        (runtime, Some(requester_session_id))
                    }
                    _ => {
                        // Fallback covers unusual tool contexts and startup states where the
                        // global scheduler is not available; concrete cancellation still works.
                        (runtime.clone(), None)
                    }
                };
                let cancelled_turn_id = runtime
                    .cancel_turn(AgentTurnCancellationRequest {
                        session_id: session_id.to_string(),
                        turn_id: None,
                        source: Some(AgentSubmissionSource::AgentSession),
                        requester_session_id,
                        reason: None,
                        wait_timeout_ms: Some(CANCEL_WAIT_TIMEOUT.as_millis() as u64),
                        cancel_descendants: true,
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?
                    .turn_id;
                let had_active_turn = cancelled_turn_id.is_some();
                let status = session_control_cancel_status(cancelled_turn_id.as_deref());
                let result_for_assistant = session_control_cancel_result_message(
                    session_id,
                    &workspace.display_workspace,
                    cancelled_turn_id.as_deref(),
                );

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "cancel",
                        "workspace": workspace.display_workspace.clone(),
                        "session_id": session_id,
                        "had_active_turn": had_active_turn,
                        "cancelled_turn_id": cancelled_turn_id,
                        "status": status,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            SessionControlAction::Delete => {
                let session_id = params.session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("session_id is required for delete".to_string())
                })?;
                validate_session_id(session_id).map_err(BitFunError::tool)?;
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::Delete,
                        Some(session_id),
                        context,
                        &runtime,
                    )
                    .await?;
                if self.current_workspace_session(context, &workspace.display_workspace)
                    == Some(session_id)
                {
                    return Err(BitFunError::tool(
                        "cannot delete the current session from SessionControl".to_string(),
                    ));
                }

                self.ensure_session_exists(&runtime, &workspace, session_id)
                    .await?;

                let scheduler = get_global_scheduler().ok_or_else(|| {
                    BitFunError::tool("scheduler not initialized for session deletion".to_string())
                })?;
                let deletion_runtime = CoreServiceAgentRuntime::agent_runtime_with_scheduler_ports(
                    coordinator.clone(),
                    scheduler,
                )
                .map_err(BitFunError::tool)?;

                deletion_runtime
                    .delete_session(AgentSessionDeleteRequest {
                        workspace_path: workspace.project_workspace.clone(),
                        session_id: session_id.to_string(),
                        remote_connection_id: workspace.remote_connection_id.clone(),
                        remote_ssh_host: workspace.remote_ssh_host.clone(),
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "delete",
                        "workspace": workspace.display_workspace.clone(),
                        "session_id": session_id,
                    }),
                    result_for_assistant: Some(session_control_deleted_result_message(
                        session_id,
                        &workspace.display_workspace,
                    )),
                    image_attachments: None,
                }])
            }
            SessionControlAction::Rename => {
                let session_id = params.session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("session_id is required for rename".to_string())
                })?;
                validate_session_id(session_id).map_err(BitFunError::tool)?;
                let session_name = params
                    .session_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "session_name is required and must not be empty for rename".to_string(),
                        )
                    })?;
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::Rename,
                        Some(session_id),
                        context,
                        &runtime,
                    )
                    .await?;
                if self.current_workspace_session(context, &workspace.display_workspace)
                    == Some(session_id)
                {
                    return Err(BitFunError::tool(
                        "cannot rename the current session from SessionControl".to_string(),
                    ));
                }

                // Reuse the same rename channel as the frontend
                // renameChatSessionTitle (AgentSessionManagementPort::rename_session)
                // so the persisted title stays consistent with the desktop/frontend.
                runtime
                    .rename_session(Self::rename_request(&workspace, session_id, session_name))
                    .await
                    .map_err(|error| {
                        BitFunError::tool(format!(
                            "cannot rename session '{session_id}': {}",
                            CoreServiceAgentRuntime::runtime_error_message(error)
                        ))
                    })?;

                let result_for_assistant = session_control_renamed_result_message(
                    session_id,
                    &workspace.display_workspace,
                    session_name,
                );
                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "rename",
                        "workspace": workspace.display_workspace.clone(),
                        "session_id": session_id,
                        "session_name": session_name,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            SessionControlAction::List => {
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::List,
                        None,
                        context,
                        &runtime,
                    )
                    .await?;
                // Cross-workspace listing requires authorization: the caller
                // may list the workspace it currently belongs to, but an
                // explicit `workspace` argument pointing elsewhere is only
                // allowed for the owner (a top-level session with no creator).
                // This prevents a delegated session from silently enumerating
                // other workspaces' session summaries.
                if let Some(caller_session_id) = context.session_id.as_deref() {
                    let current_workspace = context
                        .workspace_root()
                        .map(|path| normalize_path(path.to_string_lossy().as_ref()));
                    let explicit_workspace = normalize_path(&workspace.project_workspace);
                    let is_cross_workspace = current_workspace
                        .as_ref()
                        .is_none_or(|current| *current != explicit_workspace);
                    if is_cross_workspace
                        && !coordinator
                            .get_session_manager()
                            .get_session(caller_session_id)
                            .is_some_and(|session| session.created_by.is_none())
                    {
                        return Err(BitFunError::tool(format!(
                            "cannot list sessions in workspace '{}': caller session '{caller_session_id}' does not belong to that workspace and is not the owner",
                            workspace.display_workspace
                        )));
                    }
                }
                let sessions = runtime
                    .list_sessions(AgentSessionListRequest {
                        workspace_path: workspace.project_workspace.clone(),
                        remote_connection_id: workspace.remote_connection_id.clone(),
                        remote_ssh_host: workspace.remote_ssh_host.clone(),
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;
                let current_session_id =
                    self.current_workspace_session(context, &workspace.display_workspace);
                let result_for_assistant = self.build_list_result_for_assistant(
                    &workspace.display_workspace,
                    &sessions,
                    current_session_id,
                );

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "list",
                        "workspace": workspace.display_workspace.clone(),
                        "current_session_id": current_session_id,
                        "count": sessions.len(),
                        "sessions": sessions,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
        }
    }
}

/// Options for the SessionHistory export authorization gate.
///
/// `allow_owner_bypass` lets the owner session (a top-level session with no
/// creator, i.e. `created_by.is_none()`) export any transcript, mirroring the
/// owner semantics of session deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SessionHistoryAuthOptions {
    pub allow_owner_bypass: bool,
}

impl SessionHistoryAuthOptions {
    pub(crate) const fn read() -> Self {
        Self {
            allow_owner_bypass: true,
        }
    }
}

/// Authorize a SessionHistory transcript export against the caller session.
///
/// Decision chain (fail-closed — `Err` rejects the export):
/// 1. Same-workspace check: the caller and the target must share the same
///    session storage directory; cross-workspace exports are always rejected.
/// 2. Owner bypass: a top-level caller session (`created_by.is_none()`) may
///    export any transcript in its workspace when allowed by the options.
/// 3. Creator match: the target metadata `created_by` marker names the caller
///    (`session-<caller_session_id>`).
/// 4. In-tree ancestry: the caller is an ancestor of the target or the target
///    is an ancestor of the caller (either direction inside one session tree).
///    The chain is resolved from persisted session metadata
///    (`relationship.parent_session_id`) with cycle protection, so a corrupt
///    lineage cannot hang or bypass the gate.
fn same_session_storage_dir(a: &std::path::Path, b: &std::path::Path) -> bool {
    let canonical =
        |path: &std::path::Path| dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    canonical(a) == canonical(b)
}

#[allow(clippy::too_many_arguments)] // full authorization context; kept flat for call-site clarity
pub(crate) async fn resolve_session_read_authorization(
    session_manager: &crate::agentic::session::session_manager::SessionManager,
    caller_session_id: &str,
    caller_workspace_path: &std::path::Path,
    target_session_id: &str,
    target_workspace_path: &std::path::Path,
    action_label: &str,
    options: SessionHistoryAuthOptions,
) -> BitFunResult<()> {
    // Same-workspace containment: exporting a transcript from another
    // workspace is rejected regardless of any other relationship.
    if !same_session_storage_dir(caller_workspace_path, target_workspace_path) {
        return Err(BitFunError::tool(format!(
            "cannot {action_label} session '{target_session_id}': caller session '{caller_session_id}' belongs to a different workspace"
        )));
    }

    // Owner bypass: a top-level session (no creator) is the workspace owner.
    let caller_is_owner = options.allow_owner_bypass
        && session_manager
            .get_session(caller_session_id)
            .is_some_and(|session| session.created_by.is_none());

    // Creator match: the target was created by the caller session.
    let created_by_match = session_manager
        .load_session_metadata(target_workspace_path, target_session_id)
        .await
        .ok()
        .flatten()
        .and_then(|metadata| metadata.created_by)
        .is_some_and(|creator| creator == session_control_creator_marker(caller_session_id));

    if caller_is_owner || created_by_match {
        return Ok(());
    }

    // In-tree ancestry, both directions: ancestors may read descendants and
    // descendants may read ancestors. Walk the persisted parent chain from
    // each side with cycle protection (an empty or corrupt chain must not
    // bypass the gate — fail-closed below).
    let target_ancestors = collect_session_ancestor_chain(
        session_manager,
        target_workspace_path,
        target_session_id,
    )
    .await;
    if target_ancestors.iter().any(|id| id == caller_session_id) {
        return Ok(());
    }
    let caller_ancestors = collect_session_ancestor_chain(
        session_manager,
        caller_workspace_path,
        caller_session_id,
    )
    .await;
    if caller_ancestors.iter().any(|id| id == target_session_id) {
        return Ok(());
    }

    Err(BitFunError::tool(format!(
        "session '{caller_session_id}' is not authorized to {action_label} session '{target_session_id}': not the owner, not the creator, and not in the same session tree (ancestor/descendant)"
    )))
}

/// Collect the ancestor chain of a session from persisted session metadata
/// (`relationship.parent_session_id`), nearest first. Cycle protection stops
/// the walk on a corrupt lineage chain instead of hanging.
async fn collect_session_ancestor_chain(
    session_manager: &crate::agentic::session::session_manager::SessionManager,
    workspace_path: &std::path::Path,
    session_id: &str,
) -> Vec<String> {
    let mut ancestors = Vec::new();
    let mut visited = std::collections::HashSet::new();
    visited.insert(session_id.to_string());
    let mut current = session_id.to_string();
    loop {
        let metadata = session_manager
            .load_session_metadata(workspace_path, &current)
            .await
            .ok()
            .flatten();
        match metadata.and_then(|m| m.relationship.and_then(|r| r.parent_session_id)) {
            Some(parent_id) => {
                if !visited.insert(parent_id.clone()) {
                    // Cycle detected; stop walking to avoid hanging on a
                    // corrupt lineage chain.
                    break;
                }
                ancestors.push(parent_id.clone());
                current = parent_id;
            }
            None => break,
        }
    }
    ancestors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tools::framework::ToolUseContext;
    use crate::agentic::WorkspaceBinding;
    use bitfun_core_types::{
        SessionExecutionTarget, SessionExecutionTargetKind, WorktreeLifecycle,
    };
    use serde_json::json;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn empty_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("temp workspace should be created");
            Self { path }
        }

        fn as_string(&self) -> String {
            self.path.to_string_lossy().to_string()
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn worktree_context_keeps_project_scope_for_session_operations() {
        let worktree_path = PathBuf::from("/worktrees/wt-1");
        let project_path = PathBuf::from("/repo");
        let execution_target = SessionExecutionTarget {
            kind: SessionExecutionTargetKind::ManagedWorktree,
            worktree_id: Some("wt-1".to_string()),
            root_path: "/worktrees/wt-1".to_string(),
            base_ref: Some("HEAD".to_string()),
            base_commit: Some("0123456789abcdef".to_string()),
            branch: None,
            lifecycle: Some(WorktreeLifecycle::Managed),
        };
        let binding = WorkspaceBinding::new(None, worktree_path.clone())
            .with_project_root_path(project_path.clone())
            .with_execution_target(Some(execution_target.clone()));

        let target = SessionControlTool::workspace_target_from_context(&binding);

        assert_eq!(PathBuf::from(target.display_workspace), worktree_path);
        assert_eq!(PathBuf::from(target.project_workspace), project_path);
        assert_eq!(target.execution_target, Some(execution_target));
    }

    #[test]
    fn worktree_rename_uses_project_scope_for_persistence() {
        let target = SessionControlWorkspaceTarget {
            display_workspace: "/worktrees/wt-1".to_string(),
            project_workspace: "/repo".to_string(),
            execution_target: None,
            workspace_id: None,
            remote_connection_id: None,
            remote_ssh_host: None,
        };

        let request = SessionControlTool::rename_request(&target, "session-1", "Renamed");

        assert_eq!(request.workspace_path, "/repo");
        assert_eq!(request.session_id, "session-1");
        assert_eq!(request.session_name, "Renamed");
    }

    #[tokio::test]
    async fn validate_cancel_requires_session_id() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "cancel",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session_id is required for cancel")
        );
    }

    // ---------------------------------------------------------------------
    // SessionHistory read authorization gate
    // (resolve_session_read_authorization) — attacker matrix:
    // unrelated reject / owner bypass / created_by allow / ancestor->descendant
    // allow / descendant->ancestor allow / sibling reject / cross-workspace
    // reject / missing-metadata reject / fail-closed on unknown sessions.
    // ---------------------------------------------------------------------

    fn read_authz_session_manager()
    -> std::sync::Arc<crate::agentic::session::session_manager::SessionManager> {
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::session_manager::{SessionManager, SessionManagerConfig};
        use crate::agentic::session::{PromptCachePolicy, SessionContextStore};
        use crate::infrastructure::app_paths::path_manager::PathManager;
        use std::sync::Arc;
        let user_root =
            std::env::temp_dir().join(format!("bitfun-read-authz-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&user_root).expect("test user root");
        let path_manager = PathManager::with_user_root_for_tests(user_root);
        let persistence =
            PersistenceManager::new(Arc::new(path_manager)).expect("persistence manager");
        Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(persistence),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                // Persistence stays enabled so the read-authorization gate can
                // exercise its persisted-metadata paths (created_by, parent
                // chain) through the same store used in production.
                enable_persistence: true,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ))
    }

    #[tokio::test]
    async fn read_authz_rejects_unrelated_caller_without_metadata() {
        // Attacker matrix A: not the owner, no created_by, no tree relation ->
        // reject. The caller session exists but is not top-level (created_by is
        // set), so only the creator/ancestor paths could authorize and both
        // are absent.
        let session_manager = read_authz_session_manager();
        let workspace = TestTempDir::new("bitfun-read-authz-unrelated");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        let mut caller = crate::agentic::core::SessionConfig::default();
        caller.workspace_path = Some(workspace_string.clone());
        session_manager
            .create_session_with_id_and_creator(
                Some("caller-1".to_string()),
                "Caller".to_string(),
                "agentic".to_string(),
                caller,
                Some(session_control_creator_marker("another-root")),
            )
            .await
            .expect("create caller session");
        let error = resolve_session_read_authorization(
            &session_manager,
            "caller-1",
            workspace_path,
            "target-1",
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect_err("unrelated caller without metadata must be rejected");
        assert!(
            error
                .to_string()
                .contains("not authorized to export history of"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn read_authz_created_by_match_allows_caller() {
        // Attacker matrix C: created_by == session-<caller> -> allow.
        let session_manager = read_authz_session_manager();
        let workspace = TestTempDir::new("bitfun-read-authz-created-by");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        let target_id = "target-1";
        let metadata = crate::service::session::SessionMetadata::new(
            target_id.to_string(),
            "target".to_string(),
            "agentic".to_string(),
            "auto".to_string(),
        );
        let mut created_metadata = metadata.clone();
        created_metadata.created_by = Some(session_control_creator_marker("caller-1"));
        session_manager
            .save_session_metadata(workspace_path, &created_metadata)
            .await
            .expect("save metadata");

        resolve_session_read_authorization(
            &session_manager,
            "caller-1",
            workspace_path,
            target_id,
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect("creator should be authorized to read");
    }

    #[tokio::test]
    async fn read_authz_ancestor_allows_caller_to_read_descendant() {
        // Attacker matrix D: ancestor may export the descendant (persisted
        // parent chain relationship).
        let session_manager = read_authz_session_manager();
        let workspace = TestTempDir::new("bitfun-read-authz-ancestor");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        register_persisted_parent(&session_manager, workspace_path, "caller-1", "child-1")
            .await;

        resolve_session_read_authorization(
            &session_manager,
            "caller-1",
            workspace_path,
            "child-1",
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect("ancestor should be authorized to read descendant");
    }

    #[tokio::test]
    async fn read_authz_descendant_allows_caller_to_read_ancestor() {
        // Attacker matrix E: descendant may export the ancestor (read is
        // bidirectional inside one session tree).
        let session_manager = read_authz_session_manager();
        let workspace = TestTempDir::new("bitfun-read-authz-descendant");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        register_persisted_parent(&session_manager, workspace_path, "root-1", "caller-1")
            .await;

        resolve_session_read_authorization(
            &session_manager,
            "caller-1",
            workspace_path,
            "root-1",
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect("descendant should be authorized to read ancestor");
    }

    #[tokio::test]
    async fn read_authz_rejects_sibling_without_creator_link() {
        // Attacker matrix F: siblings under one parent (no ancestor/descendant
        // relation, not owner/creator) -> reject.
        let session_manager = read_authz_session_manager();
        let workspace = TestTempDir::new("bitfun-read-authz-sibling");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        register_persisted_parent(&session_manager, workspace_path, "root-1", "caller-1")
            .await;
        register_persisted_parent(&session_manager, workspace_path, "root-1", "target-1")
            .await;

        let error = resolve_session_read_authorization(
            &session_manager,
            "caller-1",
            workspace_path,
            "target-1",
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect_err("sibling sessions must not read each other");
        assert!(
            error
                .to_string()
                .contains("not authorized to export history of"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn read_authz_rejects_cross_workspace() {
        // Attacker matrix G: caller and target live in different workspaces ->
        // always reject. Cross-workspace export is the core isolation
        // boundary.
        let session_manager = read_authz_session_manager();
        let caller_ws = TestTempDir::new("bitfun-read-authz-caller-ws");
        let target_ws = TestTempDir::new("bitfun-read-authz-target-ws");

        let error = resolve_session_read_authorization(
            &session_manager,
            "read-authz-cross-ws",
            std::path::Path::new(&caller_ws.as_string()),
            "target-1",
            std::path::Path::new(&target_ws.as_string()),
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect_err("cross-workspace export must be rejected");
        assert!(
            error
                .to_string()
                .contains("belongs to a different workspace"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn read_authz_owner_bypass_allows_any_target_in_workspace() {
        // Owner bypass: a top-level caller (created_by = None) may export any
        // transcript within its own workspace.
        let session_manager = read_authz_session_manager();
        let workspace = TestTempDir::new("bitfun-read-authz-owner");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        let mut caller = crate::agentic::core::SessionConfig::default();
        caller.workspace_path = Some(workspace_string.clone());
        session_manager
            .create_session_with_id(
                Some("caller-1".to_string()),
                "Caller".to_string(),
                "agentic".to_string(),
                caller,
            )
            .await
            .expect("create caller session");

        resolve_session_read_authorization(
            &session_manager,
            "caller-1",
            workspace_path,
            "any-target-1",
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect("owner should be authorized to read any target in its workspace");
    }

    #[tokio::test]
    async fn read_authz_owner_bypass_disabled_keeps_gate_closed() {
        // With allow_owner_bypass = false the owner exemption is not applied
        // and the gate stays closed for an unrelated caller.
        let session_manager = read_authz_session_manager();
        let workspace = TestTempDir::new("bitfun-read-authz-no-bypass");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        let mut caller = crate::agentic::core::SessionConfig::default();
        caller.workspace_path = Some(workspace_string.clone());
        session_manager
            .create_session_with_id(
                Some("caller-1".to_string()),
                "Caller".to_string(),
                "agentic".to_string(),
                caller,
            )
            .await
            .expect("create caller session");

        let error = resolve_session_read_authorization(
            &session_manager,
            "caller-1",
            workspace_path,
            "target-1",
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions {
                allow_owner_bypass: false,
            },
        )
        .await
        .expect_err("owner bypass disabled must keep the gate closed");
        assert!(
            error
                .to_string()
                .contains("not authorized to export history of"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn read_authz_missing_metadata_fails_closed() {
        // Fail-closed: without metadata (no created_by, no parent chain) the
        // unrelated caller is rejected — an empty chain cannot be abused to
        // bypass the gate.
        let session_manager = read_authz_session_manager();
        let workspace = TestTempDir::new("bitfun-read-authz-fail-closed");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        let mut caller = crate::agentic::core::SessionConfig::default();
        caller.workspace_path = Some(workspace_string.clone());
        session_manager
            .create_session_with_id_and_creator(
                Some("caller-1".to_string()),
                "Caller".to_string(),
                "agentic".to_string(),
                caller,
                Some(session_control_creator_marker("someone-else")),
            )
            .await
            .expect("create caller session");

        let error = resolve_session_read_authorization(
            &session_manager,
            "caller-1",
            workspace_path,
            "target-1",
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect_err("missing target metadata must fail closed");
        assert!(
            error
                .to_string()
                .contains("not authorized to export history of"),
            "{error}"
        );
    }

    /// Persist a parent->child relationship so the ancestor chain walk can
    /// resolve it from session metadata.
    async fn register_persisted_parent(
        session_manager: &std::sync::Arc<
            crate::agentic::session::session_manager::SessionManager,
        >,
        workspace_path: &std::path::Path,
        parent_id: &str,
        child_id: &str,
    ) {
        let metadata = crate::service::session::SessionMetadata::new(
            child_id.to_string(),
            child_id.to_string(),
            "agentic".to_string(),
            "auto".to_string(),
        );
        let mut child_metadata = metadata;
        child_metadata.relationship = Some(crate::service::session::SessionRelationship {
            parent_session_id: Some(parent_id.to_string()),
            ..Default::default()
        });
        session_manager
            .save_session_metadata(workspace_path, &child_metadata)
            .await
            .expect("save child metadata");
    }

    // ---------------------------------------------------------------------
    // Cross-workspace list authorization (SessionControl list)
    // Owner (top-level session, created_by = None) may list another
    // workspace; delegated sessions may only list their own workspace.
    // ---------------------------------------------------------------------

    fn normalized(value: &str) -> String {
        normalize_path(value)
    }

    fn list_gate_current_workspace_matches(
        current: Option<&str>,
        explicit: &str,
    ) -> bool {
        current.is_some_and(|current| normalized(current) == normalized(explicit))
    }

    fn list_gate_rejected(
        caller_created_by: Option<&str>,
        current_workspace: Option<&str>,
        explicit_workspace: &str,
    ) -> bool {
        let is_cross_workspace = !list_gate_current_workspace_matches(
            current_workspace,
            explicit_workspace,
        );
        let caller_is_owner = caller_created_by.is_none();
        is_cross_workspace && !caller_is_owner
    }

    #[test]
    fn list_gate_allows_own_workspace_listing() {
        // A delegated session listing its own workspace passes the gate.
        assert!(list_gate_current_workspace_matches(Some("/repo"), "/repo/"));
        assert!(!list_gate_rejected(
            Some(session_control_creator_marker("root-1").as_str()),
            Some("/repo"),
            "/repo"
        ));
    }

    #[test]
    fn list_gate_rejects_delegated_cross_workspace_listing() {
        // Attacker matrix: a delegated session (created_by set) listing a
        // workspace it does not belong to is rejected.
        assert!(list_gate_rejected(
            Some(session_control_creator_marker("root-1").as_str()),
            Some("/other-workspace"),
            "/repo"
        ));
        // No workspace binding at all also counts as cross-workspace.
        assert!(list_gate_rejected(
            Some(session_control_creator_marker("root-1").as_str()),
            None,
            "/repo"
        ));
    }

    #[test]
    fn list_gate_allows_owner_cross_workspace_listing() {
        // Owner semantics: a top-level session (created_by = None) may list
        // any workspace.
        assert!(!list_gate_rejected(None, Some("/other-workspace"), "/repo"));
    }

    #[tokio::test]
    async fn validate_cancel_rejects_session_name() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "cancel",
                    "session_id": "worker_1",
                    "session_name": "should-not-be-here",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session_name is only allowed for create")
        );
    }

    #[tokio::test]
    async fn validate_cancel_allows_missing_workspace() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "cancel",
                    "session_id": "worker_1",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_cancel_ignores_workspace_when_provided() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "cancel",
                    "session_id": "worker_1",
                    "workspace": "not-an-absolute-path",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_list_rejects_session_id() {
        let tool = SessionControlTool::new();
        let workspace = TestTempDir::new("bitfun-session-control-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "action": "list",
                    "workspace": workspace.as_string(),
                    "session_id": "worker_1",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session_id is not allowed for list")
        );
    }

    #[tokio::test]
    async fn validate_list_requires_workspace() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "list",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("workspace is required for list")
        );
    }

    #[test]
    fn render_message_for_cancel_is_specific() {
        let tool = SessionControlTool::new();
        let message = tool.render_tool_use_message(
            &json!({
                "action": "cancel",
                "workspace": "/repo",
                "session_id": "worker_1",
            }),
            &ToolRenderOptions { verbose: false },
        );

        assert_eq!(message, "Cancel active turn for session worker_1");
    }

    #[tokio::test]
    async fn validate_rename_requires_session_name() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "rename",
                    "session_id": "worker_1",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session_name is required for rename")
        );
    }

    #[tokio::test]
    async fn validate_rename_requires_session_id() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "rename",
                    "session_name": "new-title",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session_id is required for rename")
        );
    }

    #[tokio::test]
    async fn validate_rename_accepts_session_id_and_name() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "rename",
                    "session_id": "worker_1",
                    "session_name": "new-title",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }
}
