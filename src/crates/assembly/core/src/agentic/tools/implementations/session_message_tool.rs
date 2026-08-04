use super::session_control_tool::get_available_agent_type_ids_for_creation;
use super::util::normalize_path;
use crate::agentic::agents::AcpAgent;
use crate::agentic::coordination::plan_todo_binding::{
    PLAN_FILE_METADATA_KEY, TODO_ID_METADATA_KEY,
};
use crate::agentic::coordination::{
    get_global_coordinator, get_global_scheduler, ConversationCoordinator, DialogScheduler,
    DialogSubmissionPolicy, DialogTriggerSource,
};
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::restrictions::get_session_role;
use crate::agentic::tools::workspace_paths::posix_style_path_is_absolute;
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_core_types::SessionExecutionTarget;
use bitfun_runtime_ports::{
    AcpClientBitfunMessageRequest, AcpClientMessageRequest, AcpClientPort, AgentDialogPrependedReminder,
    AgentDialogSteerRequest, AgentDialogTurnPort, AgentDialogTurnRequest, AgentSessionCreateRequest,
    AgentSessionListRequest, AgentSessionReplyRoute, AgentSessionSummary,
    AgentSessionWorkspaceBinding, AgentSessionWorkspaceRequest,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use log::{info, warn};

/// Primary channel for legion communication. With a session_id, messages can be sent and received across conversations.
/// Obtain session_id via Task spawn or SessionControl list_tasks.
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

/// Source-session facts and global runtime handles shared by a single
/// dispatch and by every batch item. Built once per tool call so a batch
/// dispatch performs a single resource setup.
struct DispatchShared {
    source_session_id: String,
    source_workspace: String,
    source_remote_connection_id: Option<String>,
    source_remote_ssh_host: Option<String>,
    coordinator: Arc<ConversationCoordinator>,
    scheduler: Arc<DialogScheduler>,
    runtime: bitfun_agent_runtime::sdk::AgentRuntime,
}

/// Result of one create+send (or send-to-existing) dispatch.
struct DispatchOutcome {
    target_session_id: String,
    target_agent_type: String,
    created_session_id: Option<String>,
    workspace_path: String,
    delivery: &'static str,
    result_text: String,
    /// External response of the ACP direct path; `None` for local dispatches.
    acp_response: Option<String>,
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

    fn forwarded_user_input_metadata(
        context: &ToolUseContext,
        sender: &SenderIdentity,
    ) -> serde_json::Map<String, Value> {
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
        // Sender identity triple for UI badges on forwarded agent messages
        // (R-23): every field degrades gracefully when unknown, so the badge
        // renders with whatever is available and never blocks delivery.
        metadata.insert("senderSessionId".to_string(), json!(sender.session_id));
        if let Some(role) = &sender.role {
            metadata.insert("senderRole".to_string(), json!(role));
        }
        if let Some(depth) = sender.depth {
            metadata.insert("senderDepth".to_string(), json!(depth));
        }
        if let Some(name) = sender
            .name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            metadata.insert("senderName".to_string(), json!(name));
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

    /// Best-effort identity of the sending session: RBAC role (R-14
    /// SESSION_ROLES registry), session-tree depth (R-19), and display name
    /// (session name, else agent type). Every field degrades gracefully when
    /// unknown, so a forwarding send never fails because identity data is
    /// missing.
    #[allow(clippy::too_many_arguments)]
    async fn resolve_sender_identity(
        &self,
        runtime: &bitfun_agent_runtime::sdk::AgentRuntime,
        context: &ToolUseContext,
        source_session_id: &str,
        source_workspace: &str,
        source_remote_connection_id: Option<&str>,
        source_remote_ssh_host: Option<&str>,
        coordinator: &ConversationCoordinator,
    ) -> SenderIdentity {
        let role = get_session_role(source_session_id)
            .map(|agent_role| format_role_display(agent_role.as_str()));
        let depth = coordinator.session_tree().get_depth(source_session_id);
        let session_name = runtime
            .list_sessions(AgentSessionListRequest {
                workspace_path: source_workspace.to_string(),
                remote_connection_id: source_remote_connection_id.map(ToOwned::to_owned),
                remote_ssh_host: source_remote_ssh_host.map(ToOwned::to_owned),
                include_hidden: false,
            })
            .await
            .ok()
            .and_then(|sessions| {
                sessions
                    .into_iter()
                    .find(|summary| summary.session_id == source_session_id)
                    .map(|summary| summary.session_name)
            })
            .filter(|name| !name.trim().is_empty());
        let name = session_name.or_else(|| {
            context
                .agent_type
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned)
        });
        SenderIdentity {
            session_id: source_session_id.to_string(),
            role,
            depth,
            name,
        }
    }

    fn format_forwarded_message(
        &self,
        message: &str,
        sender: &SenderIdentity,
    ) -> (String, Vec<AgentDialogPrependedReminder>) {
        let mut lines = vec![
            format!(
                "This request was sent by {} (session {}), not the human user. Do not use interactive tools for this request. In particular, do not call AskUserQuestion.",
                sender.display_label(),
                sender.session_id
            ),
            format!("From session: {}", sender.session_id),
            format!("From role: {}", sender.role.as_deref().unwrap_or("Agent")),
        ];
        if let Some(depth) = sender.depth {
            lines.push(format!("From depth: {depth}"));
        }
        if let Some(name) = sender
            .name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            lines.push(format!("From agent: {name}"));
        }
        (
            message.to_string(),
            vec![AgentDialogPrependedReminder {
                kind: "session_message_request".to_string(),
                text: lines.join("\n"),
            }],
        )
    }

}

/// Identity of the session that sent a forwarded message.
#[derive(Debug, Clone, PartialEq)]
struct SenderIdentity {
    /// Session id of the sender; always present.
    session_id: String,
    /// RBAC role display label (e.g. "Commander"), when registered.
    role: Option<String>,
    /// Session-tree depth (0 means the root level L0), when known.
    depth: Option<u32>,
    /// Session name, or the agent type fallback, when available.
    name: Option<String>,
}

impl SenderIdentity {
    /// "[Commander L0]" when role and depth are known; "[Commander]" with role
    /// only; "[Agent]" when no role is registered. Depth is omitted when unknown.
    fn role_label(&self) -> String {
        let role = self.role.as_deref().unwrap_or("Agent");
        match self.depth {
            Some(depth) => format!("[{role} L{depth}]"),
            None => format!("[{role}]"),
        }
    }

    /// "[Commander L0] Name (session abc)" or "[Agent] (session abc)" when the
    /// display name is unavailable.
    fn display_label(&self) -> String {
        let mut label = self.role_label();
        if let Some(name) = self.name.as_deref().filter(|value| !value.trim().is_empty()) {
            label.push(' ');
            label.push_str(name);
        }
        label
    }
}

/// "commander" -> "Commander", "punishment_executor" -> "PunishmentExecutor".
fn format_role_display(role: &str) -> String {
    role.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut word = first.to_uppercase().to_string();
                    word.push_str(chars.as_str());
                    word
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Lightweight UUID shape check (8-4-4-4-12, 36 chars) for the trailing
/// segment of an ACP flow session id (`acp_<client_id>_<uuid>`). Kept
/// dependency-free so core does not need the uuid crate for this guard.
fn looks_like_uuid(segment: &str) -> bool {
    segment.len() == 36
        && segment.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

use bitfun_runtime_ports::AgentType;

#[derive(Debug, Clone, Deserialize)]
struct SessionMessageInput {
    workspace: Option<String>,
    session_id: Option<String>,
    session_name: Option<String>,
    /// Top-level message for single-target dispatch. Mutually exclusive with
    /// `batch`: when batch is present this field must be omitted or empty.
    #[serde(default)]
    message: Option<String>,
    agent_type: Option<AgentType>,
    /// When true, deliver as an urgent mid-turn correction: if the target session
    /// is currently processing, the message is injected into its running turn via
    /// the UserSteering channel instead of starting a new turn. Falls back to
    /// normal delivery when the target session is not processing.
    #[serde(default)]
    urgent: bool,
    /// Optional plan-todo binding: when creating a new session, the dispatched
    /// turn carries planFile/todoId in the forwarded metadata so the scheduler
    /// auto-marks the plan todo (in_progress at turn start, completed when the
    /// turn finishes with a Completed outcome). Only allowed when session_id is
    /// omitted; both fields must be provided together.
    #[serde(default)]
    plan_file: Option<String>,
    #[serde(default)]
    todo_id: Option<String>,
    /// Batch dispatch: perform multiple create+send (or send-to-existing)
    /// operations in a single tool call. All items are validated up front (the
    /// whole batch is rejected when any item is structurally invalid), then each
    /// item executes sequentially and independently: a failed item never rolls
    /// back already-succeeded items and never stops later items. The top-level
    /// session fields (session_id/session_name/agent_type/urgent/plan_file/
    /// todo_id) must stay empty when batch is used; the top-level workspace is
    /// shared by every item that creates a new session.
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
    agent_type: Option<AgentType>,
    /// Per-item urgent delivery flag (same semantics as the top-level flag).
    #[serde(default)]
    urgent: bool,
    /// Per-item plan-todo binding (only when session_id is omitted, and
    /// requires todo_id).
    #[serde(default)]
    plan_file: Option<String>,
    /// Per-item todo id within plan_file (only when session_id is omitted, and
    /// requires plan_file).
    #[serde(default)]
    todo_id: Option<String>,
}

/// Delivery decision for an urgent message against a target session.
#[derive(Debug, Clone, PartialEq)]
enum UrgentDelivery {
    /// Target session is processing a turn; steer into the running turn.
    Steer { turn_id: String },
    /// Target session is idle (or the turn ended); use normal submission.
    NormalSubmit,
}

fn resolve_urgent_delivery(processing_turn_id: Option<String>) -> UrgentDelivery {
    match processing_turn_id {
        Some(turn_id) => UrgentDelivery::Steer { turn_id },
        None => UrgentDelivery::NormalSubmit,
    }
}

/// Dual-channel redundancy decision for urgent messages:
/// only attempt the steering channel when the message is urgent AND the target
/// session already exists (a brand-new session has no running turn to steer
/// into). Every other case uses the normal submission channel. When steering
/// is attempted but rejected, the caller falls back to the normal channel, so
/// one of the two channels always delivers the message.
fn should_attempt_steering(urgent: bool, created_session_id: Option<&str>) -> bool {
    urgent && created_session_id.is_none()
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
- Urgent correction: set "urgent" to true to inject the message into the target session's running turn instead of waiting for a new turn. Requires "session_id".

Use SessionControl (list) to discover existing sessions before sending messages.
Use SessionHistory to export a transcript of any session.
Use Task to spawn subagent sessions that can receive messages.

Allowed agent types when creating a session are dynamically resolved from the available agent registry (common values include "agentic", "Plan", "Cowork", and any custom/external subagent types).
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
                    "description": "Required when session_id is omitted. Valid values are dynamically resolved from the available agent registry."
                },
                "urgent": {
                    "type": "boolean",
                    "description": "When true, deliver as an urgent mid-turn correction: if the target session is processing, inject into its running turn via the UserSteering channel; otherwise fall back to normal delivery. Requires session_id."
                },
                "plan_file": {
                    "type": "string",
                    "description": "Optional plan-todo binding for a created session (only when session_id is omitted, and requires todo_id): the plan file name or absolute path whose todo is auto-marked in_progress when the dispatched turn starts and completed when it finishes with a Completed outcome."
                },
                "todo_id": {
                    "type": "string",
                    "description": "Optional todo id within plan_file for a created session (only when session_id is omitted, and requires plan_file)."
                },
                "batch": {
                    "type": "array",
                    "description": "Batch dispatch: perform multiple create+send (or send-to-existing) operations in one tool call. Mutually exclusive with the top-level message and session fields; the top-level workspace is shared by items that create a session. All items validate up front; each item then runs independently (a failed item never rolls back succeeded ones). Item shape: {session_id?, session_name?, message, agent_type?, plan_file?, todo_id?, urgent?}.",
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
                                "description": "Required when session_id is omitted. Agent type for the new session."
                            },
                            "urgent": {
                                "type": "boolean",
                                "description": "Per-item urgent delivery flag (same semantics as the top-level flag). Requires session_id."
                            },
                            "plan_file": {
                                "type": "string",
                                "description": "Per-item plan-todo binding (only when session_id is omitted, and requires todo_id)."
                            },
                            "todo_id": {
                                "type": "string",
                                "description": "Per-item todo id within plan_file (only when session_id is omitted, and requires plan_file)."
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

    /// Dynamically resolves allowed agent_type values from the agent registry.
    async fn input_schema_for_model_with_context(&self, context: Option<&ToolUseContext>) -> Value {
        let agent_type_ids = get_available_agent_type_ids_for_creation(context).await;
        let agent_type_enum: Vec<&str> = agent_type_ids.iter().map(|s| s.as_str()).collect();
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
                    "enum": agent_type_enum,
                    "description": "Required when session_id is omitted. Not allowed when sending to an existing session."
                },
                "urgent": {
                    "type": "boolean",
                    "description": "When true, deliver as an urgent mid-turn correction: if the target session is processing, inject into its running turn via the UserSteering channel; otherwise fall back to normal delivery. Requires session_id."
                },
                "plan_file": {
                    "type": "string",
                    "description": "Optional plan-todo binding for a created session (only when session_id is omitted, and requires todo_id): the plan file name or absolute path whose todo is auto-marked in_progress when the dispatched turn starts and completed when it finishes with a Completed outcome."
                },
                "todo_id": {
                    "type": "string",
                    "description": "Optional todo id within plan_file for a created session (only when session_id is omitted, and requires plan_file)."
                },
                "batch": {
                    "type": "array",
                    "description": "Batch dispatch: perform multiple create+send (or send-to-existing) operations in one tool call. Mutually exclusive with the top-level message and session fields; the top-level workspace is shared by items that create a session. All items validate up front; each item then runs independently (a failed item never rolls back succeeded ones). Item shape: {session_id?, session_name?, message, agent_type?, plan_file?, todo_id?, urgent?}.",
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
                                "description": "Required when session_id is omitted. Agent type for the new session."
                            },
                            "urgent": {
                                "type": "boolean",
                                "description": "Per-item urgent delivery flag (same semantics as the top-level flag). Requires session_id."
                            },
                            "plan_file": {
                                "type": "string",
                                "description": "Per-item plan-todo binding (only when session_id is omitted, and requires todo_id)."
                            },
                            "todo_id": {
                                "type": "string",
                                "description": "Per-item todo id within plan_file (only when session_id is omitted, and requires plan_file)."
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

        // Batch mode: the whole batch is validated up front — any structurally
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

                if parsed.plan_file.is_some() || parsed.todo_id.is_some() {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "plan_file/todo_id binding is only allowed when session_id is omitted"
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
                if parsed.plan_file.is_some() != parsed.todo_id.is_some() {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "plan_file and todo_id must be provided together".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

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
        if let Some(batch) = input.get("batch").and_then(|value| value.as_array()) {
            return format!(
                "Batch dispatch {} message(s) in {}",
                batch.len(),
                workspace
            );
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
            "target_workspace": outcome.workspace_path,
            "target_session_id": outcome.target_session_id,
            "target_agent_type": outcome.target_agent_type,
            "created_session_id": outcome.created_session_id,
            "delivery": outcome.delivery,
        });
        // ACP direct path: the external response is exposed verbatim on the
        // result payload so programmatic callers can consume it.
        if let Some(response) = outcome.acp_response.as_ref() {
            data["response"] = json!(response);
        }
        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(outcome.result_text),
            image_attachments: None,
        }])
    }
}

impl SessionMessageTool {
    /// Validates a batch payload up front. Structural rules mirror the
    /// single-target shape, applied per item with `batch[N]` prefixes; any
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
            || parsed.plan_file.is_some()
            || parsed.todo_id.is_some()
            || parsed.urgent
        {
            return Self::invalid("session fields must be provided per batch item when batch is used");
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
                    if item.plan_file.is_some() || item.todo_id.is_some() {
                        return Self::invalid(format!(
                            "{} binding is only allowed when session_id is omitted",
                            field("plan_file/todo_id")
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
                    if item.plan_file.is_some() != item.todo_id.is_some() {
                        return Self::invalid(format!(
                            "{} and {} must be provided together",
                            field("plan_file"),
                            field("todo_id")
                        ));
                    }
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

    /// Resolves the source-session facts and the global coordinator, scheduler
    /// and runtime once per tool call, so a batch dispatch shares one resource
    /// setup instead of re-resolving globals for every item.
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
            scheduler.clone(),
        )
        .map_err(BitFunError::tool)?;
        Ok(DispatchShared {
            source_session_id,
            source_workspace,
            source_remote_connection_id,
            source_remote_ssh_host,
            coordinator,
            scheduler,
            runtime,
        })
    }

    /// The ACP client id when the target agent type is an ACP bridge agent
    /// (`acp__<client_id>`; see AcpAgent::agent_id_for), otherwise `None`.
    /// ACP targets bypass the local model entirely: SessionMessage forwards
    /// the message through the ACP client port instead of submitting a local
    /// dialog turn, so no bridge re-translation (and no double billing) can
    /// happen.
    fn acp_client_id_from_agent_type(agent_type: &str) -> Option<&str> {
        agent_type
            .strip_prefix(AcpAgent::agent_id_prefix())
            .filter(|client_id| !client_id.trim().is_empty())
    }

    /// The ACP client id when `session_id` is a flow session id of the shape
    /// `acp_<client_id>_<uuid>` (created by the frontend `create_acp_flow_session`,
    /// `acp_control` create, or the SessionControl `acp__` path; see
    /// interfaces/acp session_persistence.rs:44). Flow sessions live in the ACP
    /// persistence store, not the internal session store, so they are detected
    /// by id shape instead of a registry lookup. The trailing UUID segment is
    /// shape-checked so an internal session id that happens to start with
    /// `acp_` is never mistaken for a flow session.
    fn acp_flow_client_id_from_session_id(session_id: &str) -> Option<&str> {
        let rest = session_id.strip_prefix("acp_")?;
        let (client_id, uuid_segment) = rest.rsplit_once('_')?;
        if client_id.is_empty() || !looks_like_uuid(uuid_segment) {
            return None;
        }
        Some(client_id)
    }

    /// ACP direct path: forward the message to the external agent through the
    /// port, addressed by the internal BitFun session id (the same session
    /// identity the `acp__<client>__prompt` bridge tool uses), and return the
    /// external response verbatim. No local model turn is involved.
    ///
    /// 参考 bitfun-acp interfaces/acp/src/client/tool.rs:157-168 —
    /// AcpAgentTool::call_impl → service.prompt_agent（内部 session 键），
    /// Rust 翻译实现，非 Cargo 依赖。
    async fn dispatch_acp_direct(
        port: &dyn AcpClientPort,
        client_id: &str,
        bitfun_session_id: &str,
        message: &str,
        workspace_path: Option<String>,
    ) -> BitFunResult<String> {
        let sent = port
            .send_message_to_bitfun_session(AcpClientBitfunMessageRequest {
                client_id: client_id.to_string(),
                bitfun_session_id: bitfun_session_id.to_string(),
                message: message.to_string(),
                workspace_path,
                timeout_seconds: None,
            })
            .await
            .map_err(|error| {
                BitFunError::tool(format!(
                    "ACP client port failed ({:?}): {}",
                    error.kind, error.message
                ))
            })?;
        Ok(sent.response)
    }

    /// Performs one create+send (or send-to-existing) dispatch and returns the
    /// resolved outcome. Shared by the single-target call and every batch item.
    async fn dispatch_single(
        &self,
        params: SessionMessageInput,
        shared: &DispatchShared,
        context: &ToolUseContext,
    ) -> BitFunResult<DispatchOutcome> {
        let message = params
            .message
            .clone()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| BitFunError::tool("message cannot be empty".to_string()))?;
        let source_session_id = &shared.source_session_id;
        let source_workspace = &shared.source_workspace;
        let source_remote_connection_id = shared.source_remote_connection_id.as_deref();
        let source_remote_ssh_host = shared.source_remote_ssh_host.as_deref();
        let coordinator = &shared.coordinator;
        let scheduler = &shared.scheduler;
        let runtime = &shared.runtime;

        let (target_session_id, target_agent_type, created_session_id, workspace_target) =
            if let Some(target_session_id) = params.session_id.clone() {
                if source_session_id == &target_session_id {
                    return Err(BitFunError::tool(
                        "SessionMessage cannot send a message to the same session".to_string(),
                    ));
                }

                // ACP 流会话直通：session_id 形状 `acp_<client_id>_<uuid>`（前端
                // create_acp_flow_session / acp_control / SessionControl acp__ 创建的
                // 真外部 ACP 会话）。流会话不在内部 session store，无法走 workspace
                // binding / list_sessions 解析；直接经 AcpClientPort::send_message 真
                // 通道转发并返回外部响应原文（与 acp_message 同通道，无本地模型 turn）。
                if let Some(flow_client_id) =
                    Self::acp_flow_client_id_from_session_id(&target_session_id)
                {
                    let port = coordinator.acp_client_port().ok_or_else(|| {
                        BitFunError::tool(
                            "ACP client port is not available; the desktop host did not inject it"
                                .to_string(),
                        )
                    })?;
                    let workspace_path = params.workspace.clone().or_else(|| {
                        context
                            .workspace_root()
                            .map(|path| path.to_string_lossy().to_string())
                    });
                    let sent = port
                        .send_message(AcpClientMessageRequest {
                            session_id: target_session_id.clone(),
                            message: message.clone(),
                            workspace_path: workspace_path.clone(),
                            timeout_seconds: None,
                        })
                        .await
                        .map_err(|error| {
                            BitFunError::tool(format!(
                                "ACP client port failed ({:?}): {}",
                                error.kind, error.message
                            ))
                        })?;
                    // Resolve before the move below: the flow client id borrows
                    // from `target_session_id`, which is moved into the outcome.
                    let target_agent_type = format!("acp:{}", flow_client_id);
                    return Ok(DispatchOutcome {
                        target_session_id,
                        target_agent_type,
                        created_session_id: None,
                        workspace_path: workspace_path.unwrap_or_default(),
                        delivery: "acp_direct",
                        result_text: sent.response.clone(),
                        acp_response: Some(sent.response),
                    });
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
                        include_hidden: false,
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
                let created_by = self.creator_session_marker(context)?;
                let mut metadata = serde_json::Map::new();
                metadata.insert("createdBy".to_string(), json!(created_by));
                // Persistent copy of the plan-todo binding on the created
                // session record (the turn-channel copy is injected at submit).
                if let Some(plan_file) = params.plan_file.as_deref() {
                    metadata.insert(PLAN_FILE_METADATA_KEY.to_string(), json!(plan_file));
                }
                if let Some(todo_id) = params.todo_id.as_deref() {
                    metadata.insert(TODO_ID_METADATA_KEY.to_string(), json!(todo_id));
                }
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

        // ACP direct path: `acp__<client>` targets are external agents.
        // Forward the message through the ACP client port (addressed by the
        // internal BitFun session id, same identity the AcpAgentTool bridge
        // uses) and return the external response verbatim — no local model
        // turn, no bridge re-translation. When the port is unavailable the
        // dispatch fails loudly instead of falling back to the local model
        // (a fallback would re-introduce the double-billing path).
        if let Some(client_id) = Self::acp_client_id_from_agent_type(&target_agent_type) {
            let port = coordinator.acp_client_port().ok_or_else(|| {
                BitFunError::tool(
                    "ACP client port is not available; the desktop host did not inject it"
                        .to_string(),
                )
            })?;
            let response = Self::dispatch_acp_direct(
                port.as_ref(),
                client_id,
                &target_session_id,
                &message,
                Some(workspace_target.workspace_path.clone()),
            )
            .await?;
            return Ok(DispatchOutcome {
                target_session_id,
                target_agent_type,
                created_session_id,
                workspace_path: workspace_target.workspace_path,
                delivery: "acp_direct",
                result_text: response.clone(),
                acp_response: Some(response),
            });
        }

        let sender_identity = self
            .resolve_sender_identity(
                runtime,
                context,
                source_session_id,
                source_workspace,
                source_remote_connection_id,
                source_remote_ssh_host,
                coordinator,
            )
            .await;
        let (forwarded_message, prepended_messages) =
            self.format_forwarded_message(&message, &sender_identity);

        // Urgent delivery: when the target session is currently processing a turn,
        // inject the message into that running turn via the UserSteering channel
        // (interrupts after the current atomic unit) instead of starting a new turn.
        // Honest fallback: when the target session is not processing, or the steering
        // is rejected (the turn ended between the state query and the submit), deliver
        // through the normal submission path so the message is never dropped.
        let mut steering_turn_id: Option<String> = None;
        if should_attempt_steering(params.urgent, created_session_id.as_deref()) {
            match resolve_urgent_delivery(scheduler.current_processing_turn_id(&target_session_id)) {
                UrgentDelivery::Steer { turn_id } => {
                    match scheduler
                        .steer_dialog_turn(AgentDialogSteerRequest {
                            session_id: target_session_id.clone(),
                            turn_id: turn_id.clone(),
                            content: forwarded_message.clone(),
                            display_content: Some(message.clone()),
                            prepended_reminders: prepended_messages.clone(),
                        })
                        .await
                    {
                        Ok(_outcome) => {
                            steering_turn_id = Some(turn_id.clone());
                            info!(
                                "Urgent SessionMessage steered into running turn: source_session_id={}, target_session_id={}, turn_id={}",
                                source_session_id, target_session_id, turn_id
                            );
                        }
                        Err(error) => {
                            warn!(
                                "Urgent SessionMessage steering rejected, falling back to normal submit: target_session_id={}, turn_id={}, error={}",
                                target_session_id, turn_id, error
                            );
                        }
                    }
                }
                UrgentDelivery::NormalSubmit => {}
            }
        }

        if steering_turn_id.is_none() {
            // Turn-channel binding injection: when the caller bound the
            // dispatched session to a plan todo, carry planFile/todoId in the
            // forwarded turn metadata so the scheduler can auto-mark the todo
            // (in_progress at turn start, completed on a Completed outcome).
            let mut forwarded_metadata =
                Self::forwarded_user_input_metadata(context, &sender_identity);
            if let Some(plan_file) = params.plan_file.as_deref() {
                forwarded_metadata.insert(PLAN_FILE_METADATA_KEY.to_string(), json!(plan_file));
            }
            if let Some(todo_id) = params.todo_id.as_deref() {
                forwarded_metadata.insert(TODO_ID_METADATA_KEY.to_string(), json!(todo_id));
            }
            runtime
                .submit_dialog_turn(AgentDialogTurnRequest {
                    session_id: target_session_id.clone(),
                    message: forwarded_message,
                    original_message: Some(message.clone()),
                    turn_id: None,
                    execution: Default::default(),
                    agent_type: target_agent_type.clone(),
                    workspace_path: Some(workspace_target.workspace_path.clone()),
                    remote_connection_id: workspace_target.remote_connection_id.clone(),
                    remote_ssh_host: workspace_target.remote_ssh_host.clone(),
                    policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
                    reply_route: Some(AgentSessionReplyRoute {
                        source_session_id: source_session_id.clone(),
                        source_workspace_path: source_workspace.clone(),
                        source_remote_connection_id: source_remote_connection_id.map(ToOwned::to_owned),
                        source_remote_ssh_host: source_remote_ssh_host.map(ToOwned::to_owned),
                    }),
                    prepended_reminders: prepended_messages,
                    attachments: Vec::new(),
                    metadata: forwarded_metadata,
                })
                .await
                .map_err(|error| {
                    BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                })?;
        }

        let urgent_fell_back = params.urgent
            && steering_turn_id.is_none()
            && created_session_id.is_none();
        let mut result_text = if let Some(steered_turn_id) = steering_turn_id.as_ref() {
            format!(
                "Urgent message injected into the running turn '{}' of session '{}' in workspace '{}' using agent type '{}'.",
                steered_turn_id, target_session_id, workspace_target.workspace_path, target_agent_type
            )
        } else if let Some(created_session_id) = created_session_id.as_ref() {
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
        if urgent_fell_back {
            result_text.push_str(
                " Steering into the running turn was not possible (the target session was idle, its turn had just ended, or the queue was congested), so the urgent message was delivered as a normal submission instead of a mid-turn correction.",
            );
        }

        Ok(DispatchOutcome {
            target_session_id,
            target_agent_type,
            created_session_id,
            workspace_path: workspace_target.workspace_path,
            delivery: if steering_turn_id.is_some() {
                "steered"
            } else {
                "submitted"
            },
            result_text,
            acp_response: None,
        })
    }

    /// Batch dispatch: runs each item sequentially and independently. A failed
    /// item never rolls back already-succeeded items and never stops later
    /// items; the per-item result array keeps every session id so the caller
    /// can skip succeeded items when retrying the failed ones.
    async fn call_batch(
        &self,
        params: &SessionMessageInput,
        items: &[BatchItem],
        shared: &DispatchShared,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let mut results = Vec::with_capacity(items.len());
        for item in items {
            let item_params = SessionMessageInput {
                workspace: params.workspace.clone(),
                session_id: item.session_id.clone(),
                session_name: item.session_name.clone(),
                message: Some(item.message.clone()),
                agent_type: item.agent_type.clone(),
                urgent: item.urgent,
                plan_file: item.plan_file.clone(),
                todo_id: item.todo_id.clone(),
                batch: None,
            };
            match self.dispatch_single(item_params, shared, context).await {
                Ok(outcome) => {
                    let result_text = outcome.result_text;
                    let mut item_data = json!({
                        "status": "success",
                        "target_session_id": outcome.target_session_id,
                        "target_agent_type": outcome.target_agent_type,
                        "target_workspace": outcome.workspace_path,
                        "created_session_id": outcome.created_session_id,
                        "delivery": outcome.delivery,
                        "result": result_text,
                    });
                    // ACP direct path: expose the external response verbatim.
                    if let Some(response) = outcome.acp_response.as_ref() {
                        item_data["response"] = json!(response);
                    }
                    results.push(item_data);
                }
                Err(error) => {
                    warn!(
                        "Batch SessionMessage item failed (successful items are not rolled back): session_name={:?}, session_id={:?}, error={}",
                        item.session_name, item.session_id, error
                    );
                    results.push(json!({
                        "status": "error",
                        "session_name": item.session_name.clone(),
                        "session_id": item.session_id.clone(),
                        "error": error.to_string(),
                    }));
                }
            }
        }

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

    /// Aggregates per-item outcomes into success/failed counts and the summary
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tools::framework::ToolUseContext;
    use crate::agentic::WorkspaceBinding;
    use bitfun_core_types::{
        SessionExecutionTarget, SessionExecutionTargetKind, WorktreeLifecycle,
    };
    use bitfun_runtime_ports::{
        PortError, PortErrorKind, PortResult, RuntimeServiceCapability, RuntimeServicePort,
    };
    use serde_json::json;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;
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
    fn acp_flow_client_id_parses_flow_session_id() {
        assert_eq!(
            SessionMessageTool::acp_flow_client_id_from_session_id(
                "acp_codebuddy_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"
            ),
            Some("codebuddy")
        );
    }

    #[test]
    fn acp_flow_client_id_parses_client_ids_with_underscores() {
        assert_eq!(
            SessionMessageTool::acp_flow_client_id_from_session_id(
                "acp_claude_code_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"
            ),
            Some("claude_code")
        );
    }

    #[test]
    fn acp_flow_client_id_rejects_non_flow_session_ids() {
        // Internal session ids are not flow sessions even when they start with
        // "acp_": the trailing segment must be a well-formed UUID.
        assert_eq!(
            SessionMessageTool::acp_flow_client_id_from_session_id("acp_codebuddy"),
            None
        );
        assert_eq!(
            SessionMessageTool::acp_flow_client_id_from_session_id(
                "acp_codebuddy_not-a-uuid"
            ),
            None
        );
        assert_eq!(
            SessionMessageTool::acp_flow_client_id_from_session_id("session-123"),
            None
        );
        assert_eq!(
            SessionMessageTool::acp_flow_client_id_from_session_id(""),
            None
        );
    }

    #[test]
    fn looks_like_uuid_accepts_only_canonical_shape() {
        assert!(looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"));
        assert!(!looks_like_uuid("7f0e1a2b3c4d4e5f8a9b0c1d2e3f4a5b"));
        assert!(!looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b-extra"));
        assert!(!looks_like_uuid(""));
        assert!(!looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5"));
    }

    #[test]
    fn session_message_forwards_noninteractive_user_input_fact() {
        use bitfun_agent_runtime::user_questions::USER_INPUT_AVAILABLE_CONTEXT_KEY;

        let mut context = empty_context();
        context.custom_data.insert(
            USER_INPUT_AVAILABLE_CONTEXT_KEY.to_string(),
            Value::Bool(false),
        );
        let sender = SenderIdentity {
            session_id: "source-1".to_string(),
            role: Some("Commander".to_string()),
            depth: Some(0),
            name: Some("Mengdie".to_string()),
        };

        let metadata = SessionMessageTool::forwarded_user_input_metadata(&context, &sender);

        assert_eq!(
            metadata.get(USER_INPUT_AVAILABLE_CONTEXT_KEY),
            Some(&Value::Bool(false))
        );
        assert_eq!(metadata.get("senderSessionId"), Some(&Value::String("source-1".to_string())));
        assert_eq!(metadata.get("senderRole"), Some(&Value::String("Commander".to_string())));
        assert_eq!(metadata.get("senderDepth"), Some(&Value::from(0)));
        assert_eq!(metadata.get("senderName"), Some(&Value::String("Mengdie".to_string())));
    }

    #[test]
    fn forwarded_metadata_omits_unknown_sender_fields() {
        let context = empty_context();
        let sender = SenderIdentity {
            session_id: "source-2".to_string(),
            role: None,
            depth: None,
            name: None,
        };

        let metadata = SessionMessageTool::forwarded_user_input_metadata(&context, &sender);

        assert_eq!(metadata.get("senderSessionId"), Some(&Value::String("source-2".to_string())));
        assert!(!metadata.contains_key("senderRole"));
        assert!(!metadata.contains_key("senderDepth"));
        assert!(!metadata.contains_key("senderName"));
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
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            turn_count: 0,
            created_at_ms: 1,
            last_active_at_ms: 2,
            parent_session_id: None,
            status: None,
            is_daemon: false,
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
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            turn_count: 0,
            created_at_ms: 1,
            last_active_at_ms: 2,
            parent_session_id: None,
            status: None,
            is_daemon: false,
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
                    "agent_type": "Plan",
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
                    "agent_type": "agentic",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_new_session_accepts_plan_todo_binding() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "message": "hello",
                    "session_name": "Worker Session",
                    "agent_type": "agentic",
                    "plan_file": "my_plan_1234.plan.md",
                    "todo_id": "setup-auth",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_new_session_rejects_plan_file_without_todo_id() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "message": "hello",
                    "session_name": "Worker Session",
                    "agent_type": "agentic",
                    "plan_file": "my_plan_1234.plan.md",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("plan_file and todo_id must be provided together")
        );
    }

    #[tokio::test]
    async fn validate_new_session_rejects_todo_id_without_plan_file() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "message": "hello",
                    "session_name": "Worker Session",
                    "agent_type": "agentic",
                    "todo_id": "setup-auth",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("plan_file and todo_id must be provided together")
        );
    }

    #[tokio::test]
    async fn validate_existing_session_rejects_plan_todo_binding() {
        let tool = SessionMessageTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": "C:/work",
                    "session_id": "worker_1",
                    "message": "hello",
                    "plan_file": "my_plan_1234.plan.md",
                    "todo_id": "setup-auth",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("plan_file/todo_id binding is only allowed when session_id is omitted")
        );
    }

    #[test]
    fn session_message_input_parses_plan_todo_binding() {
        let input: SessionMessageInput = serde_json::from_value(json!({
            "workspace": "C:/work",
            "message": "hello",
            "session_name": "Worker Session",
            "agent_type": "agentic",
            "plan_file": "my_plan_1234.plan.md",
            "todo_id": "setup-auth",
        }))
        .expect("payload with plan-todo binding must parse");

        assert_eq!(
            input.plan_file.as_deref(),
            Some("my_plan_1234.plan.md")
        );
        assert_eq!(input.todo_id.as_deref(), Some("setup-auth"));
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

    #[test]
    fn session_message_input_defaults_urgent_to_false_for_backward_compat() {
        let input: SessionMessageInput = serde_json::from_value(json!({
            "session_id": "worker_1",
            "message": "hello",
        }))
        .expect("legacy payload without urgent must parse");

        assert!(!input.urgent);
    }

    #[test]
    fn session_message_input_parses_urgent_flag() {
        let input: SessionMessageInput = serde_json::from_value(json!({
            "session_id": "worker_1",
            "message": "stop what you are doing and correct this",
            "urgent": true,
        }))
        .expect("payload with urgent must parse");

        assert!(input.urgent);
    }

    #[test]
    fn urgent_delivery_steers_into_a_processing_turn() {
        assert_eq!(
            resolve_urgent_delivery(Some("turn-7".to_string())),
            UrgentDelivery::Steer {
                turn_id: "turn-7".to_string()
            }
        );
    }

    #[test]
    fn urgent_delivery_falls_back_to_normal_submit_for_idle_session() {
        assert_eq!(resolve_urgent_delivery(None), UrgentDelivery::NormalSubmit);
    }

    #[test]
    fn urgent_message_to_existing_session_attempts_steering_channel() {
        assert!(should_attempt_steering(true, None));
    }

    #[test]
    fn urgent_message_to_new_session_uses_normal_channel_only() {
        assert!(!should_attempt_steering(true, Some("new-session-1")));
    }

    #[test]
    fn non_urgent_message_never_attempts_steering_channel() {
        assert!(!should_attempt_steering(false, None));
        assert!(!should_attempt_steering(false, Some("new-session-1")));
    }

    #[test]
    fn forwarded_reminder_includes_full_sender_identity() {
        let sender = SenderIdentity {
            session_id: "source-1".to_string(),
            role: Some("Commander".to_string()),
            depth: Some(0),
            name: Some("Mengdie".to_string()),
        };
        let (message, reminders) =
            SessionMessageTool::new().format_forwarded_message("hello", &sender);
        assert_eq!(message, "hello");
        assert_eq!(reminders.len(), 1);
        let reminder = &reminders[0];
        assert_eq!(reminder.kind, "session_message_request");
        assert!(reminder.text.contains("[Commander L0]"));
        assert!(reminder.text.contains("Mengdie"));
        assert!(reminder.text.contains("(session source-1)"));
        assert!(reminder.text.contains("not the human user"));
        assert!(reminder.text.contains("From session: source-1"));
        assert!(reminder.text.contains("From role: Commander"));
        assert!(reminder.text.contains("From depth: 0"));
        assert!(reminder.text.contains("From agent: Mengdie"));
    }

    #[test]
    fn forwarded_reminder_falls_back_when_role_is_unregistered() {
        let sender = SenderIdentity {
            session_id: "source-2".to_string(),
            role: None,
            depth: Some(2),
            name: None,
        };
        let (_, reminders) = SessionMessageTool::new().format_forwarded_message("hello", &sender);
        let text = &reminders[0].text;
        assert!(text.contains("[Agent L2]"));
        assert!(text.contains("(session source-2)"));
        assert!(text.contains("From role: Agent"));
        assert!(text.contains("From depth: 2"));
        assert!(!text.contains("From agent:"));
    }

    #[test]
    fn forwarded_reminder_omits_depth_when_unknown() {
        let sender = SenderIdentity {
            session_id: "source-3".to_string(),
            role: Some("Executor".to_string()),
            depth: None,
            name: Some("Worker".to_string()),
        };
        let (_, reminders) = SessionMessageTool::new().format_forwarded_message("hello", &sender);
        assert!(reminders[0]
            .text
            .contains("[Executor] Worker (session source-3)"));
        assert!(!reminders[0].text.contains("From depth:"));
        assert!(reminders[0].text.contains("From agent: Worker"));
    }

    #[test]
    fn forwarded_reminder_always_identifies_session() {
        let sender = SenderIdentity {
            session_id: "source-4".to_string(),
            role: None,
            depth: None,
            name: None,
        };
        let (_, reminders) = SessionMessageTool::new().format_forwarded_message("hello", &sender);
        assert!(reminders[0].text.contains("[Agent] (session source-4)"));
        assert!(reminders[0].text.contains("From session: source-4"));
        assert!(reminders[0].text.contains("From role: Agent"));
        assert!(!reminders[0].text.contains("From depth:"));
        assert!(!reminders[0].text.contains("From agent:"));
    }

    #[test]
    fn role_display_title_cases_snake_case_keys() {
        assert_eq!(format_role_display("commander"), "Commander");
        assert_eq!(format_role_display("punishment_executor"), "PunishmentExecutor");
    }

    #[test]
    fn session_message_input_parses_batch_items() {
        let input: SessionMessageInput = serde_json::from_value(json!({
            "workspace": "C:/work",
            "batch": [
                {
                    "session_name": "Worker One",
                    "message": "hello one",
                    "agent_type": "agentic"
                },
                {
                    "session_id": "worker_2",
                    "message": "hello two",
                    "urgent": true
                }
            ]
        }))
        .expect("payload with batch must parse");

        let batch = input.batch.expect("batch must be present");
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].session_name.as_deref(), Some("Worker One"));
        assert_eq!(batch[0].message, "hello one");
        assert_eq!(batch[0].agent_type.as_ref().map(AgentType::as_str), Some("agentic"));
        assert!(batch[0].session_id.is_none());
        assert!(!batch[0].urgent);
        assert_eq!(batch[1].session_id.as_deref(), Some("worker_2"));
        assert!(batch[1].urgent);
        assert!(batch[1].session_name.is_none());
        assert!(batch[1].agent_type.is_none());
    }

    #[test]
    fn session_message_input_batch_defaults_to_none_for_backward_compat() {
        let input: SessionMessageInput = serde_json::from_value(json!({
            "session_id": "worker_1",
            "message": "hello",
        }))
        .expect("legacy payload without batch must parse");

        assert!(input.batch.is_none());
    }

    #[test]
    fn session_message_input_allows_omitting_top_level_message_for_batch() {
        let input: SessionMessageInput = serde_json::from_value(json!({
            "workspace": "C:/work",
            "batch": [
                {
                    "session_name": "Worker One",
                    "message": "hello",
                    "agent_type": "agentic"
                }
            ]
        }))
        .expect("batch payload without top-level message must parse");

        assert!(input.message.is_none());
        assert_eq!(input.batch.as_ref().expect("batch must be present").len(), 1);
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
        assert_eq!(validation.message.as_deref(), Some("batch cannot be empty"));
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
                            "session_name": "Worker One",
                            "message": "hello one",
                            "agent_type": "agentic"
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
                            "session_name": "Worker One",
                            "message": "hello one",
                            "agent_type": "agentic"
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
    async fn validate_batch_rejects_missing_workspace_for_create_item() {
        let tool = SessionMessageTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "batch": [
                        {
                            "session_name": "Worker One",
                            "message": "hello one",
                            "agent_type": "agentic"
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
    async fn validate_batch_rejects_item_missing_session_name() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "message": "hello one",
                            "agent_type": "agentic"
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
    async fn validate_batch_rejects_item_missing_agent_type() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_name": "Worker One",
                            "message": "hello one"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].agent_type is required when session_id is omitted")
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
                            "session_name": "Worker One",
                            "message": "   ",
                            "agent_type": "agentic"
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
    async fn validate_batch_rejects_self_session_item() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_id": "source_1",
                            "message": "hello one"
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
    async fn validate_batch_rejects_item_plan_without_todo() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_name": "Worker One",
                            "message": "hello one",
                            "agent_type": "agentic",
                            "plan_file": "my_plan_1234.plan.md"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].plan_file and batch[0].todo_id must be provided together")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_item_session_name_with_session_id() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_id": "worker_1",
                            "session_name": "Worker One",
                            "message": "hello one"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].session_name is only allowed when session_id is omitted")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_item_agent_type_with_session_id() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_id": "worker_1",
                            "message": "hello one",
                            "agent_type": "agentic"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].agent_type override is not allowed when session_id is provided")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_item_plan_binding_with_session_id() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_id": "worker_1",
                            "message": "hello one",
                            "plan_file": "my_plan_1234.plan.md",
                            "todo_id": "setup-auth"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].plan_file/todo_id binding is only allowed when session_id is omitted")
        );
    }

    #[tokio::test]
    async fn validate_batch_accepts_all_create_items() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_name": "Worker One",
                            "message": "hello one",
                            "agent_type": "agentic"
                        },
                        {
                            "session_name": "Worker Two",
                            "message": "hello two",
                            "agent_type": "Plan"
                        }
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
                            "session_id": "worker_1",
                            "message": "hello existing"
                        },
                        {
                            "session_name": "Worker Two",
                            "message": "hello new",
                            "agent_type": "agentic"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_batch_accepts_item_plan_todo_binding_and_urgent() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_name": "Worker One",
                            "message": "hello one",
                            "agent_type": "agentic",
                            "plan_file": "my_plan_1234.plan.md",
                            "todo_id": "setup-auth"
                        },
                        {
                            "session_id": "worker_1",
                            "message": "urgent hello",
                            "urgent": true
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[test]
    fn batch_summary_counts_success_and_failure() {
        let results = vec![
            json!({
                "status": "success",
                "target_session_id": "session-1",
                "created_session_id": "session-1",
            }),
            json!({
                "status": "error",
                "error": "session not found",
            }),
            json!({
                "status": "error",
                "error": "workspace mismatch",
            }),
        ];

        let (succeeded, failed, summary) = SessionMessageTool::summarize_batch_results(&results);

        assert_eq!(succeeded, 1);
        assert_eq!(failed, 2);
        assert!(summary.contains("3 message(s): 1 succeeded, 2 failed"));
        assert!(summary.contains("Successful items are not rolled back"));
        assert!(summary.contains("A failed item never rolls back earlier successes"));
    }

    #[test]
    fn batch_summary_omits_partial_failure_note_when_all_succeed() {
        let results = vec![
            json!({
                "status": "success",
                "target_session_id": "session-1",
            }),
            json!({
                "status": "success",
                "target_session_id": "session-2",
            }),
        ];

        let (succeeded, failed, summary) = SessionMessageTool::summarize_batch_results(&results);

        assert_eq!(succeeded, 2);
        assert_eq!(failed, 0);
        assert!(summary.contains("2 message(s): 2 succeeded, 0 failed"));
        assert!(!summary.contains("A failed item never rolls back"));
    }

    /// Minimal ACP port recording `send_message_to_bitfun_session` calls;
    /// the remaining trait methods are not exercised by these tests.
    #[derive(Debug, Default)]
    struct FakeAcpPort {
        bitfun_messages: Mutex<Vec<AcpClientBitfunMessageRequest>>,
        fail_send: bool,
    }

    impl RuntimeServicePort for FakeAcpPort {
        fn capability(&self) -> RuntimeServiceCapability {
            RuntimeServiceCapability::AcpClient
        }
    }

    #[async_trait]
    impl AcpClientPort for FakeAcpPort {
        async fn create_session(
            &self,
            _request: bitfun_runtime_ports::AcpClientCreateRequest,
        ) -> PortResult<bitfun_runtime_ports::AcpClientCreateResult> {
            Err(PortError::new(
                PortErrorKind::Backend,
                "not exercised by the ACP direct-path tests",
            ))
        }

        async fn list_clients(
            &self,
        ) -> PortResult<bitfun_runtime_ports::AcpClientListResult> {
            Err(PortError::new(
                PortErrorKind::Backend,
                "not exercised by the ACP direct-path tests",
            ))
        }

        async fn release_session(
            &self,
            _request: bitfun_runtime_ports::AcpClientReleaseRequest,
        ) -> PortResult<()> {
            Err(PortError::new(
                PortErrorKind::Backend,
                "not exercised by the ACP direct-path tests",
            ))
        }

        async fn cancel_session(
            &self,
            _request: bitfun_runtime_ports::AcpClientCancelRequest,
        ) -> PortResult<()> {
            Err(PortError::new(
                PortErrorKind::Backend,
                "not exercised by the ACP direct-path tests",
            ))
        }

        async fn send_message(
            &self,
            _request: bitfun_runtime_ports::AcpClientMessageRequest,
        ) -> PortResult<bitfun_runtime_ports::AcpClientMessageResult> {
            Err(PortError::new(
                PortErrorKind::Backend,
                "not exercised by the ACP direct-path tests",
            ))
        }

        async fn send_message_to_bitfun_session(
            &self,
            request: AcpClientBitfunMessageRequest,
        ) -> PortResult<bitfun_runtime_ports::AcpClientMessageResult> {
            if self.fail_send {
                return Err(PortError::new(
                    PortErrorKind::Backend,
                    "simulated external agent failure",
                ));
            }
            self.bitfun_messages.lock().unwrap().push(request.clone());
            Ok(bitfun_runtime_ports::AcpClientMessageResult {
                session_id: request.bitfun_session_id,
                response: "external response".to_string(),
            })
        }

        async fn read_history(
            &self,
            _request: bitfun_runtime_ports::AcpClientHistoryRequest,
        ) -> PortResult<bitfun_runtime_ports::AcpClientHistoryResult> {
            Err(PortError::new(
                PortErrorKind::Backend,
                "not exercised by the ACP direct-path tests",
            ))
        }
    }

    #[test]
    fn acp_client_id_is_extracted_from_agent_type_prefix() {
        assert_eq!(
            SessionMessageTool::acp_client_id_from_agent_type("acp__codex"),
            Some("codex")
        );
        assert_eq!(
            SessionMessageTool::acp_client_id_from_agent_type("acp__Claude Code"),
            Some("Claude Code")
        );
        assert_eq!(
            SessionMessageTool::acp_client_id_from_agent_type("agentic"),
            None
        );
        assert_eq!(SessionMessageTool::acp_client_id_from_agent_type("Plan"), None);
        // A flow session id (acp_<client>_<uuid>) is not an agent type prefix.
        assert_eq!(
            SessionMessageTool::acp_client_id_from_agent_type("acp_codex_abc123"),
            None
        );
        assert_eq!(SessionMessageTool::acp_client_id_from_agent_type(""), None);
        // A bare prefix with no client id is rejected (empty client id).
        assert_eq!(SessionMessageTool::acp_client_id_from_agent_type("acp__"), None);
    }

    #[tokio::test]
    async fn acp_direct_forwards_through_port_and_returns_response_verbatim() {
        let port = FakeAcpPort::default();
        let response = SessionMessageTool::dispatch_acp_direct(
            &port,
            "codex",
            "session-internal-1",
            "hello external agent",
            Some("/repo/project".to_string()),
        )
        .await
        .expect("direct path should succeed");

        let messages = port.bitfun_messages.lock().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].client_id, "codex");
        assert_eq!(messages[0].bitfun_session_id, "session-internal-1");
        assert_eq!(messages[0].message, "hello external agent");
        assert_eq!(messages[0].workspace_path.as_deref(), Some("/repo/project"));
        assert_eq!(messages[0].timeout_seconds, None);

        // The external response is returned verbatim, no re-translation.
        assert_eq!(response, "external response");
    }

    #[tokio::test]
    async fn acp_direct_propagates_port_failure() {
        let port = FakeAcpPort {
            fail_send: true,
            ..FakeAcpPort::default()
        };
        let error = SessionMessageTool::dispatch_acp_direct(
            &port,
            "codex",
            "session-internal-1",
            "hello",
            None,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("simulated external agent failure"));
    }
}
