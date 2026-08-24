use super::session_control_tool::{
    create_worktree_for_session, ensure_worktree_not_remote, SessionWorktreeCreateResult,
    WorktreeSessionOptions,
};
use super::util::normalize_path;
use crate::agentic::coordination::{
    get_global_coordinator, get_global_scheduler, DialogSubmissionPolicy, DialogTriggerSource,
};
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::workspace_paths::posix_style_path_is_absolute;
use crate::service::workspace::get_global_workspace_service;
use crate::service::worktree::WorktreeService;
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_agent_runtime::sdk::AgentRuntime;
use bitfun_core_types::SessionExecutionTarget;
use bitfun_runtime_ports::{
    AgentDialogPrependedReminder, AgentDialogTurnRequest, AgentSessionCreateRequest,
    AgentSessionListRequest, AgentSessionReplyRoute, AgentSessionSummary,
    AgentSessionWorkspaceBinding, AgentSessionWorkspaceRequest,
};
use log::warn;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

/// SessionMessage tool - send a message to another session via the dialog scheduler
pub struct SessionMessageTool;

#[derive(Debug, Clone)]
struct SessionMessageWorkspaceTarget {
    workspace_path: String,
    project_workspace_path: String,
    execution_target: Option<SessionExecutionTarget>,
    workspace_id: Option<String>,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
}

impl Default for SessionMessageTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionMessageTool {
    pub fn new() -> Self {
        Self
    }

    fn validate_session_id(session_id: &str) -> Result<(), String> {
        bitfun_core_types::validate_session_id(session_id)
    }

    fn forwarded_user_input_metadata(context: &ToolUseContext) -> serde_json::Map<String, Value> {
        use bitfun_agent_runtime::user_questions::USER_INPUT_AVAILABLE_CONTEXT_KEY;

        let mut metadata = serde_json::Map::new();
        if let Some(value @ (Value::Bool(_) | Value::String(_))) =
            context.custom_data.get(USER_INPUT_AVAILABLE_CONTEXT_KEY)
        {
            let is_boolean_fact = matches!(value, Value::Bool(_))
                || matches!(value, Value::String(text) if matches!(text.as_str(), "true" | "false"));
            if is_boolean_fact {
                metadata.insert(USER_INPUT_AVAILABLE_CONTEXT_KEY.to_string(), value.clone());
            }
        }
        metadata
    }

    fn resolve_workspace(&self, workspace: &str, context: &ToolUseContext) -> BitFunResult<String> {
        let workspace = workspace.trim();
        if workspace.is_empty() {
            return Err(BitFunError::tool(
                "workspace is required and cannot be empty".to_string(),
            ));
        }

        if context.is_remote() {
            if !posix_style_path_is_absolute(workspace) {
                return Err(BitFunError::tool(
                    "workspace must be an absolute POSIX path on the remote host".to_string(),
                ));
            }
            return context.resolve_workspace_tool_path(workspace);
        }

        let path = Path::new(workspace);
        if !path.is_absolute() {
            return Err(BitFunError::tool(
                "workspace must be an absolute path".to_string(),
            ));
        }

        let resolved = normalize_path(workspace);
        let path = Path::new(&resolved);
        if !path.exists() {
            return Err(BitFunError::tool(format!(
                "Workspace does not exist: {}",
                resolved
            )));
        }
        if !path.is_dir() {
            return Err(BitFunError::tool(format!(
                "Workspace is not a directory: {}",
                resolved
            )));
        }
        Ok(resolved)
    }

    fn validate_workspace_shape(
        &self,
        workspace: &str,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let workspace = workspace.trim();
        if workspace.is_empty() {
            return ValidationResult {
                result: false,
                message: Some("workspace is required and cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        match context {
            Some(context) => {
                let ws_ok = if context.is_remote() {
                    posix_style_path_is_absolute(workspace)
                } else {
                    Path::new(workspace).is_absolute()
                };
                if !ws_ok {
                    return ValidationResult {
                        result: false,
                        message: Some("workspace must be an absolute path".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }
            }
            None => {
                if !Path::new(workspace).is_absolute() && !posix_style_path_is_absolute(workspace) {
                    return ValidationResult {
                        result: false,
                        message: Some("workspace must be an absolute path".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }
            }
        }

        ValidationResult::default()
    }

    fn sender_session_id<'a>(&self, context: &'a ToolUseContext) -> BitFunResult<&'a str> {
        context.session_id.as_deref().ok_or_else(|| {
            BitFunError::tool("SessionMessage requires a source session".to_string())
        })
    }

    fn sender_workspace(&self, context: &ToolUseContext) -> BitFunResult<String> {
        context
            .workspace_root()
            .map(|path| path.to_string_lossy().to_string())
            .ok_or_else(|| {
                BitFunError::tool("SessionMessage requires a source workspace".to_string())
            })
    }

    fn creator_session_marker(&self, context: &ToolUseContext) -> BitFunResult<String> {
        let creator_session_id = context.session_id.as_ref().ok_or_else(|| {
            BitFunError::tool("SessionMessage requires a source session".to_string())
        })?;
        Ok(format!("session-{}", creator_session_id))
    }

    fn workspace_target_from_context(
        &self,
        workspace_path: String,
        context: &ToolUseContext,
    ) -> SessionMessageWorkspaceTarget {
        let binding = context.workspace.as_ref();
        let inherits_current_target = binding.is_some_and(|binding| {
            normalize_path(&binding.root_path_string()) == normalize_path(&workspace_path)
        });
        let remote_connection_id =
            binding.and_then(|workspace| workspace.connection_id().map(ToOwned::to_owned));
        let remote_ssh_host = binding
            .filter(|workspace| workspace.is_remote())
            .map(|workspace| workspace.session_identity.hostname.clone())
            .filter(|value| !value.trim().is_empty());
        let project_workspace_path = if inherits_current_target {
            binding
                .map(|workspace| normalize_path(&workspace.project_root_path_string()))
                .unwrap_or_else(|| workspace_path.clone())
        } else {
            workspace_path.clone()
        };
        SessionMessageWorkspaceTarget {
            workspace_path,
            project_workspace_path,
            execution_target: binding
                .filter(|_| inherits_current_target)
                .and_then(|workspace| workspace.execution_target.clone()),
            workspace_id: binding
                .filter(|_| inherits_current_target)
                .and_then(|workspace| workspace.workspace_id.clone()),
            remote_connection_id,
            remote_ssh_host,
        }
    }

    fn workspace_target_from_binding(
        &self,
        binding: AgentSessionWorkspaceBinding,
    ) -> SessionMessageWorkspaceTarget {
        let project_workspace_path = binding
            .project_workspace_path
            .clone()
            .unwrap_or_else(|| binding.workspace_path.clone());
        SessionMessageWorkspaceTarget {
            workspace_path: binding.workspace_path,
            project_workspace_path,
            execution_target: binding.execution_target,
            workspace_id: binding.workspace_id,
            remote_connection_id: binding.remote_connection_id,
            remote_ssh_host: binding.remote_ssh_host,
        }
    }

    fn same_workspace_identity(
        left: &SessionMessageWorkspaceTarget,
        right: &SessionMessageWorkspaceTarget,
    ) -> bool {
        left.workspace_path == right.workspace_path
            && left.remote_connection_id == right.remote_connection_id
            && left.remote_ssh_host == right.remote_ssh_host
    }

    fn target_agent_type_from_resolution(agent_type: Option<String>) -> Option<String> {
        agent_type.filter(|value| !value.trim().is_empty())
    }

    fn target_agent_type_from_sessions(
        sessions: &[AgentSessionSummary],
        target_session_id: &str,
    ) -> Option<String> {
        sessions
            .iter()
            .find(|session| {
                session.session_id == target_session_id && !session.agent_type.trim().is_empty()
            })
            .map(|session| session.agent_type.clone())
    }

    fn format_forwarded_message(
        &self,
        message: &str,
    ) -> (String, Vec<AgentDialogPrependedReminder>) {
        (
            message.to_string(),
            vec![AgentDialogPrependedReminder {
                kind: "session_message_request".to_string(),
                text: "This request was sent by another agent, not human user. Do not use interactive tools for this request. In particular, do not call AskUserQuestion."
                    .to_string(),
            }],
        )
    }

    /// Resolve the source-session identity and the shared delivery runtime once
    /// for the whole tool call. A single-target dispatch uses it once; a batch
    /// dispatch reuses it across every item so coordinator/scheduler are not
    /// re-resolved per item.
    async fn build_dispatch_shared(
        &self,
        context: &ToolUseContext,
    ) -> BitFunResult<DispatchShared> {
        let source_session_id = self.sender_session_id(context)?.to_string();
        let source_workspace = self.sender_workspace(context)?;
        let source_remote_connection_id = context
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.connection_id().map(ToOwned::to_owned));
        let source_remote_ssh_host = context
            .workspace
            .as_ref()
            .filter(|workspace| workspace.is_remote())
            .map(|workspace| workspace.session_identity.hostname.clone())
            .filter(|value| !value.trim().is_empty());

        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
        let scheduler = get_global_scheduler()
            .ok_or_else(|| BitFunError::tool("scheduler not initialized".to_string()))?;
        let runtime = CoreServiceAgentRuntime::agent_runtime_with_dialog_turns(
            coordinator.clone(),
            scheduler,
        )
        .map_err(BitFunError::tool)?;

        Ok(DispatchShared {
            source_session_id,
            source_workspace,
            source_remote_connection_id,
            source_remote_ssh_host,
            runtime,
        })
    }

    /// Perform one create+send (or send-to-existing) dispatch and return the
    /// structured delivery outcome. This is the single-target path extracted
    /// from the original inline `call_impl` so a batch dispatch can loop over
    /// it without re-implementing create+send.
    async fn dispatch_single(
        &self,
        params: SessionMessageInput,
        shared: &DispatchShared,
        context: &ToolUseContext,
    ) -> BitFunResult<DispatchOutcome> {
        let mut created_worktree: Option<SessionWorktreeCreateResult> = None;
        let (target_session_id, target_agent_type, created_session_id, workspace_target) =
            if let Some(target_session_id) = params.session_id.clone() {
                if shared.source_session_id == target_session_id {
                    return Err(BitFunError::tool(
                        "SessionMessage cannot send a message to the same session".to_string(),
                    ));
                }

                let workspace_target = shared
                    .runtime
                    .resolve_session_workspace_binding(AgentSessionWorkspaceRequest {
                        session_id: target_session_id.clone(),
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;
                let workspace_target = workspace_target.ok_or_else(|| {
                    BitFunError::NotFound(format!(
                        "Workspace for session '{}' could not be resolved",
                        target_session_id
                    ))
                })?;
                let workspace_target = self.workspace_target_from_binding(workspace_target);

                if let Some(workspace) = params.workspace.as_deref() {
                    let requested_workspace = self.resolve_workspace(workspace, context)?;
                    let requested_target =
                        self.workspace_target_from_context(requested_workspace.clone(), context);
                    if !Self::same_workspace_identity(&requested_target, &workspace_target) {
                        return Err(BitFunError::NotFound(format!(
                            "Session '{}' not found in workspace '{}'",
                            target_session_id, requested_target.workspace_path
                        )));
                    }
                }

                let visible_sessions = shared
                    .runtime
                    .list_sessions(AgentSessionListRequest {
                        workspace_path: workspace_target.project_workspace_path.clone(),
                        remote_connection_id: workspace_target.remote_connection_id.clone(),
                        remote_ssh_host: workspace_target.remote_ssh_host.clone(),
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;
                let listed_agent_type =
                    Self::target_agent_type_from_sessions(&visible_sessions, &target_session_id);
                let resolved_agent_type = if listed_agent_type.is_none() {
                    Self::target_agent_type_from_resolution(
                        shared
                            .runtime
                            .resolve_session_agent_type(&target_session_id)
                            .await
                            .map_err(|error| {
                                BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(
                                    error,
                                ))
                            })?,
                    )
                } else {
                    None
                };
                let target_agent_type =
                    listed_agent_type.or(resolved_agent_type).ok_or_else(|| {
                        BitFunError::NotFound(format!("Session '{}' not found", target_session_id))
                    })?;

                (target_session_id, target_agent_type, None, workspace_target)
            } else {
                let workspace = self.resolve_workspace(
                    params.workspace.as_deref().ok_or_else(|| {
                        BitFunError::tool(
                            "workspace is required when session_id is omitted".to_string(),
                        )
                    })?,
                    context,
                )?;
                let workspace_target = self.workspace_target_from_context(workspace, context);
                let session_name = params
                    .session_name
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "session_name is required when session_id is omitted".to_string(),
                        )
                    })?;
                let agent_type = params
                    .agent_type
                    .as_ref()
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "agent_type is required when session_id is omitted".to_string(),
                        )
                    })?
                    .as_str()
                    .to_string();
                let created_by = self.creator_session_marker(context)?;
                let mut metadata = serde_json::Map::new();
                metadata.insert("createdBy".to_string(), json!(created_by));

                // G2 (shared core): optional managed Git worktree. When the
                // worktree param is present, create the worktree first (shared
                // core in session_control_tool) and bind the new session to it.
                // Failure = session not created + worktree rolled back.
                if let Some(worktree) = params.worktree.as_ref() {
                    ensure_worktree_not_remote(context)?;
                    let request_id = context
                        .tool_call_id
                        .as_deref()
                        .map(|tool_call_id| format!("session-message:{tool_call_id}:worktree"))
                        .unwrap_or_else(|| {
                            format!("session-message:{}:worktree", uuid::Uuid::new_v4())
                        });
                    created_worktree = Some(
                        create_worktree_for_session(
                            &request_id,
                            &workspace_target.project_workspace_path,
                            worktree,
                            context,
                        )
                        .await?,
                    );
                }

                let session = match shared
                    .runtime
                    .create_session(AgentSessionCreateRequest {
                        session_name,
                        agent_type: agent_type.clone(),
                        workspace_path: Some(
                            created_worktree
                                .as_ref()
                                .map(|wt| wt.execution_target.root_path.clone())
                                .unwrap_or_else(|| workspace_target.workspace_path.clone()),
                        ),
                        project_workspace_path: Some(
                            created_worktree
                                .as_ref()
                                .map(|wt| wt.project_workspace_path.clone())
                                .unwrap_or_else(|| workspace_target.project_workspace_path.clone()),
                        ),
                        execution_target: created_worktree
                            .as_ref()
                            .map(|wt| wt.execution_target.clone())
                            .or_else(|| workspace_target.execution_target.clone()),
                        workspace_id: created_worktree
                            .as_ref()
                            .and_then(|wt| wt.tracked_workspace_id.clone())
                            .or_else(|| workspace_target.workspace_id.clone()),
                        remote_connection_id: workspace_target.remote_connection_id.clone(),
                        remote_ssh_host: workspace_target.remote_ssh_host.clone(),
                        model_id: None,
                        metadata,
                    })
                    .await
                {
                    Ok(session) => session,
                    Err(create_error) => {
                        // Session creation failed -> roll back any freshly-created worktree.
                        if let Some(worktree) = created_worktree.as_ref() {
                            if worktree.created {
                                if let Some(workspace_service) = get_global_workspace_service() {
                                    if let Some(workspace_id) =
                                        worktree.tracked_workspace_id.as_deref()
                                    {
                                        let _ =
                                            workspace_service.remove_workspace(workspace_id).await;
                                    }
                                }
                                if let Some(worktree_id) =
                                    worktree.execution_target.worktree_id.as_deref()
                                {
                                    let _ = WorktreeService::rollback_created(
                                        &worktree.project_workspace_path,
                                        worktree_id,
                                    )
                                    .await;
                                }
                            }
                        }
                        return Err(BitFunError::tool(
                            CoreServiceAgentRuntime::runtime_error_message(create_error),
                        ));
                    }
                };

                (
                    session.session_id.clone(),
                    session.agent_type.clone(),
                    Some(session.session_id),
                    workspace_target,
                )
            };

        // Explicitly reject an empty message rather than silently defaulting the
        // now-optional top-level field (the single-target path requires one).
        let message = params
            .message
            .clone()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| BitFunError::tool("message cannot be empty".to_string()))?;

        let (forwarded_message, prepended_messages) =
            self.format_forwarded_message(&message);

        shared
            .runtime
            .submit_dialog_turn(AgentDialogTurnRequest {
                session_id: target_session_id.clone(),
                message: forwarded_message,
                original_message: Some(message),
                turn_id: None,
                execution: Default::default(),
                agent_type: target_agent_type.clone(),
                workspace_path: Some(workspace_target.workspace_path.clone()),
                remote_connection_id: workspace_target.remote_connection_id.clone(),
                remote_ssh_host: workspace_target.remote_ssh_host.clone(),
                policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
                reply_route: Some(AgentSessionReplyRoute {
                    source_session_id: shared.source_session_id.clone(),
                    source_workspace_path: shared.source_workspace.clone(),
                    source_remote_connection_id: shared.source_remote_connection_id.clone(),
                    source_remote_ssh_host: shared.source_remote_ssh_host.clone(),
                }),
                prepended_reminders: prepended_messages,
                attachments: Vec::new(),
                metadata: Self::forwarded_user_input_metadata(context),
            })
            .await
            .map_err(|error| {
                BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
            })?;

        let result_text = if let Some(created_session_id) = created_session_id.clone() {
            format!(
                "Created session '{}' and accepted the message in workspace '{}' using agent type '{}'.",
                created_session_id, workspace_target.workspace_path, target_agent_type
            )
        } else {
            format!(
                "Message accepted for session '{}' in workspace '{}' using agent type '{}'.",
                target_session_id, workspace_target.workspace_path, target_agent_type
            )
        };

        Ok(DispatchOutcome {
            target_session_id,
            target_agent_type,
            created_session_id,
            workspace_path: workspace_target.workspace_path,
            result_text,
            created_worktree,
        })
    }

    /// Validate a batch up front: the batch must be non-empty, the top-level
    /// `message` and session fields must be omitted, the shared workspace must
    /// be present (and shape-checked) when any item creates a session, and every
    /// item must satisfy the single-target structural rules. Any structurally
    /// invalid item rejects the whole batch before anything executes.
    async fn validate_batch(
        &self,
        parsed: &SessionMessageInput,
        batch: &[BatchItem],
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if batch.is_empty() {
            return Self::invalid("batch cannot be empty");
        }
        if parsed
            .message
            .as_deref()
            .is_some_and(|message| !message.trim().is_empty())
        {
            return Self::invalid("message cannot be combined with batch");
        }
        if parsed.session_id.is_some()
            || parsed.session_name.is_some()
            || parsed.agent_type.is_some()
        {
            return Self::invalid(
                "session fields must be provided per batch item when batch is used",
            );
        }

        // The shared workspace must be present (and well-formed) when any item
        // creates a new session; when present it is always shape-checked.
        if let Some(workspace) = parsed.workspace.as_deref() {
            let workspace_validation = self.validate_workspace_shape(workspace, context);
            if !workspace_validation.result {
                return workspace_validation;
            }
        } else if batch.iter().any(|item| item.session_id.is_none()) {
            return Self::invalid("workspace is required when a batch item omits session_id");
        }

        let source_session_id = context.and_then(|context| context.session_id.as_deref());
        for (index, item) in batch.iter().enumerate() {
            let field = |name: &str| format!("batch[{index}].{name}");
            if item.message.trim().is_empty() {
                return Self::invalid(format!("{} cannot be empty", field("message")));
            }
            match item.session_id.as_deref() {
                Some(session_id) => {
                    if let Err(message) = Self::validate_session_id(session_id) {
                        return Self::invalid(format!("{}: {message}", field("session_id")));
                    }
                    if item.session_name.is_some() {
                        return Self::invalid(format!(
                            "{} is only allowed when session_id is omitted",
                            field("session_name")
                        ));
                    }
                    if item.agent_type.is_some() {
                        return Self::invalid(format!(
                            "{} override is not allowed when session_id is provided",
                            field("agent_type")
                        ));
                    }
                    if item.worktree.is_some() {
                        return Self::invalid(format!(
                            "{} is only allowed when session_id is omitted",
                            field("worktree")
                        ));
                    }
                    if let Some(source_session_id) = source_session_id {
                        if source_session_id == session_id {
                            return Self::invalid(format!(
                                "{} cannot send a message to the same session",
                                field("session_id")
                            ));
                        }
                    }
                }
                None => {
                    if item
                        .session_name
                        .as_deref()
                        .is_none_or(|value| value.trim().is_empty())
                    {
                        return Self::invalid(format!(
                            "{} is required when session_id is omitted",
                            field("session_name")
                        ));
                    }
                    if item.agent_type.is_none() {
                        return Self::invalid(format!(
                            "{} is required when session_id is omitted",
                            field("agent_type")
                        ));
                    }
                    if let Some(worktree) = item.worktree.as_ref() {
                        if worktree
                            .base_ref
                            .as_deref()
                            .is_some_and(|base_ref| base_ref.trim().is_empty())
                        {
                            return Self::invalid(format!(
                                "{} must not be empty when provided",
                                field("worktree.base_ref")
                            ));
                        }
                        if context.is_some_and(|context| context.is_remote()) {
                            return Self::invalid(format!(
                                "{} is not supported for remote workspaces",
                                field("worktree")
                            ));
                        }
                        if item
                            .agent_type
                            .as_ref()
                            .is_some_and(|agent_type| agent_type.as_str().starts_with("acp__"))
                        {
                            return Self::invalid(format!(
                                "{} is not supported with acp__ agent types",
                                field("worktree")
                            ));
                        }
                    }
                }
            }
        }

        let Some(context) = context else {
            return ValidationResult::default();
        };
        let Some(_source_session_id) = context.session_id.as_deref() else {
            return Self::invalid("SessionMessage requires a source session in tool context");
        };
        ValidationResult::default()
    }

    fn invalid(message: impl Into<String>) -> ValidationResult {
        ValidationResult {
            result: false,
            message: Some(message.into()),
            error_code: Some(400),
            meta: None,
        }
    }

    /// Map a single batch item's dispatch outcome to its result payload. A
    /// failed item is recorded as an `error` result (never rolled back, never
    /// stops later items); a successful item keeps its delivery fields. Kept
    /// pure so the batch dispatch semantics can be verified by unit tests
    /// without a live agent runtime.
    fn batch_item_result(item: &BatchItem, outcome: BitFunResult<DispatchOutcome>) -> Value {
        match outcome {
            Ok(outcome) => json!({
                "status": "success",
                "target_session_id": outcome.target_session_id,
                "target_agent_type": outcome.target_agent_type,
                "target_workspace": outcome.workspace_path,
                "created_session_id": outcome.created_session_id,
                "result": outcome.result_text,
            }),
            Err(error) => {
                warn!(
                    "Batch SessionMessage item failed (successful items are not rolled back): session_name={:?}, session_id={:?}, error={}",
                    item.session_name, item.session_id, error
                );
                json!({
                    "status": "error",
                    "session_name": item.session_name.clone(),
                    "session_id": item.session_id.clone(),
                    "error": error.to_string(),
                })
            }
        }
    }

    /// Materialize one result per batch item from its dispatch outcome. Every
    /// item yields exactly one entry, so a failure never stops later items and
    /// the batch summary reflects the true total. Keeping this a pure step lets
    /// the "per-item independent, no rollback, no stop" semantics be tested
    /// without driving the real create+send runtime.
    fn batch_results(
        items: &[BatchItem],
        outcomes: impl IntoIterator<Item = BitFunResult<DispatchOutcome>>,
    ) -> Vec<Value> {
        items
            .iter()
            .zip(outcomes)
            .map(|(item, outcome)| Self::batch_item_result(item, outcome))
            .collect()
    }

    /// Dispatch every batch item sequentially and independently. Each item is a
    /// standalone single-target dispatch; a failed item is recorded as an error
    /// result and never rolls back already-succeeded items or stops later items.
    async fn call_batch(
        &self,
        params: &SessionMessageInput,
        items: &[BatchItem],
        shared: &DispatchShared,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        // Dispatch each item sequentially and independently, then materialize
        // exactly one result per item so a failure never rolls back earlier
        // successes or stops later items.
        let mut outcomes = Vec::with_capacity(items.len());
        for item in items {
            let item_params = SessionMessageInput {
                workspace: params.workspace.clone(),
                session_id: item.session_id.clone(),
                session_name: item.session_name.clone(),
                message: Some(item.message.clone()),
                agent_type: item.agent_type.clone(),
                worktree: item.worktree.clone(),
                batch: None,
            };
            outcomes.push(self.dispatch_single(item_params, shared, context).await);
        }
        let results = Self::batch_results(items, outcomes);

        let (succeeded, failed, summary) = Self::summarize_batch_results(&results);

        Ok(vec![ToolResult::Result {
            data: json!({
                "success": true,
                "total": results.len(),
                "succeeded": succeeded,
                "failed": failed,
                "results": results,
            }),
            result_for_assistant: Some(summary),
            image_attachments: None,
        }])
    }

    /// Aggregate per-item outcomes into success/failed counts and the summary
    /// text. Successful items are never rolled back; the summary tells the
    /// caller to retry only the failed items using the per-item session ids.
    fn summarize_batch_results(results: &[Value]) -> (usize, usize, String) {
        let succeeded = results
            .iter()
            .filter(|result| result.get("status").and_then(Value::as_str) == Some("success"))
            .count();
        let failed = results.len() - succeeded;
        let mut summary = format!(
            "Batch dispatch of {} message(s): {} succeeded, {} failed. Successful items are not rolled back; retry only the failed items (skip the succeeded session ids below).",
            results.len(),
            succeeded,
            failed
        );
        if failed > 0 {
            summary.push_str(
                " A failed item never rolls back earlier successes, and later items still ran.",
            );
        }
        (succeeded, failed, summary)
    }
}

#[derive(Debug, Clone, Deserialize)]
enum SessionMessageAgentType {
    #[serde(rename = "agentic", alias = "Agentic", alias = "AGENTIC")]
    Agentic,
    #[serde(rename = "Plan", alias = "plan", alias = "PLAN")]
    Plan,
    #[serde(rename = "Cowork", alias = "cowork", alias = "COWORK")]
    Cowork,
    #[serde(
        rename = "DeepResearch",
        alias = "deepresearch",
        alias = "DEEPRESEARCH"
    )]
    DeepResearch,
}

impl SessionMessageAgentType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Agentic => "agentic",
            Self::Plan => "Plan",
            Self::Cowork => "Cowork",
            Self::DeepResearch => "DeepResearch",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SessionMessageInput {
    workspace: Option<String>,
    session_id: Option<String>,
    session_name: Option<String>,
    /// Top-level message for single-target dispatch. Mutually exclusive with
    /// `batch`: when a batch is present this field must be omitted or empty.
    #[serde(default)]
    message: Option<String>,
    agent_type: Option<SessionMessageAgentType>,
    /// Optional worktree options for create: when present (and session_id is
    /// omitted), a managed worktree is created together with the session via
    /// the G2 shared core (`create_worktree_for_session`) and the session is
    /// bound to it. `None` keeps the legacy behavior (session runs in the
    /// project checkout). Rejected for remote workspaces and for
    /// session_id-based sends.
    #[serde(default)]
    worktree: Option<WorktreeSessionOptions>,
    /// Batch dispatch: perform multiple create+send (or send-to-existing)
    /// operations in one tool call. Every item is validated up front (the whole
    /// batch is rejected when any item is structurally invalid), then each item
    /// executes sequentially and independently: a failed item never rolls back
    /// already-succeeded items and never stops later items. The top-level
    /// session fields (session_id/session_name/agent_type) must stay empty when
    /// a batch is used; the top-level workspace is shared by every item that
    /// creates a new session.
    #[serde(default)]
    batch: Option<Vec<BatchItem>>,
}

/// One create+send (or send-to-existing-session) operation inside a batch
/// dispatch. Fields mirror the top-level SessionMessageInput semantics, except
/// that the workspace is shared from the top level.
#[derive(Debug, Clone, Deserialize)]
struct BatchItem {
    /// Optional target session ID. Omit it to create a new session (requires
    /// session_name and agent_type; the top-level workspace is used).
    session_id: Option<String>,
    /// Display name for a new session. Required when session_id is omitted.
    session_name: Option<String>,
    /// Message to send to the target session.
    message: String,
    /// Agent type for a new session. Required when session_id is omitted.
    agent_type: Option<SessionMessageAgentType>,
    /// Per-item worktree options for a new session (only when session_id is
    /// omitted; rejected for remote workspaces). Passthrough only — the actual
    /// worktree creation is delegated to the G2 shared core
    /// (`create_worktree_for_session`).
    #[serde(default)]
    worktree: Option<WorktreeSessionOptions>,
}

/// Result of one create+send (or send-to-existing) dispatch. Structured so both
/// the single-target path and the batch dispatcher can consume a uniform
/// outcome instead of each re-deriving the delivery fields.
struct DispatchOutcome {
    target_session_id: String,
    target_agent_type: String,
    created_session_id: Option<String>,
    workspace_path: String,
    result_text: String,
    /// Optional managed worktree created alongside a fresh session (G2 shared
    /// core). `None` when no worktree was requested or when sending to an
    /// existing session.
    created_worktree: Option<SessionWorktreeCreateResult>,
}

/// Per-call context shared across a single-tool call. Built once from the tool
/// context so a batch dispatch can reuse the resolver/scheduler and source
/// session identity across every item instead of re-resolving them per item.
struct DispatchShared {
    source_session_id: String,
    source_workspace: String,
    source_remote_connection_id: Option<String>,
    source_remote_ssh_host: Option<String>,
    runtime: AgentRuntime,
}

#[async_trait]
impl Tool for SessionMessageTool {
    fn name(&self) -> &str {
        "SessionMessage"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Asynchronously send a message to another agent session. When the target session finishes, its result is automatically sent back to you as a follow-up message.

Usage:
- Create a new session and send: omit "session_id", and provide "workspace", "session_name", "agent_type", and "message".
- Reusing an existing session: provide "session_id" and "message". You may omit "workspace"; the tool will resolve it from the target session when possible.

Allowed agent types when creating a session:
- "agentic": Coding-focused agent for implementation, debugging, and code changes.
- "Plan": Planning agent for clarifying requirements and producing an implementation plan before coding.
- "Cowork": Collaborative agent for office-style work such as research, documentation, presentations, etc.
- "DeepResearch": Research agent for systematic investigation and evidence-driven reports.
"#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Send a message to another agent session and receive the result asynchronously.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "workspace": {
                    "type": "string",
                    "description": "Required absolute target workspace path when creating a new session. Optional when session_id is provided."
                },
                "session_id": {
                    "type": "string",
                    "description": "Optional target session ID. Omit it to create a new session and send the message there."
                },
                "session_name": {
                    "type": "string",
                    "description": "Required when session_id is omitted. Display name for the new session."
                },
                "message": {
                    "type": "string",
                    "description": "Message to send to the target session. Mutually exclusive with `batch`; when a batch is present this field must be omitted or empty (single-target requires it)."
                },
                "agent_type": {
                    "type": "string",
                    "enum": ["agentic", "Plan", "Cowork", "DeepResearch"],
                    "description": "Required when session_id is omitted. Not allowed when sending to an existing session."
                },
                "worktree": {
                    "type": "object",
                    "description": "Optional worktree options when creating a new session (session_id omitted): creates a managed Git worktree together with the session and binds the session to it (not supported for remote workspaces). Shape: {baseRef?, copyLocalChanges?}.",
                    "properties": {
                        "baseRef": {
                            "type": "string",
                            "description": "Optional Git ref for the new worktree. Defaults to HEAD."
                        },
                        "copyLocalChanges": {
                            "type": "boolean",
                            "description": "Copy staged, unstaged, untracked, and .worktreeinclude-selected ignored files when the selected base equals source HEAD."
                        }
                    },
                    "additionalProperties": false
                },
                "batch": {
                    "type": "array",
                    "description": "Batch dispatch: perform multiple create+send (or send-to-existing) operations in one tool call. Mutually exclusive with the top-level message and session fields; the top-level workspace is shared by items that create a session. All items validate up front; each item then runs independently (a failed item never rolls back succeeded ones). Item shape: {session_id?, session_name?, message, agent_type?, worktree?}.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "session_id": {
                                "type": "string",
                                "description": "Optional target session ID. Omit it to create a new session."
                            },
                            "session_name": {
                                "type": "string",
                                "description": "Required when session_id is omitted. Display name for the new session."
                            },
                            "message": {
                                "type": "string",
                                "description": "Message to send to the target session."
                            },
                            "agent_type": {
                                "type": "string",
                                "enum": ["agentic", "Plan", "Cowork", "DeepResearch"],
                                "description": "Required when session_id is omitted. Agent type for the new session."
                            },
                            "worktree": {
                                "type": "object",
                                "description": "Per-item worktree options for a new session (only when session_id is omitted; not supported for remote workspaces). Shape: {baseRef?, copyLocalChanges?}.",
                                "properties": {
                                    "baseRef": {
                                        "type": "string",
                                        "description": "Optional Git ref for the new worktree. Defaults to HEAD."
                                    },
                                    "copyLocalChanges": {
                                        "type": "boolean",
                                        "description": "Copy staged, unstaged, untracked, and .worktreeinclude-selected ignored files when the selected base equals source HEAD."
                                    }
                                },
                                "additionalProperties": false
                            }
                        },
                        "required": ["message"],
                        "additionalProperties": false
                    }
                }
            },
            "required": [],
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
        let parsed: SessionMessageInput = match serde_json::from_value(input.clone()) {
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

        // Batch mode: the whole batch is validated up front - any structurally
        // invalid item rejects the entire batch before anything executes.
        if let Some(batch) = parsed.batch.as_ref() {
            return self.validate_batch(&parsed, batch, context).await;
        }

        let message = parsed.message.as_deref().unwrap_or_default();
        if message.trim().is_empty() {
            return ValidationResult {
                result: false,
                message: Some("message cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        // G2: worktree is only valid when creating a new session (session_id omitted).
        if parsed.worktree.is_some() && parsed.session_id.is_some() {
            return ValidationResult {
                result: false,
                message: Some("worktree is only allowed when session_id is omitted".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        match parsed.session_id.as_deref() {
            Some(session_id) => {
                if let Err(message) = Self::validate_session_id(session_id) {
                    return ValidationResult {
                        result: false,
                        message: Some(message),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if parsed.session_name.is_some() {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "session_name is only allowed when session_id is omitted".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if parsed.agent_type.is_some() {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "agent_type override is not allowed when session_id is provided"
                                .to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if let Some(workspace) = parsed.workspace.as_deref() {
                    let workspace_validation = self.validate_workspace_shape(workspace, context);
                    if !workspace_validation.result {
                        return workspace_validation;
                    }
                }
            }
            None => {
                if parsed
                    .session_name
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
                {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "session_name is required when session_id is omitted".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if parsed.agent_type.is_none() {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "agent_type is required when session_id is omitted".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                let Some(workspace) = parsed.workspace.as_deref() else {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "workspace is required when session_id is omitted".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                };
                let workspace_validation = self.validate_workspace_shape(workspace, context);
                if !workspace_validation.result {
                    return workspace_validation;
                }

                // G2: worktree input validation for a fresh session.
                if let Some(worktree) = parsed.worktree.as_ref() {
                    if let Some(base_ref) = worktree.base_ref.as_deref() {
                        if base_ref.trim().is_empty() {
                            return ValidationResult {
                                result: false,
                                message: Some(
                                    "worktree.base_ref must not be empty when provided".to_string(),
                                ),
                                error_code: Some(400),
                                meta: None,
                            };
                        }
                    }
                    // Worktrees are not supported with ACP bridge agents (G5 boundary):
                    // an ACP session records an external process, so a local execution
                    // target would be silently ignored / orphaned.
                    if let Some(agent_type) = input.get("agent_type").and_then(Value::as_str) {
                        if agent_type.starts_with("acp__") {
                            return ValidationResult {
                                result: false,
                                message: Some(
                                    "worktree is not supported with acp__ agent types".to_string(),
                                ),
                                error_code: Some(400),
                                meta: None,
                            };
                        }
                    }
                }
            }
        }

        let Some(context) = context else {
            return ValidationResult::default();
        };

        let Some(source_session_id) = context.session_id.as_deref() else {
            return ValidationResult {
                result: false,
                message: Some(
                    "SessionMessage requires a source session in tool context".to_string(),
                ),
                error_code: Some(400),
                meta: None,
            };
        };

        if let Some(target_session_id) = parsed.session_id.as_deref() {
            if source_session_id == target_session_id {
                return ValidationResult {
                    result: false,
                    message: Some(
                        "SessionMessage cannot send a message to the same session".to_string(),
                    ),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        ValidationResult::default()
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let workspace = input
            .get("workspace")
            .and_then(|value| value.as_str())
            .unwrap_or("resolved workspace");
        if let Some(batch) = input.get("batch").and_then(|value| value.as_array()) {
            return format!("Batch dispatch {} message(s) in {}", batch.len(), workspace);
        }
        if let Some(session_id) = input.get("session_id").and_then(|value| value.as_str()) {
            format!("Send message to session {} in {}", session_id, workspace)
        } else {
            let session_name = input
                .get("session_name")
                .and_then(|value| value.as_str())
                .unwrap_or("new session");
            format!(
                "Create session {} in {} and send message",
                session_name, workspace
            )
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: SessionMessageInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;
        let shared = self.build_dispatch_shared(context).await?;
        if let Some(batch) = params.batch.as_ref() {
            return self.call_batch(&params, batch, &shared, context).await;
        }
        let outcome = self.dispatch_single(params, &shared, context).await?;
        let mut data = json!({
            "success": true,
            "target_workspace": outcome.workspace_path.clone(),
            "target_session_id": outcome.target_session_id.clone(),
            "target_agent_type": outcome.target_agent_type.clone(),
            "created_session_id": outcome.created_session_id.clone(),
        });
        if let Some(worktree) = outcome.created_worktree.as_ref() {
            data["worktree"] = json!({
                "worktree_id": worktree.execution_target.worktree_id.clone(),
                "path": worktree.execution_target.root_path.clone(),
                "branch": worktree.branch_name.clone(),
            });
        }
        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(outcome.result_text),
            image_attachments: None,
        }])
    }
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

    fn session_context(session_id: &str) -> ToolUseContext {
        ToolUseContext {
            session_id: Some(session_id.to_string()),
            ..empty_context()
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

    fn workspace_target(
        workspace_path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
    ) -> SessionMessageWorkspaceTarget {
        SessionMessageWorkspaceTarget {
            workspace_path: workspace_path.to_string(),
            project_workspace_path: workspace_path.to_string(),
            execution_target: None,
            workspace_id: None,
            remote_connection_id: remote_connection_id.map(ToOwned::to_owned),
            remote_ssh_host: remote_ssh_host.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn creating_in_current_worktree_inherits_project_scope_and_target() {
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
        let binding = WorkspaceBinding::new(None, worktree_path)
            .with_project_root_path(project_path.clone())
            .with_execution_target(Some(execution_target.clone()));
        let mut context = empty_context();
        context.workspace = Some(binding);

        let target = SessionMessageTool::new()
            .workspace_target_from_context("/worktrees/wt-1".to_string(), &context);

        assert_eq!(target.workspace_path, "/worktrees/wt-1");
        assert_eq!(PathBuf::from(target.project_workspace_path), project_path);
        assert_eq!(target.execution_target, Some(execution_target));
    }

    #[test]
    fn workspace_identity_matches_full_remote_tuple() {
        let left = workspace_target("/root/repo", Some("conn-1"), Some("host-a"));
        let right = workspace_target("/root/repo", Some("conn-1"), Some("host-a"));

        assert!(SessionMessageTool::same_workspace_identity(&left, &right));
    }

    #[test]
    fn workspace_identity_rejects_remote_local_parity_mismatch() {
        let requested = workspace_target("/root/repo", None, None);
        let target = workspace_target("/root/repo", Some("conn-1"), Some("host-a"));

        assert!(!SessionMessageTool::same_workspace_identity(
            &requested, &target
        ));
    }

    #[test]
    fn workspace_identity_rejects_remote_host_mismatch() {
        let requested = workspace_target("/root/repo", Some("conn-1"), Some("host-a"));
        let target = workspace_target("/root/repo", Some("conn-1"), Some("host-b"));

        assert!(!SessionMessageTool::same_workspace_identity(
            &requested, &target
        ));
    }

    #[test]
    fn target_agent_type_rejects_empty_agent_type_resolution() {
        assert_eq!(
            SessionMessageTool::target_agent_type_from_resolution(Some(" ".to_string())),
            None
        );
    }

    #[test]
    fn session_message_forwards_noninteractive_user_input_fact() {
        use bitfun_agent_runtime::user_questions::USER_INPUT_AVAILABLE_CONTEXT_KEY;

        let mut context = empty_context();
        context.custom_data.insert(
            USER_INPUT_AVAILABLE_CONTEXT_KEY.to_string(),
            Value::Bool(false),
        );

        let metadata = SessionMessageTool::forwarded_user_input_metadata(&context);

        assert_eq!(
            metadata.get(USER_INPUT_AVAILABLE_CONTEXT_KEY),
            Some(&Value::Bool(false))
        );
    }

    #[test]
    fn target_agent_type_uses_resolved_agent_type() {
        assert_eq!(
            SessionMessageTool::target_agent_type_from_resolution(Some("agentic".to_string()))
                .as_deref(),
            Some("agentic")
        );
    }

    #[test]
    fn target_agent_type_uses_matching_session_agent_type() {
        let sessions = vec![AgentSessionSummary {
            session_id: "worker_1".to_string(),
            session_name: "Worker".to_string(),
            agent_type: "agentic".to_string(),
            model_id: None,
            reasoning_preset: None,
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            turn_count: 0,
            created_at_ms: 1,
            last_active_at_ms: 2,
        }];

        assert_eq!(
            SessionMessageTool::target_agent_type_from_sessions(&sessions, "worker_1").as_deref(),
            Some("agentic")
        );
    }

    #[test]
    fn target_agent_type_rejects_empty_session_agent_type() {
        let sessions = vec![AgentSessionSummary {
            session_id: "worker_1".to_string(),
            session_name: "Worker".to_string(),
            agent_type: " ".to_string(),
            model_id: None,
            reasoning_preset: None,
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            turn_count: 0,
            created_at_ms: 1,
            last_active_at_ms: 2,
        }];

        assert_eq!(
            SessionMessageTool::target_agent_type_from_sessions(&sessions, "worker_1"),
            None
        );
    }

    #[tokio::test]
    async fn validate_existing_session_rejects_agent_type_override() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "session_id": "worker_1",
                    "message": "hello",
                    "agent_type": "DeepResearch",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("agent_type override is not allowed when session_id is provided")
        );
    }

    #[tokio::test]
    async fn validate_new_session_requires_session_name() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "message": "hello",
                    "agent_type": "agentic",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session_name is required when session_id is omitted")
        );
    }

    #[tokio::test]
    async fn validate_new_session_requires_agent_type() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "message": "hello",
                    "session_name": "Worker Session",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("agent_type is required when session_id is omitted")
        );
    }

    #[tokio::test]
    async fn validate_new_session_accepts_create_and_send_shape() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "message": "hello",
                    "session_name": "Worker Session",
                    "agent_type": "DeepResearch",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_existing_session_allows_missing_workspace() {
        let tool = SessionMessageTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "session_id": "worker_1",
                    "message": "hello",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_new_session_requires_workspace() {
        let tool = SessionMessageTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "message": "hello",
                    "session_name": "Worker Session",
                    "agent_type": "agentic",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("workspace is required when session_id is omitted")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_empty_batch() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch cannot be empty")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_top_level_message() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "message": "hello",
                    "batch": [
                        {
                            "session_name": "Worker Session",
                            "message": "hi",
                            "agent_type": "agentic",
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("message cannot be combined with batch")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_top_level_session_fields() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "session_id": "worker_1",
                    "batch": [
                        {
                            "session_name": "Worker Session",
                            "message": "hi",
                            "agent_type": "agentic",
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session fields must be provided per batch item when batch is used")
        );
    }

    #[tokio::test]
    async fn validate_batch_requires_workspace_for_create_item() {
        let tool = SessionMessageTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "batch": [
                        {
                            "session_name": "Worker Session",
                            "message": "hi",
                            "agent_type": "agentic",
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("workspace is required when a batch item omits session_id")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_item_empty_message() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_name": "Worker Session",
                            "message": "",
                            "agent_type": "agentic",
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].message cannot be empty")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_item_missing_session_name() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "message": "hi",
                            "agent_type": "agentic",
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].session_name is required when session_id is omitted")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_item_self_session() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_id": "source_1",
                            "message": "hi",
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].session_id cannot send a message to the same session")
        );
    }

    #[tokio::test]
    async fn validate_batch_accepts_create_items() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_name": "Worker Session",
                            "message": "hi",
                            "agent_type": "agentic",
                        },
                        {
                            "session_name": "Plan Session",
                            "message": "plan",
                            "agent_type": "Plan",
                        },
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_batch_accepts_mixed_send_and_create_items() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_name": "Worker Session",
                            "message": "hi",
                            "agent_type": "agentic",
                        },
                        {
                            "session_id": "worker_2",
                            "message": "hello worker",
                        },
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_batch_rejects_worktree_with_session_id() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_id": "worker_1",
                            "message": "hello",
                            "worktree": {"baseRef": "main"},
                        },
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result, "{:?}", validation.message);
        let message = validation.message.unwrap_or_default();
        assert!(
            message.contains("worktree is only allowed when session_id is omitted"),
            "unexpected: {:?}",
            message
        );
    }

    #[tokio::test]
    async fn validate_batch_accepts_create_item_with_worktree() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_name": "Worker Session",
                            "message": "hi",
                            "agent_type": "agentic",
                            "worktree": {"copyLocalChanges": false},
                        },
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_batch_rejects_create_item_empty_worktree_base_ref() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_name": "Worker Session",
                            "message": "hi",
                            "agent_type": "agentic",
                            "worktree": {"baseRef": "   "},
                        },
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result, "{:?}", validation.message);
        let message = validation.message.unwrap_or_default();
        assert!(
            message.contains("worktree.base_ref must not be empty"),
            "unexpected: {:?}",
            message
        );
    }

    #[test]
    fn batch_item_result_maps_success_outcome() {
        let item = BatchItem {
            session_id: Some("worker_1".to_string()),
            session_name: None,
            message: "hello".to_string(),
            agent_type: None,
            worktree: None,
        };
        let outcome = DispatchOutcome {
            target_session_id: "worker_1".to_string(),
            target_agent_type: "agentic".to_string(),
            created_session_id: None,
            workspace_path: "/repo".to_string(),
            result_text: "Message accepted for session 'worker_1' in workspace '/repo' using agent type 'agentic'."
                .to_string(),
            created_worktree: None,
        };

        let result = SessionMessageTool::batch_item_result(&item, Ok(outcome));

        assert_eq!(result["status"].as_str(), Some("success"));
        assert_eq!(result["target_session_id"].as_str(), Some("worker_1"));
        assert_eq!(result["target_agent_type"].as_str(), Some("agentic"));
        assert_eq!(result["target_workspace"].as_str(), Some("/repo"));
        assert!(result["created_session_id"].is_null());
        assert_eq!(
            result["result"].as_str(),
            Some("Message accepted for session 'worker_1' in workspace '/repo' using agent type 'agentic'.")
        );
    }

    #[test]
    fn batch_item_result_maps_failure_outcome() {
        let item = BatchItem {
            session_id: Some("worker_2".to_string()),
            session_name: None,
            message: "hi".to_string(),
            agent_type: None,
            worktree: None,
        };
        let error = BitFunError::tool(
            "SessionMessage cannot send a message to the same session".to_string(),
        );
        let expected_error = error.to_string();

        let result = SessionMessageTool::batch_item_result(&item, Err(error));

        assert_eq!(result["status"].as_str(), Some("error"));
        assert_eq!(result["session_id"].as_str(), Some("worker_2"));
        assert_eq!(result["session_name"].as_str(), None);
        assert_eq!(result["error"].as_str(), Some(expected_error.as_str()));
    }

    #[test]
    fn batch_results_keeps_successes_and_records_failure_in_order() {
        // A failure in the middle must not roll back the earlier success and
        // must not stop the later success: every item yields exactly one entry.
        let first = BatchItem {
            session_id: Some("worker_1".to_string()),
            session_name: None,
            message: "hello".to_string(),
            agent_type: None,
            worktree: None,
        };
        let failed = BatchItem {
            session_id: Some("worker_2".to_string()),
            session_name: None,
            message: "hi".to_string(),
            agent_type: None,
            worktree: None,
        };
        let last = BatchItem {
            session_id: Some("worker_3".to_string()),
            session_name: None,
            message: "still runs".to_string(),
            agent_type: None,
            worktree: None,
        };

        let first_outcome = DispatchOutcome {
            target_session_id: "worker_1".to_string(),
            target_agent_type: "agentic".to_string(),
            created_session_id: None,
            workspace_path: "/repo".to_string(),
            result_text: "Message accepted for session 'worker_1' in workspace '/repo' using agent type 'agentic'."
                .to_string(),
            created_worktree: None,
        };
        let last_outcome = DispatchOutcome {
            target_session_id: "worker_3".to_string(),
            target_agent_type: "agentic".to_string(),
            created_session_id: None,
            workspace_path: "/repo".to_string(),
            result_text: "Message accepted for session 'worker_3' in workspace '/repo' using agent type 'agentic'."
                .to_string(),
            created_worktree: None,
        };
        let failure = BitFunError::tool("Session 'worker_2' not found".to_string());
        let expected_error = failure.to_string();

        let results = SessionMessageTool::batch_results(
            &[first, failed, last],
            vec![Ok(first_outcome), Err(failure), Ok(last_outcome)],
        );

        assert_eq!(results.len(), 3);
        assert_eq!(results[0]["status"].as_str(), Some("success"));
        assert_eq!(results[0]["target_session_id"].as_str(), Some("worker_1"));
        assert_eq!(results[1]["status"].as_str(), Some("error"));
        assert_eq!(results[1]["session_id"].as_str(), Some("worker_2"));
        assert_eq!(results[1]["error"].as_str(), Some(expected_error.as_str()));
        assert_eq!(results[2]["status"].as_str(), Some("success"));
        assert_eq!(results[2]["target_session_id"].as_str(), Some("worker_3"));
    }

    #[test]
    fn summarize_batch_results_counts_and_mentions_failed_items() {
        let results = vec![
            json!({"status": "success"}),
            json!({"status": "error"}),
            json!({"status": "success"}),
        ];

        let (succeeded, failed, summary) = SessionMessageTool::summarize_batch_results(&results);

        assert_eq!(succeeded, 2);
        assert_eq!(failed, 1);
        assert!(summary.starts_with(
            "Batch dispatch of 3 message(s): 2 succeeded, 1 failed. Successful items are not rolled back; retry only the failed items"
        ));
        assert!(summary.contains("A failed item never rolls back earlier successes, and later items still ran."));
    }

    #[test]
    fn summarize_batch_results_all_success_omits_failure_note() {
        let results = vec![json!({"status": "success"}), json!({"status": "success"})];

        let (succeeded, failed, summary) = SessionMessageTool::summarize_batch_results(&results);

        assert_eq!(succeeded, 2);
        assert_eq!(failed, 0);
        assert!(summary.starts_with(
            "Batch dispatch of 2 message(s): 2 succeeded, 0 failed. Successful items are not rolled back; retry only the failed items"
        ));
        assert!(!summary.contains("A failed item never rolls back earlier successes"));
    }
}
