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
    compact_session_display_name, render_session_control_tool_use_message,
    resolve_session_control_cancel_route, session_control_agent_type_or_default,
    session_control_cancel_result_message, session_control_cancel_status,
    session_control_created_result_message, session_control_creator_marker,
    session_control_deleted_result_message, session_control_session_name_or_default,
    validate_session_control_input, validate_session_id, SessionControlAction,
    SessionControlCancelRoute, SessionControlInput, SessionControlValidationContext,
    SessionControlValidationResult,
};
use bitfun_core_types::SessionExecutionTarget;
use bitfun_runtime_ports::{
    AgentSessionCreateRequest, AgentSessionDeleteRequest, AgentSessionListRequest,
    AgentSessionSummary, AgentSessionWorkspaceBinding, AgentSessionWorkspaceRequest,
    AgentSubmissionSource, AgentTurnCancellationRequest,
};
use bitfun_services_core::session::merge_session_custom_metadata;
use bitfun_services_core::session::tree::SessionTreeManager;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// SessionControl tool - create, cancel, delete, or list persisted sessions
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
            SessionControlAction::Cancel | SessionControlAction::Delete => {
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

    fn validation_context(context: Option<&ToolUseContext>) -> SessionControlValidationContext<'_> {
        SessionControlValidationContext {
            current_session_id: context.and_then(|value| value.session_id.as_deref()),
            has_workspace_root: context.and_then(|value| value.workspace_root()).is_some(),
            short_name_max_chars: None,
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
        tree: Option<&SessionTreeManager>,
        short_names: &HashMap<String, Option<String>>,
        detail: bool,
    ) -> String {
        if sessions.is_empty() {
            return format!("No sessions found in workspace '{}'.", workspace);
        }

        let mut output_lines = vec![format!(
            "Found {} session(s) in workspace '{}'",
            sessions.len(),
            workspace
        )];
        output_lines.push(String::new());
        if let Some(current_session_id) = current_session_id {
            output_lines.push(format!("Note: '{}' is your session_id", current_session_id));
            output_lines.push(String::new());
        }

        if detail {
            // Full tree JSON view (legacy verbose output). The full `sessions`
            // array and parsed `tree` stay available in the `data` payload for
            // programmatic consumers.
            push_full_tree_section(&mut output_lines, sessions, tree);
        } else {
            // Compact tree text view (default).
            push_compact_tree_section(&mut output_lines, sessions, tree, short_names);
        }

        output_lines.join("\n")
    }
}

/// Append the full-tree JSON section to the assistant-facing `list` output.
fn push_full_tree_section(
    lines: &mut Vec<String>,
    sessions: &[AgentSessionSummary],
    tree: Option<&SessionTreeManager>,
) {
    lines.push("## Session Tree (JSON)".to_string());
    lines.push("```json".to_string());
    lines.push(build_session_tree_json(sessions, tree));
    lines.push("```".to_string());
}

/// Append the compact one-line tree section to the assistant-facing `list`
/// output.
fn push_compact_tree_section(
    lines: &mut Vec<String>,
    sessions: &[AgentSessionSummary],
    tree: Option<&SessionTreeManager>,
    short_names: &HashMap<String, Option<String>>,
) {
    lines.push("## Sessions (compact)".to_string());
    lines.push("format: [sessionId] agentType | status | display_state | name".to_string());
    lines.extend(build_compact_tree_lines(sessions, tree, short_names));
}

// ── SC-7 session display engines ─────────────────────────────────────
//
// These are pure, self-contained renderers used by the `list` action to turn
// the flat `AgentSessionSummary` array into a tree-shaped text (compact) or
// JSON (detail) view. They share the same grouping / orphan-rehang / sort
// rules so both engined outputs stay consistent.

/// Build a JSON tree structure from the flat session list.
/// Sessions are grouped by `parent_session_id` into a forest of root nodes.
fn build_session_tree_json(
    sessions: &[AgentSessionSummary],
    tree: Option<&SessionTreeManager>,
) -> String {
    build_session_tree_json_impl(sessions, tree)
}

/// Grouping result for a session forest: nodes keyed by their effective
/// parent, the trunk (root) nodes, and the ids whose parent chain was fully
/// filtered out of the current listing.
struct SessionForest<'a> {
    children_of_parent: HashMap<String, Vec<&'a AgentSessionSummary>>,
    trunk_nodes: Vec<&'a AgentSessionSummary>,
    detached_session_ids: HashSet<&'a str>,
}

impl<'a> SessionForest<'a> {
    /// Group a flat session list into a forest using each session's effective
    /// parent. A session whose direct parent is absent from the listing is
    /// re-hung onto the nearest surviving ancestor (orphan re-hang). Only
    /// sessions with no surviving ancestor at all become trunk (root) nodes;
    /// those whose parent chain was filtered out are flagged as detached.
    fn build(sessions: &'a [AgentSessionSummary], tree: Option<&SessionTreeManager>) -> Self {
        let present_ids: HashSet<&'a str> = sessions
            .iter()
            .map(|session| session.session_id.as_str())
            .collect();

        let nearest_surviving_ancestor = |session: &'a AgentSessionSummary| -> Option<String> {
            let mut cursor = session.parent_session_id.clone()?;
            loop {
                if present_ids.contains(cursor.as_str()) {
                    return Some(cursor);
                }
                cursor = tree.and_then(|manager| manager.get_parent(&cursor))?;
            }
        };

        let mut children_of_parent: HashMap<String, Vec<&'a AgentSessionSummary>> = HashMap::new();
        let mut trunk_nodes: Vec<&'a AgentSessionSummary> = Vec::new();
        let mut detached_session_ids: HashSet<&'a str> = HashSet::new();

        for session in sessions {
            if let Some(parent_id) = nearest_surviving_ancestor(session) {
                children_of_parent
                    .entry(parent_id)
                    .or_default()
                    .push(session);
            } else {
                if session.parent_session_id.is_some() {
                    detached_session_ids.insert(session.session_id.as_str());
                }
                trunk_nodes.push(session);
            }
        }

        Self {
            children_of_parent,
            trunk_nodes,
            detached_session_ids,
        }
    }
}

/// Build the JSON tree view of the session forest. Sessions are grouped by
/// their effective parent and rendered top-down, with a depth guard so a deep
/// tree cannot overflow the stack.
fn build_session_tree_json_impl(
    sessions: &[AgentSessionSummary],
    tree: Option<&SessionTreeManager>,
) -> String {
    let forest = SessionForest::build(sessions, tree);

    const MAX_SERIALIZE_LEVEL: usize = bitfun_core_types::session_tree::MAX_TREE_SERIALIZE_DEPTH;

    fn encode_session_node(
        session: &AgentSessionSummary,
        forest: &SessionForest,
        tree: Option<&SessionTreeManager>,
        level: usize,
    ) -> serde_json::Value {
        // Beyond the recursion guard the subtree is cut short; a flag lets the
        // reader tell a complete tree from a depth-capped one.
        let truncated = level >= MAX_SERIALIZE_LEVEL;
        let child_nodes = if truncated {
            Vec::new()
        } else {
            forest
                .children_of_parent
                .get(session.session_id.as_str())
                .map(|entries| {
                    let mut ordered = entries.clone();
                    ordered.sort_by_key(|entry| entry.created_at_ms);
                    ordered
                        .into_iter()
                        .map(|entry| encode_session_node(entry, forest, tree, level + 1))
                        .collect()
                })
                .unwrap_or_default()
        };

        let node_depth = tree
            .and_then(|manager| manager.get_depth(&session.session_id))
            .unwrap_or(0);
        let runtime_status = session
            .status
            .clone()
            .unwrap_or_else(|| "active".to_string());

        let mut node = serde_json::Map::new();
        node.insert("sessionId".to_string(), json!(session.session_id));
        node.insert("sessionName".to_string(), json!(session.session_name));
        node.insert("agentType".to_string(), json!(session.agent_type));
        node.insert("depth".to_string(), json!(node_depth));
        node.insert("status".to_string(), json!(runtime_status));
        // Surface the seven-state display projection so tree consumers can
        // render markers without re-deriving it from the runtime status.
        node.insert(
            "display_state".to_string(),
            json!(session
                .display_state
                .clone()
                .unwrap_or_else(|| runtime_status.clone())),
        );
        if forest
            .detached_session_ids
            .contains(session.session_id.as_str())
        {
            node.insert("orphaned".to_string(), json!(true));
        }
        if truncated {
            node.insert("truncated".to_string(), json!(true));
        }
        node.insert("children".to_string(), json!(child_nodes));
        serde_json::Value::Object(node)
    }

    let mut ranked_trunks = forest.trunk_nodes.clone();
    ranked_trunks.sort_by_key(|node| std::cmp::Reverse(node.created_at_ms));

    let forest_values: Vec<serde_json::Value> = ranked_trunks
        .iter()
        .map(|node| encode_session_node(node, &forest, tree, 0))
        .collect();

    serde_json::to_string_pretty(&forest_values).unwrap_or_else(|_| "[]".to_string())
}

/// Build the compact one-line-per-session text view of the session forest. The
/// tree shape mirrors [`build_session_tree_json_impl`] (same grouping, orphan
/// re-hang and sort orders); only the per-node rendering is text.
fn build_compact_tree_lines(
    sessions: &[AgentSessionSummary],
    tree: Option<&SessionTreeManager>,
    short_names: &HashMap<String, Option<String>>,
) -> Vec<String> {
    let forest = SessionForest::build(sessions, tree);

    fn render_compact_entry(
        session: &AgentSessionSummary,
        short_names: &HashMap<String, Option<String>>,
        forest: &SessionForest,
    ) -> String {
        let runtime_status = session
            .status
            .clone()
            .unwrap_or_else(|| "active".to_string());
        let display_state = session
            .display_state
            .clone()
            .unwrap_or_else(|| runtime_status.clone());
        let shown_name = compact_session_display_name(
            &session.session_name,
            short_names
                .get(&session.session_id)
                .and_then(Option::as_deref),
        );
        let detached_tag = if forest
            .detached_session_ids
            .contains(session.session_id.as_str())
        {
            " (orphaned)"
        } else {
            ""
        };
        format!(
            "- [{}] {} | {} | {} | {}{}",
            session.session_id,
            session.agent_type,
            runtime_status,
            display_state,
            shown_name,
            detached_tag
        )
    }

    fn append_visible_branch(
        session: &AgentSessionSummary,
        level: usize,
        forest: &SessionForest,
        short_names: &HashMap<String, Option<String>>,
        lines: &mut Vec<String>,
    ) {
        let pad = "  ".repeat(level);
        lines.push(format!(
            "{pad}{}",
            render_compact_entry(session, short_names, forest)
        ));
        if let Some(entries) = forest.children_of_parent.get(session.session_id.as_str()) {
            let mut ordered = entries.clone();
            ordered.sort_by_key(|entry| entry.created_at_ms);
            for child in ordered {
                append_visible_branch(child, level + 1, forest, short_names, lines);
            }
        }
    }

    let mut ranked_trunks = forest.trunk_nodes.clone();
    ranked_trunks.sort_by_key(|node| std::cmp::Reverse(node.created_at_ms));

    let mut lines = Vec::new();
    for root in ranked_trunks {
        append_visible_branch(root, 0, &forest, short_names, &mut lines);
    }
    lines
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
- "create": Create a new session. You may optionally provide session_name, short_name and agent_type.
- "cancel": Cancel the target session's currently running dialog turn. This does not delete the session or clear any queued messages that may still run later.
- "delete": Delete an existing session by session_id.
- "list": List all sessions. Sessions are displayed in a tree structure showing parent-child relationships (created via Task tool). By default the output is compact (sessionId | agentType | status | short name); pass "detail": true to expand the full session tree including full session names.

Arguments:
- "workspace": Absolute workspace path. Required for create and list. Ignored for cancel and delete.
- "session_name": Only used by create. Defaults to "New Session".
- "short_name": Only used by create. Optional compact display name (e.g. "assistant"); it becomes the name shown in the compact list output, keeping the model context small.
- "detail": Only used by list. When true, the full session tree with full session names is returned instead of the compact output. Defaults to false.
- "agent_type": Only used by create. Defaults to "agentic".
  - "agentic": Coding-focused agent for implementation, debugging, and code changes.
  - "Plan": Planning agent for clarifying requirements and producing an implementation plan before coding.
  - "Cowork": Collaborative agent for office-style work such as research, documentation, presentations, etc.
  - "DeepResearch": Research agent for systematic investigation and evidence-driven reports.
- "session_id": Required for cancel and delete."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Create, list, cancel, and delete persisted agent sessions.".to_string()
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
                    "enum": ["create", "cancel", "delete", "list"],
                    "description": "The session action to perform."
                },
                "workspace": {
                    "type": "string",
                    "description": "Required absolute workspace path for create and list. Ignored for cancel and delete."
                },
                "session_id": {
                    "type": "string",
                    "description": "Required for cancel and delete."
                },
                "session_name": {
                    "type": "string",
                    "description": "Optional display name when creating a session."
                },
                "short_name": {
                    "type": "string",
                    "description": "Optional compact display name when creating a session (used by compact list output)."
                },
                "detail": {
                    "type": "boolean",
                    "description": "When true, list returns the full session tree with full session names instead of the compact output."
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

                // Persist the optional compact short name directly into the
                // session `custom_metadata` (best-effort). The `create_session`
                // wire `request.metadata` is NOT persisted as-is (only
                // `created_by` is extracted), so the short name must be merged
                // into the stored `SessionMetadata` via the upstream util.
                if let Some(short_name) = params
                    .short_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                {
                    if let Err(e) = coordinator
                        .session_manager()
                        .update_session_metadata(
                            &PathBuf::from(&workspace.project_workspace),
                            &created_session_id,
                            |metadata| {
                                merge_session_custom_metadata(
                                    metadata,
                                    serde_json::json!({ "shortName": short_name }),
                                );
                            },
                        )
                        .await
                    {
                        log::warn!(
                            "SessionControl create: failed to persist short name for {}: {:?}",
                            created_session_id,
                            e
                        );
                    }
                }
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
            SessionControlAction::List => {
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::List,
                        None,
                        context,
                        &runtime,
                    )
                    .await?;
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

                // Filter out daemon sessions from the surfaced list.
                let sessions: Vec<_> = sessions.into_iter().filter(|s| !s.is_daemon).collect();

                // Resolve compact short names from persisted session metadata
                // (custom_metadata.shortName, written by create when a
                // short_name argument was provided). Best-effort: sessions
                // without metadata or without a shortName fall back to the
                // truncated full name in the compact output.
                let mut short_names: HashMap<String, Option<String>> = HashMap::new();
                let surfaced_session_ids: HashSet<&str> = sessions
                    .iter()
                    .map(|session| session.session_id.as_str())
                    .collect();
                let metadata_list = coordinator
                    .session_manager()
                    .persistence_manager()
                    .list_session_metadata_including_internal(&PathBuf::from(
                        &workspace.project_workspace,
                    ))
                    .await
                    // Batch-read failure degrades to "no short name" (best-effort),
                    // consistent with per-session fallback semantics.
                    .unwrap_or_default();
                for metadata in metadata_list {
                    // Keep only surfaced (non-daemon) sessions' short names so the
                    // output contract stays stable.
                    if !surfaced_session_ids.contains(metadata.session_id.as_str()) {
                        continue;
                    }
                    let short_name = metadata
                        .custom_metadata
                        .as_ref()
                        .and_then(|custom| custom.get("shortName"))
                        .and_then(|value| value.as_str())
                        .map(str::to_string);
                    short_names.insert(metadata.session_id, short_name);
                }

                let detail = params.detail.unwrap_or(false);
                let current_session_id =
                    self.current_workspace_session(context, &workspace.display_workspace);
                let result_for_assistant = self.build_list_result_for_assistant(
                    &workspace.display_workspace,
                    &sessions,
                    current_session_id,
                    Some(coordinator.session_tree().as_ref()),
                    &short_names,
                    detail,
                );

                // The full JSON tree is always built into `data.tree` so
                // programmatic consumers can read the tree shape regardless of
                // the `detail` text toggle.
                let tree_json =
                    build_session_tree_json(&sessions, Some(coordinator.session_tree().as_ref()));
                let tree_value: Value = serde_json::from_str(&tree_json).unwrap_or(Value::Null);

                // When detail=false, keep the machine-readable `data.sessions`
                // payload compact too: each session's `name` follows the same
                // rule as the compact list lines (short name wins, else full
                // name truncated). The full names stay available in the
                // detail=true payload.
                let data_sessions: Vec<AgentSessionSummary> = if detail {
                    sessions
                } else {
                    sessions
                        .iter()
                        .map(|session| AgentSessionSummary {
                            session_name: compact_session_display_name(
                                &session.session_name,
                                short_names
                                    .get(&session.session_id)
                                    .and_then(Option::as_deref),
                            ),
                            ..session.clone()
                        })
                        .collect()
                };

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "list",
                        "workspace": workspace.display_workspace.clone(),
                        "current_session_id": current_session_id,
                        "count": data_sessions.len(),
                        "sessions": data_sessions,
                        "tree": tree_value,
                        "short_names": short_names,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::persistence::PersistenceManager;
    use crate::agentic::tools::framework::ToolUseContext;
    use crate::agentic::WorkspaceBinding;
    use crate::infrastructure::PathManager;
    use bitfun_core_types::{
        SessionExecutionTarget, SessionExecutionTargetKind, WorktreeLifecycle,
    };
    use bitfun_services_core::session::SessionMetadata;
    use serde_json::json;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
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

    fn summary(
        session_id: &str,
        parent_session_id: Option<&str>,
        is_daemon: bool,
        created_at_ms: u64,
    ) -> AgentSessionSummary {
        AgentSessionSummary {
            session_id: session_id.to_string(),
            session_name: format!("Session {session_id}"),
            agent_type: "agentic".to_string(),
            model_id: None,
            reasoning_preset: None,
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            turn_count: 0,
            created_at_ms,
            last_active_at_ms: created_at_ms,
            parent_session_id: parent_session_id.map(ToOwned::to_owned),
            status: None,
            display_state: None,
            is_daemon,
        }
    }

    // --- short_name / detail / compact output ---

    fn temp_workspace() -> TestTempDir {
        TestTempDir::new("bitfun-session-control-tool-test")
    }

    async fn validate_tool_input(
        input: serde_json::Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        SessionControlTool::new()
            .validate_input(&input, context)
            .await
    }

    #[tokio::test]
    async fn validate_list_rejects_short_name() {
        let workspace = temp_workspace();
        let validation = validate_tool_input(
            json!({
                "action": "list",
                "workspace": workspace.as_string(),
                "short_name": "secretary",
            }),
            Some(&empty_context()),
        )
        .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("short_name is only allowed for create")
        );
    }

    #[tokio::test]
    async fn validate_list_allows_detail_flag() {
        let workspace = temp_workspace();
        let validation = validate_tool_input(
            json!({
                "action": "list",
                "workspace": workspace.as_string(),
                "detail": true,
            }),
            Some(&empty_context()),
        )
        .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_cancel_rejects_detail_flag() {
        let validation = validate_tool_input(
            json!({
                "action": "cancel",
                "session_id": "worker_1",
                "detail": true,
            }),
            Some(&empty_context()),
        )
        .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("detail is only allowed for list")
        );
    }

    #[tokio::test]
    async fn validate_create_allows_short_name() {
        let workspace = temp_workspace();
        let mut context = empty_context();
        context.session_id = Some("creator-1".to_string());
        let validation = validate_tool_input(
            json!({
                "action": "create",
                "workspace": workspace.as_string(),
                "short_name": "secretary-standing",
            }),
            Some(&context),
        )
        .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_create_rejects_detail_flag() {
        let workspace = temp_workspace();
        let mut context = empty_context();
        context.session_id = Some("creator-1".to_string());
        let validation = validate_tool_input(
            json!({
                "action": "create",
                "workspace": workspace.as_string(),
                "detail": true,
            }),
            Some(&context),
        )
        .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("detail is only allowed for list")
        );
    }

    #[test]
    fn compact_display_name_prefers_short_name_and_truncates() {
        let long_name = "task-description".repeat(10); // 150 chars
        assert_eq!(
            compact_session_display_name("abc", Some("秘书·常驻")),
            "秘书·常驻"
        );
        assert_eq!(compact_session_display_name("abc", Some("  ")), "abc");

        let truncated = compact_session_display_name(&long_name, None);
        assert!(truncated.ends_with("..."));
        assert_eq!(truncated.chars().count(), 60 + 3);

        assert_eq!(
            compact_session_display_name("short name", None),
            "short name"
        );
    }

    #[test]
    fn compact_list_uses_short_names_and_preserves_tree_indentation() {
        let tool = SessionControlTool::new();
        let sessions = vec![
            summary("root", None, false, 1),
            summary("child", Some("root"), false, 2),
        ];
        let mut short_names = HashMap::new();
        short_names.insert("root".to_string(), Some("秘书·常驻".to_string()));
        short_names.insert("child".to_string(), None);

        let output = tool.build_list_result_for_assistant(
            "/repo",
            &sessions,
            None,
            None,
            &short_names,
            false,
        );

        assert!(output.contains("[root] agentic | active | active | 秘书·常驻"));
        assert!(output.contains("  - [child] agentic | active | active | Session child"));
        assert!(output.contains("## Sessions (compact)"));
        assert!(!output.contains("## Session Tree (JSON)"));
    }

    #[test]
    fn compact_list_truncates_long_session_names_without_short_name() {
        let tool = SessionControlTool::new();
        let long_name = "派单提示词全文-".repeat(20); // 140 chars
        let mut root = summary("root", None, false, 1);
        root.session_name = long_name.clone();
        let sessions = vec![root];
        let short_names = HashMap::new();

        let output = tool.build_list_result_for_assistant(
            "/repo",
            &sessions,
            None,
            None,
            &short_names,
            false,
        );

        assert!(
            !output.contains(&long_name),
            "full session name must be omitted"
        );
        assert!(output.contains("..."));
        assert!(output.contains("[root] agentic | active | "));
    }

    #[test]
    fn detail_list_keeps_full_tree_json_output() {
        let tool = SessionControlTool::new();
        let sessions = vec![summary("root", None, false, 1)];
        let short_names = HashMap::new();

        let output = tool.build_list_result_for_assistant(
            "/repo",
            &sessions,
            None,
            None,
            &short_names,
            true,
        );

        assert!(output.contains("## Session Tree (JSON)"));
        assert!(output.contains("\"sessionName\": \"Session root\""));
        assert!(output.contains("\"sessionId\": \"root\""));
    }

    #[tokio::test]
    async fn short_name_round_trips_through_persisted_session_metadata() {
        // B-T-3: prove the create→persist→list link the SC-7 short name relies
        // on. A short name merged into a session's `custom_metadata` via
        // `merge_session_custom_metadata` is actually readable back through the
        // same persisted-metadata path the `list` action uses.
        let root = std::env::temp_dir().join(format!("bitfun-g3-shortname-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(root.join("sessions")).unwrap();
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(root.clone()));
        let persistence =
            Arc::new(PersistenceManager::new(path_manager).expect("persistence manager"));

        let ws_path = root.join("workspace");
        std::fs::create_dir_all(&ws_path).unwrap();

        let mut metadata = SessionMetadata::new(
            "session-1".to_string(),
            "Session".to_string(),
            "agentic".to_string(),
            "model".to_string(),
        );
        metadata.project_workspace_path = Some(ws_path.to_string_lossy().to_string());
        persistence
            .save_session_metadata(&ws_path, &metadata)
            .await
            .unwrap();

        persistence
            .update_session_metadata(&ws_path, "session-1", |metadata| {
                merge_session_custom_metadata(metadata, json!({ "shortName": "秘书·常驻" }));
            })
            .await
            .unwrap();

        let listed = persistence
            .list_session_metadata_including_internal(&ws_path)
            .await
            .unwrap();
        let found = listed
            .iter()
            .find(|m| m.session_id == "session-1")
            .expect("session should be listed");
        let short_name = found
            .custom_metadata
            .as_ref()
            .and_then(|custom| custom.get("shortName"))
            .and_then(|value| value.as_str());
        assert_eq!(short_name, Some("秘书·常驻"));

        let _ = std::fs::remove_dir_all(&root);
    }
}
