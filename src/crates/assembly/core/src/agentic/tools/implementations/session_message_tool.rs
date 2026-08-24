use super::session_control_tool::get_available_agent_type_ids_for_creation;
use super::util::normalize_path;
use crate::agentic::coordination::{
    get_global_coordinator, get_global_scheduler, DialogSubmissionPolicy, DialogTriggerSource,
};
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::workspace_paths::posix_style_path_is_absolute;
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_agent_tools::ACP_TOOL_PREFIX;
use bitfun_core_types::SessionExecutionTarget;
use bitfun_runtime_ports::{
    AcpClientBitfunMessageRequest, AcpClientMessageRequest, AgentDialogPrependedReminder,
    AgentDialogTurnRequest, AgentSessionCreateRequest, AgentSessionListRequest,
    AgentSessionReplyRoute, AgentSessionSummary, AgentSessionWorkspaceBinding,
    AgentSessionWorkspaceRequest,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

/// SessionMessage tool - send a message to another session via the dialog scheduler
pub struct SessionMessageTool;

/// Background ACP direct-delivery window (seconds). The tool returns immediately
/// and the external ACP turn runs asynchronously under this cap.
const ACP_DIRECT_TIMEOUT_SECONDS: u64 = 1800;

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

    /// The ACP client id when the target agent type is an ACP bridge agent
    /// (`acp__<client_id>`; see `ACP_TOOL_PREFIX`), otherwise `None`. ACP
    /// targets bypass the local model entirely: the message is forwarded through
    /// the ACP client port instead of submitting a local dialog turn.
    fn acp_client_id_from_agent_type(agent_type: &str) -> Option<&str> {
        agent_type
            .strip_prefix(ACP_TOOL_PREFIX)
            .filter(|client_id| !client_id.trim().is_empty())
    }

    /// The ACP client id when `session_id` is a flow session id of the shape
    /// `acp_<client_id>_<uuid>`; otherwise `None`. Flow sessions live in the ACP
    /// persistence store, not the internal session store. Single authoritative
    /// implementation lives in `bitfun_runtime_ports` (d3-P2-2).
    fn acp_flow_client_id_from_session_id(session_id: &str) -> Option<&str> {
        bitfun_runtime_ports::acp_flow_client_id_from_session_id(session_id).and_then(|_| {
            let start = "acp_".len();
            let end = session_id.len().checked_sub(37)?;
            session_id.get(start..end)
        })
    }

    /// COORD-03 authoritative check before routing a flow-session target through
    /// the external ACP direct path: the shape is only a clue, the persisted
    /// flow-session registry (provider=acp + acpClientId) is authoritative.
    /// Rejects recycled (`Missing`), non-ACP (`NotAcpFlow`) or misowned
    /// (client mismatch) sessions so a stale/shape-colliding internal session is
    /// never routed to an external ACP agent.
    async fn verify_acp_flow_session_registry(
        workspace_path: &str,
        session_id: &str,
        flow_client_id: &str,
    ) -> BitFunResult<()> {
        use crate::agentic::persistence::PersistenceManager;
        use crate::infrastructure::get_path_manager_arc;
        use crate::service::remote_ssh::workspace_state::get_effective_session_path;
        use bitfun_core_types::{SESSION_PROVIDER_ACP, SESSION_PROVIDER_METADATA_KEY};

        let storage_path = get_effective_session_path(workspace_path, None, None).await;
        let persistence = PersistenceManager::new(get_path_manager_arc())
            .map_err(|error| BitFunError::tool(error.to_string()))?;
        let Some(metadata) = persistence
            .load_session_metadata(&storage_path, session_id)
            .await
            .map_err(|error| BitFunError::tool(error.to_string()))?
        else {
            return Err(BitFunError::tool(format!(
                "ACP flow session '{}' was not found in the flow-session registry; it may have been recycled or never created",
                session_id
            )));
        };
        let Some(custom) = metadata.custom_metadata.as_ref() else {
            return Err(BitFunError::tool(format!(
                "session '{}' is not an ACP flow session (its persisted record is not an ACP session record); refusing to route it through the external ACP direct path",
                session_id
            )));
        };
        if custom
            .get(SESSION_PROVIDER_METADATA_KEY)
            .and_then(Value::as_str)
            != Some(SESSION_PROVIDER_ACP)
        {
            return Err(BitFunError::tool(format!(
                "session '{}' is not an ACP flow session (its persisted record is not an ACP session record); refusing to route it through the external ACP direct path",
                session_id
            )));
        }
        let registered_client = custom
            .get("acpClientId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if registered_client != Some(flow_client_id) {
            return Err(BitFunError::tool(format!(
                "ACP flow session '{}' is registered for client '{}', not '{}'; refusing to route",
                session_id,
                registered_client.unwrap_or(""),
                flow_client_id
            )));
        }
        Ok(())
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
}

#[derive(Debug, Clone, Deserialize)]
struct SessionMessageInput {
    workspace: Option<String>,
    session_id: Option<String>,
    session_name: Option<String>,
    message: String,
    agent_type: Option<String>,
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
                    "description": "Message to send to the target session."
                },
                "agent_type": {
                    "type": "string",
                    "description": "Required when session_id is omitted. Not allowed when sending to an existing session. Use \"acp__<client_id>\" to forward directly to a real external ACP agent."
                }
            },
            "required": ["message"],
            "additionalProperties": false
        })
    }

    async fn input_schema_for_model_with_context(&self, context: Option<&ToolUseContext>) -> Value {
        let agent_type_ids = get_available_agent_type_ids_for_creation(context).await;
        let agent_type_enum: Vec<&str> = agent_type_ids.iter().map(String::as_str).collect();
        let mut schema = self.input_schema();
        if let Some(agent_type) = schema
            .get_mut("properties")
            .and_then(|props| props.get_mut("agent_type"))
        {
            agent_type["enum"] = json!(agent_type_enum);
        }
        schema
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

        if parsed.message.trim().is_empty() {
            return ValidationResult {
                result: false,
                message: Some("message cannot be empty".to_string()),
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
            scheduler.clone(),
        )
        .map_err(BitFunError::tool)?;

        let (target_session_id, target_agent_type, created_session_id, workspace_target) =
            if let Some(target_session_id) = params.session_id.clone() {
                if source_session_id == target_session_id {
                    return Err(BitFunError::tool(
                        "SessionMessage cannot send a message to the same session".to_string(),
                    ));
                }

                let workspace_target = runtime
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

                let visible_sessions = runtime
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
                        runtime
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
                // worktree is not supported with acp__ agent types: the external
                // ACP process runs in its own workspace, not a managed worktree.
                if Self::acp_client_id_from_agent_type(&agent_type).is_some()
                    && workspace_target.execution_target.is_some()
                {
                    return Err(BitFunError::tool(
                        "worktree is not supported with acp__ agent types".to_string(),
                    ));
                }
                let created_by = self.creator_session_marker(context)?;
                let mut metadata = serde_json::Map::new();
                metadata.insert("createdBy".to_string(), json!(created_by));
                let session = runtime
                    .create_session(AgentSessionCreateRequest {
                        session_name,
                        agent_type: agent_type.clone(),
                        workspace_path: Some(workspace_target.workspace_path.clone()),
                        project_workspace_path: Some(
                            workspace_target.project_workspace_path.clone(),
                        ),
                        execution_target: workspace_target.execution_target.clone(),
                        workspace_id: workspace_target.workspace_id.clone(),
                        remote_connection_id: workspace_target.remote_connection_id.clone(),
                        remote_ssh_host: workspace_target.remote_ssh_host.clone(),
                        model_id: None,
                        metadata,
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;

                (
                    session.session_id.clone(),
                    session.agent_type.clone(),
                    Some(session.session_id),
                    workspace_target,
                )
            };

        // ACP direct path: forward a message to a real external ACP agent
        // (`acp__<client>` target or `acp_<client>_<uuid>` flow session) through
        // the ACP client port, bypassing the local model. No local dialog turn
        // is submitted, so no bridge re-translation / double billing happens.
        let acp_target_via_agent = Self::acp_client_id_from_agent_type(&target_agent_type);
        let acp_target_via_flow = Self::acp_flow_client_id_from_session_id(&target_session_id);
        if acp_target_via_agent.is_some() || acp_target_via_flow.is_some() {
            let port = coordinator.acp_client_port().ok_or_else(|| {
                BitFunError::tool(
                    "ACP client port is not available; the desktop host did not inject it"
                        .to_string(),
                )
            })?;
            // COORD-03: the `acp__`/`acp_<client>_<uuid>` shape is only a clue —
            // the ACP registry is authoritative. Verify the target is a live,
            // correctly-owned external ACP target before routing.
            if let Some(client_id) = acp_target_via_agent {
                let listed = port.list_clients().await.map_err(|error| {
                    BitFunError::tool(format!(
                        "failed to verify the ACP client registry for agent type '{}': {}",
                        target_agent_type, error.message
                    ))
                })?;
                if !listed
                    .clients
                    .iter()
                    .any(|client| client.client_id == client_id)
                {
                    return Err(BitFunError::tool(format!(
                        "session '{}' uses agent type '{}' but ACP client '{}' is not registered; refusing to route to a non-existent external agent",
                        target_session_id, target_agent_type, client_id
                    )));
                }
            } else if let Some(flow_client_id) = acp_target_via_flow {
                // Flow sessions live in the ACP flow-session registry (provider=acp
                // persisted record). Verify the record is active and owned by the
                // same client before routing; reject recycled/misowned sessions.
                Self::verify_acp_flow_session_registry(
                    &workspace_target.workspace_path,
                    &target_session_id,
                    flow_client_id,
                )
                .await?;
            }
            let (chunk_tx, _chunk_rx) = tokio::sync::mpsc::unbounded_channel();
            let target_session_for_delivery = target_session_id.clone();
            let user_input = params.message.clone();
            let target_workspace = workspace_target.workspace_path.clone();
            let target_agent_type_for_delivery = target_agent_type.clone();
            let sender_session_id = source_session_id.clone();
            let sender_workspace = source_workspace.clone();
            let sender_conn = source_remote_connection_id.clone();
            let sender_ssh = source_remote_ssh_host.clone();
            let spawn_client_id = acp_target_via_agent.map(ToOwned::to_owned);
            let port_for_spawn = port.clone();
            let scheduler_for_spawn = scheduler.clone();
            // Deliver asynchronously so the tool returns immediately; once the
            // external ACP turn completes, the full reply is routed back to the
            // sender session (K20: response must not be dropped).
            tokio::spawn(async move {
                let delivery = if let Some(client_id) = spawn_client_id {
                    port_for_spawn
                        .send_message_to_bitfun_session_stream(
                            AcpClientBitfunMessageRequest {
                                client_id,
                                bitfun_session_id: target_session_for_delivery.clone(),
                                message: user_input,
                                workspace_path: Some(target_workspace.clone()),
                                timeout_seconds: Some(ACP_DIRECT_TIMEOUT_SECONDS),
                            },
                            chunk_tx,
                        )
                        .await
                } else {
                    port_for_spawn
                        .send_message_stream(
                            AcpClientMessageRequest {
                                session_id: target_session_for_delivery.clone(),
                                message: user_input,
                                workspace_path: Some(target_workspace.clone()),
                                timeout_seconds: Some(ACP_DIRECT_TIMEOUT_SECONDS),
                            },
                            chunk_tx,
                        )
                        .await
                };
                if let Ok(result) = delivery {
                    let notice = format!(
                        "External ACP session '{}' responded: {}",
                        target_session_for_delivery, result.response
                    );
                    let _ = scheduler_for_spawn
                        .deliver_background_result(
                            sender_session_id,
                            target_agent_type_for_delivery,
                            Some(sender_workspace),
                            sender_conn,
                            sender_ssh,
                            notice,
                            Some(format!(
                                "External ACP session '{}' responded; the full reply is delivered back.",
                                target_session_for_delivery
                            )),
                            None,
                        )
                        .await;
                }
            });
            let result_for_assistant = format!(
                "Message accepted for external ACP session '{}' in workspace '{}' using agent type '{}'.",
                target_session_id, workspace_target.workspace_path, target_agent_type
            );
            return Ok(vec![ToolResult::Result {
                data: json!({
                    "success": true,
                    "target_workspace": workspace_target.workspace_path.clone(),
                    "target_session_id": target_session_id.clone(),
                    "target_agent_type": target_agent_type.clone(),
                    "created_session_id": created_session_id.clone(),
                }),
                result_for_assistant: Some(result_for_assistant),
                image_attachments: None,
            }]);
        }

        let (forwarded_message, prepended_messages) =
            self.format_forwarded_message(&params.message);

        runtime
            .submit_dialog_turn(AgentDialogTurnRequest {
                session_id: target_session_id.clone(),
                message: forwarded_message,
                original_message: Some(params.message.clone()),
                turn_id: None,
                execution: Default::default(),
                agent_type: target_agent_type.clone(),
                workspace_path: Some(workspace_target.workspace_path.clone()),
                remote_connection_id: workspace_target.remote_connection_id.clone(),
                remote_ssh_host: workspace_target.remote_ssh_host.clone(),
                policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
                reply_route: Some(AgentSessionReplyRoute {
                    source_session_id,
                    source_workspace_path: source_workspace,
                    source_remote_connection_id,
                    source_remote_ssh_host,
                }),
                prepended_reminders: prepended_messages,
                attachments: Vec::new(),
                metadata: Self::forwarded_user_input_metadata(context),
            })
            .await
            .map_err(|error| {
                BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
            })?;

        Ok(vec![ToolResult::Result {
            data: json!({
                "success": true,
                "target_workspace": workspace_target.workspace_path.clone(),
                "target_session_id": target_session_id.clone(),
                "target_agent_type": target_agent_type.clone(),
                "created_session_id": created_session_id.clone(),
            }),
            result_for_assistant: Some(if let Some(created_session_id) = created_session_id {
                format!(
                    "Created session '{}' and accepted the message in workspace '{}' using agent type '{}'.",
                    created_session_id, workspace_target.workspace_path, target_agent_type
                )
            } else {
                format!(
                    "Message accepted for session '{}' in workspace '{}' using agent type '{}'.",
                    target_session_id, workspace_target.workspace_path, target_agent_type
                )
            }),
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

    // -- ACP registry survival validation (COORD-03) --

    #[test]
    fn acp_client_id_from_agent_type_recognizes_acp_target() {
        assert_eq!(
            SessionMessageTool::acp_client_id_from_agent_type("acp__codex"),
            Some("codex")
        );
        assert_eq!(
            SessionMessageTool::acp_client_id_from_agent_type("acp__"),
            None
        );
        assert_eq!(
            SessionMessageTool::acp_client_id_from_agent_type("agentic"),
            None
        );
    }

    #[test]
    fn acp_flow_client_id_from_session_id_recognizes_flow_shapes() {
        assert_eq!(
            SessionMessageTool::acp_flow_client_id_from_session_id(
                "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"
            ),
            Some("codex")
        );
        assert_eq!(
            SessionMessageTool::acp_flow_client_id_from_session_id("worker_1"),
            None
        );
    }

    #[tokio::test]
    async fn verify_acp_flow_session_registry_missing_rejects() {
        // No persisted record in an empty workspace -> the flow-session registry
        // reports Missing -> the target is rejected (a recycled/missing flow
        // session is never routed to an external ACP agent).
        let workspace = TestTempDir::new("bitfun-acp-registry-missing");
        let result = SessionMessageTool::verify_acp_flow_session_registry(
            &workspace.as_string(),
            "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b",
            "codex",
        )
        .await;
        assert!(
            result.is_err(),
            "missing flow-session record must be rejected"
        );
    }

    // -- P7 registry fixture tests (COORD-03): seed a persistence record and
    // verify Active / NotAcpFlow ×2 / client-mismatch branches. Uses the test
    // path manager override so fixtures stay in a temp root (zero real-config
    // impact).

    async fn seed_session_metadata(
        workspace: &str,
        session_id: &str,
        custom: Option<serde_json::Value>,
    ) {
        use bitfun_services_core::session::SessionMetadata;
        let persistence = crate::agentic::persistence::PersistenceManager::new(
            crate::infrastructure::get_path_manager_arc(),
        )
        .map_err(|error| panic!("persistence init: {error}"))
        .unwrap();
        let mut metadata = SessionMetadata::new(
            session_id.to_string(),
            "acp-fixture".to_string(),
            "agentic".to_string(),
            "default".to_string(),
        );
        metadata.custom_metadata = custom;
        persistence
            .save_session_metadata(std::path::Path::new(workspace), &metadata)
            .await
            .map_err(|error| panic!("save metadata: {error}"))
            .unwrap();
    }

    #[tokio::test]
    async fn verify_acp_flow_session_registry_not_acp_custom_none_rejects() {
        crate::infrastructure::app_paths::path_manager::set_test_path_manager_override(
            std::sync::Arc::new(
                crate::infrastructure::app_paths::path_manager::PathManager::with_user_root_for_tests(
                    std::env::temp_dir().join("bitfun-acp-fixture-none"),
                ),
            ),
        );
        let ws = TestTempDir::new("bitfun-acp-fixture-none-ws");
        let sid = "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b";
        seed_session_metadata(&ws.as_string(), sid, None).await;
        let result =
            SessionMessageTool::verify_acp_flow_session_registry(&ws.as_string(), sid, "codex")
                .await;
        assert!(
            result.is_err(),
            "custom None is NotAcpFlow and must be rejected"
        );
    }

    #[tokio::test]
    async fn verify_acp_flow_session_registry_not_acp_provider_not_acp_rejects() {
        crate::infrastructure::app_paths::path_manager::set_test_path_manager_override(
            std::sync::Arc::new(
                crate::infrastructure::app_paths::path_manager::PathManager::with_user_root_for_tests(
                    std::env::temp_dir().join("bitfun-acp-fixture-provider"),
                ),
            ),
        );
        let ws = TestTempDir::new("bitfun-acp-fixture-provider-ws");
        let sid = "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b";
        seed_session_metadata(
            &ws.as_string(),
            sid,
            Some(serde_json::json!({ "provider": "internal", "acpClientId": "codex" })),
        )
        .await;
        let result =
            SessionMessageTool::verify_acp_flow_session_registry(&ws.as_string(), sid, "codex")
                .await;
        assert!(
            result.is_err(),
            "non-acp provider is NotAcpFlow and must be rejected"
        );
    }

    #[tokio::test]
    async fn verify_acp_flow_session_registry_active_client_match_succeeds() {
        crate::infrastructure::app_paths::path_manager::set_test_path_manager_override(
            std::sync::Arc::new(
                crate::infrastructure::app_paths::path_manager::PathManager::with_user_root_for_tests(
                    std::env::temp_dir().join("bitfun-acp-fixture-active"),
                ),
            ),
        );
        let ws = TestTempDir::new("bitfun-acp-fixture-active-ws");
        let sid = "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b";
        seed_session_metadata(
            &ws.as_string(),
            sid,
            Some(serde_json::json!({ "provider": "acp", "acpClientId": "codex" })),
        )
        .await;
        let result =
            SessionMessageTool::verify_acp_flow_session_registry(&ws.as_string(), sid, "codex")
                .await;
        assert!(
            result.is_ok(),
            "provider=acp + client match must route (Active)"
        );
    }

    #[tokio::test]
    async fn verify_acp_flow_session_registry_client_mismatch_rejects() {
        crate::infrastructure::app_paths::path_manager::set_test_path_manager_override(
            std::sync::Arc::new(
                crate::infrastructure::app_paths::path_manager::PathManager::with_user_root_for_tests(
                    std::env::temp_dir().join("bitfun-acp-fixture-mismatch"),
                ),
            ),
        );
        let ws = TestTempDir::new("bitfun-acp-fixture-mismatch-ws");
        let sid = "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b";
        seed_session_metadata(
            &ws.as_string(),
            sid,
            Some(serde_json::json!({ "provider": "acp", "acpClientId": "codebuddy" })),
        )
        .await;
        let result =
            SessionMessageTool::verify_acp_flow_session_registry(&ws.as_string(), sid, "codex")
                .await;
        assert!(result.is_err(), "client mismatch must be rejected");
    }
}
