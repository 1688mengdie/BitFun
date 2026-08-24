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
use crate::service::git::GitService;
use crate::service::workspace::{
    get_global_workspace_service, WorkspaceActivityMode, WorkspaceCreateOptions,
};
use crate::service::worktree::{
    WorktreeCreateBranchRequest, WorktreeCreateRequest, WorktreeService,
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
    session_control_session_name_or_default, validate_session_control_input, validate_session_id,
    SessionControlAction, SessionControlCancelRoute, SessionControlInput,
    SessionControlValidationContext, SessionControlValidationResult,
};
use bitfun_core_types::SessionExecutionTarget;
use bitfun_runtime_ports::{
    AgentSessionCreateRequest, AgentSessionDeleteRequest, AgentSessionListRequest,
    AgentSessionSummary, AgentSessionWorkspaceBinding, AgentSessionWorkspaceRequest,
    AgentSubmissionSource, AgentTurnCancellationRequest,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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

/// User-supplied options for creating a managed Git worktree alongside a session.
///
/// Mirrors the native `SessionExecutionTargetRequest::NewManagedWorktree` shape
/// (`base_ref` / `copy_local_changes`) without extending the core-types
/// contracts. Hosted on this shared module so both SessionControl and
/// SessionMessage consume a single definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct WorktreeSessionOptions {
    pub base_ref: Option<String>,
    pub copy_local_changes: bool,
}

/// Result of creating a managed worktree for a session (shared by W4/W5).
///
/// `created=false` means an idempotent replay reused an already-registered
/// worktree for the same request_id.
#[derive(Debug, Clone)]
pub(crate) struct SessionWorktreeCreateResult {
    pub execution_target: SessionExecutionTarget,
    pub tracked_workspace_id: Option<String>,
    pub created: bool,
    pub branch_name: Option<String>,
    pub project_workspace_path: String,
}

// ── Session↔worktree 联动共享核心（W4/W5/W8/W9）────────────────────
//
// 以下函数为文件级 `pub(crate)` free functions，SessionControl 与
// SessionMessage 两个工具共用（SessionMessage 经
// `use super::session_control_tool::...` 复用）。worktree 创建/命名/注册/
// 回滚全走上游原生 WorktreeService/GitService/track/remove（禁裸调 git）。

/// W9: remote SSH 互斥拒绝（worktree 不支持 remote workspace）。
pub(crate) fn ensure_worktree_not_remote(context: &ToolUseContext) -> BitFunResult<()> {
    if context.is_remote() {
        return Err(BitFunError::tool(
            "Managed worktrees are not supported for remote SSH workspaces yet".to_string(),
        ));
    }
    Ok(())
}

/// Parse the optional `worktree` object from a raw tool input value.
///
/// The worktree param is intentionally kept out of the shared
/// `SessionControlInput` contract so G2 stays confined to this module.
fn parse_worktree(input: &Value) -> Result<Option<WorktreeSessionOptions>, String> {
    match input.get("worktree") {
        None => Ok(None),
        Some(worktree_value) => serde_json::from_value(worktree_value.clone())
            .map(Some)
            .map_err(|error| format!("Invalid worktree options: {error}")),
    }
}

/// W8 自动命名：worktree 分支 `task/<序号>`（从既有 `task/*` 序号递增）。
/// 并发下由 WorktreeService 的仓库级锁 + receipt 幂等兜底（同一 request_id
/// 重放不会重复创建分支）。
async fn next_task_branch_name(project_workspace_path: &str) -> BitFunResult<String> {
    let branches = GitService::get_branches(project_workspace_path, false)
        .await
        .map_err(|error| BitFunError::tool(format!("Failed to list branches: {error}")))?;
    let max_task_index = branches
        .iter()
        .filter_map(|branch| {
            branch
                .name
                .strip_prefix("task/")
                .and_then(|suffix| suffix.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0);
    Ok(format!("task/{}", max_task_index + 1))
}

/// W8：把 `task/<序号>` 分支名清洗为合法 git 分支名（git check-ref-format
/// 规则 + 段级过滤 + 长度防御）。自动命名已保证合法，此处防御性清洗，
/// 不信任任何输入。
fn sanitize_task_branch_name(branch: &str) -> String {
    let sanitized: String = branch
        .split('/')
        .map(|segment| {
            segment
                .chars()
                .filter(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
                .collect::<String>()
        })
        .map(|segment| segment.trim_matches('.').to_string())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    if sanitized.is_empty() {
        "task/1".to_string()
    } else {
        sanitized
    }
}

/// 创建 managed worktree 并绑定到新会话（W4/W5 共享核心）。
///
/// 链路（对齐 WorktreeTool::create_session，禁裸调 git，一切走
/// WorktreeService）：
/// 1. `WorktreeService::create`——worktree 创建成功才继续；
/// 2. `track_workspace_activity`（workspace 注册）；
/// 3. 自动命名分支 `task/<序号>` 并 `create_branch`（worktree 绑定分支）；
/// 4. 返回 execution_target + tracked workspace id；任何一步失败回滚（remove +
///    注销），零孤儿。
pub(crate) async fn create_worktree_for_session(
    request_id: &str,
    project_workspace_path: &str,
    worktree_options: &WorktreeSessionOptions,
    context: &ToolUseContext,
) -> BitFunResult<SessionWorktreeCreateResult> {
    let source_workspace_path = context
        .workspace_root()
        .ok_or_else(|| BitFunError::tool("Current execution workspace is unavailable".to_string()))?
        .to_string_lossy()
        .to_string();
    let project_workspace_path = project_workspace_path.to_string();

    let created = WorktreeService::create(WorktreeCreateRequest {
        request_id: request_id.to_string(),
        project_workspace_path: project_workspace_path.clone(),
        source_workspace_path: Some(source_workspace_path),
        base_ref: worktree_options.base_ref.clone(),
        copy_local_changes: worktree_options.copy_local_changes,
        claimed_by: None,
    })
    .await
    .map_err(|error| BitFunError::tool(error.to_string()))?;

    let worktree_id = created
        .execution_target
        .worktree_id
        .clone()
        .ok_or_else(|| {
            BitFunError::tool("Created worktree is missing its worktree_id".to_string())
        })?;

    // track workspace（对齐 WorktreeTool::create_session 的 track 步骤）。
    // 失败即回滚新 worktree。
    let workspace_service = get_global_workspace_service()
        .ok_or_else(|| BitFunError::tool("Workspace service is not initialized".to_string()))?;
    let tracked_workspace = match workspace_service
        .track_workspace_activity(
            PathBuf::from(&created.execution_target.root_path),
            WorkspaceCreateOptions::default(),
            WorkspaceActivityMode::RefreshMetadata,
        )
        .await
    {
        Ok(workspace) => workspace,
        Err(track_error) => {
            return Err(cleanup_failed_worktree_create(
                &project_workspace_path,
                &created.execution_target,
                created.created,
                None,
                format!("Failed to register worktree workspace: {track_error}"),
            )
            .await);
        }
    };

    // 自动命名分支 task/<序号>（幂等重放 created=false 时可能已有分支，跳过）。
    let branch_name =
        sanitize_task_branch_name(&next_task_branch_name(&project_workspace_path).await?);
    if created.created && created.execution_target.branch.is_none() {
        let branch_request_id = format!("{request_id}:branch");
        if let Err(branch_error) = WorktreeService::create_branch(WorktreeCreateBranchRequest {
            request_id: branch_request_id,
            project_workspace_path: project_workspace_path.clone(),
            worktree_id: worktree_id.clone(),
            branch: branch_name.clone(),
        })
        .await
        {
            return Err(cleanup_failed_worktree_create(
                &project_workspace_path,
                &created.execution_target,
                created.created,
                Some(&tracked_workspace.id),
                format!("Failed to create worktree branch: {branch_error}"),
            )
            .await);
        }
    }

    Ok(SessionWorktreeCreateResult {
        execution_target: created.execution_target,
        tracked_workspace_id: Some(tracked_workspace.id.clone()),
        created: created.created,
        branch_name: Some(branch_name),
        project_workspace_path,
    })
}

/// 回滚刚创建的 worktree（W4/W5 失败路径）。
///
/// 对齐 WorktreeTool::cleanup_failed_fresh_create：注销 workspace +
/// `WorktreeService::rollback_created`。仅当本次确实创建了 worktree
/// （created=true）时回滚；幂等重放（created=false）不重复回滚。
async fn cleanup_failed_worktree_create(
    project_workspace_path: &str,
    execution_target: &SessionExecutionTarget,
    created: bool,
    tracked_workspace_id: Option<&str>,
    failure: impl Into<String>,
) -> BitFunError {
    let failure = failure.into();
    let mut rollback_issues = Vec::new();
    if let Some(workspace_id) = tracked_workspace_id {
        if let Some(workspace_service) = get_global_workspace_service() {
            if let Err(remove_error) = workspace_service.remove_workspace(workspace_id).await {
                rollback_issues.push(format!(
                    "workspace registration could not be removed: {remove_error}"
                ));
            }
        }
    }
    if created {
        if let Some(worktree_id) = execution_target.worktree_id.as_deref() {
            if let Err(rollback_error) =
                WorktreeService::rollback_created(project_workspace_path, worktree_id).await
            {
                rollback_issues.push(format!("worktree could not be removed: {rollback_error}"));
            }
        }
    }
    if rollback_issues.is_empty() {
        BitFunError::tool(failure)
    } else {
        BitFunError::tool(format!(
            "rollback_incomplete: {failure}; {}",
            rollback_issues.join("; ")
        ))
    }
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
- "list": List all sessions.

Arguments:
- "workspace": Absolute workspace path. Required for create and list. Ignored for cancel and delete.
- "session_name": Only used by create. Defaults to "New Session".
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
                "agent_type": {
                    "type": "string",
                    "enum": ["agentic", "Plan", "Cowork", "DeepResearch"],
                    "description": "Optional agent type when creating a session. Defaults to agentic."
                },
                "worktree": {
                    "type": "object",
                    "description": "Optional worktree options for create: creates a managed Git worktree together with the session and binds the session to it (only for create; not supported for remote workspaces). Shape: {baseRef?, copyLocalChanges?}.",
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

        let result = Self::into_validation_result(validate_session_control_input(
            &parsed,
            Self::validation_context(context),
        ));
        if !result.result {
            return result;
        }

        // G2: optional managed worktree validation. The worktree param is not part
        // of the shared SessionControlInput contract, so validate it here.
        match parse_worktree(input) {
            Err(message) => ValidationResult {
                result: false,
                message: Some(message),
                error_code: Some(400),
                meta: None,
            },
            Ok(Some(worktree)) => {
                if !matches!(parsed.action, SessionControlAction::Create) {
                    return ValidationResult {
                        result: false,
                        message: Some("worktree is only allowed for create".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }
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
                result
            }
            Ok(None) => result,
        }
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

                // G2: optional managed Git worktree. When the worktree param is present,
                // create the worktree first (WorktreeService chain) and bind the new
                // session to it. Failure = session not created + worktree rolled back,
                // zero orphans.
                let worktree_options: Option<WorktreeSessionOptions> =
                    serde_json::from_value(input.get("worktree").cloned().unwrap_or(Value::Null))
                        .map_err(|error| {
                        BitFunError::tool(format!("Invalid worktree options: {error}"))
                    })?;
                let mut created_worktree: Option<SessionWorktreeCreateResult> = None;
                if let Some(worktree_options) = worktree_options.as_ref() {
                    ensure_worktree_not_remote(context)?;
                    let request_id = context
                        .tool_call_id
                        .as_deref()
                        .map(|tool_call_id| format!("session-control:{tool_call_id}:worktree"))
                        .unwrap_or_else(|| {
                            format!("session-control:{}:worktree", uuid::Uuid::new_v4())
                        });
                    let worktree = create_worktree_for_session(
                        &request_id,
                        &workspace.project_workspace,
                        worktree_options,
                        context,
                    )
                    .await?;
                    created_worktree = Some(worktree);
                }

                let session = match runtime
                    .create_session(AgentSessionCreateRequest {
                        session_name,
                        agent_type,
                        workspace_path: Some(
                            created_worktree
                                .as_ref()
                                .map(|wt| wt.execution_target.root_path.clone())
                                .unwrap_or_else(|| workspace.display_workspace.clone()),
                        ),
                        project_workspace_path: Some(
                            created_worktree
                                .as_ref()
                                .map(|wt| wt.project_workspace_path.clone())
                                .unwrap_or_else(|| workspace.project_workspace.clone()),
                        ),
                        execution_target: created_worktree
                            .as_ref()
                            .map(|wt| wt.execution_target.clone())
                            .or_else(|| workspace.execution_target.clone()),
                        workspace_id: created_worktree
                            .as_ref()
                            .and_then(|wt| wt.tracked_workspace_id.clone())
                            .or_else(|| workspace.workspace_id.clone()),
                        remote_connection_id: workspace.remote_connection_id.clone(),
                        remote_ssh_host: workspace.remote_ssh_host.clone(),
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
                let created_session_id = session.session_id.clone();
                let created_session_name = session.session_name.clone();
                let created_agent_type = session.agent_type.clone();
                let result_for_assistant = session_control_created_result_message(
                    &created_session_id,
                    &workspace.display_workspace,
                    &created_agent_type,
                );

                let mut data = json!({
                    "success": true,
                    "action": "create",
                    "workspace": workspace.display_workspace.clone(),
                    "session": {
                        "session_id": created_session_id,
                        "session_name": created_session_name,
                        "agent_type": created_agent_type,
                    }
                });
                if let Some(worktree) = created_worktree.as_ref() {
                    data["worktree"] = json!({
                        "worktree_id": worktree.execution_target.worktree_id.clone(),
                        "path": worktree.execution_target.root_path.clone(),
                        "branch": worktree.branch_name.clone(),
                    });
                }

                Ok(vec![ToolResult::Result {
                    data,
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
}
