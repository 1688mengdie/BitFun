//! Session persistence API

use crate::api::app_state::AppState;
use crate::runtime::{
    DesktopRuntimeContext, DesktopSessionApplicationError, DesktopSessionScopeRequest,
    UiSessionMetadataField,
};
use crate::startup_trace::DesktopStartupTrace;
use bitfun_agent_runtime::sdk::AgentSessionLineageSnapshot;
use bitfun_core::agentic::coordination::get_global_scheduler;
use bitfun_core::agentic::persistence::{SessionBranchResult, SessionMetadataPage};
use bitfun_core::service::remote_ssh::normalize_remote_workspace_path;
use bitfun_core::service::session::{
    DialogTurnData, SessionKind, SessionMetadata, SessionStatus, SessionTranscriptExport,
    SessionTranscriptExportOptions,
};
use bitfun_core::service::session_usage::SessionUsageReport;
use bitfun_core::service::workspace::WorkspaceKind;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tauri::State;

fn desktop_session_scope(
    workspace_path: String,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
) -> DesktopSessionScopeRequest {
    DesktopSessionScopeRequest {
        workspace_path,
        remote_connection_id,
        remote_ssh_host,
    }
}

fn desktop_session_error(error: DesktopSessionApplicationError) -> String {
    error.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPersistedSessionsRequest {
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    /// When true, hidden Subagent/Ephemeral sessions are included in the
    /// result (full conversation management).
    #[serde(default)]
    pub include_hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPersistedSessionsPageRequest {
    pub workspace_path: String,
    pub limit: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    /// When true, hidden Subagent/Ephemeral sessions are included in the page
    /// (full conversation management).
    #[serde(default)]
    pub include_hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionLineageRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadSessionTurnsRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSessionTurnRequest {
    pub turn_data: DialogTurnData,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSessionMetadataRequest {
    pub metadata: SessionMetadata,
    pub fields: Vec<UiSessionMetadataField>,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSessionTranscriptRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default = "default_tools")]
    pub tools: bool,
    #[serde(default)]
    pub tool_inputs: bool,
    #[serde(default)]
    pub thinking: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchReferenceableSessionsRequest {
    pub query: String,
    #[serde(default = "default_session_reference_search_limit")]
    pub limit: usize,
}

fn default_session_reference_search_limit() -> usize {
    30
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionReferenceCandidate {
    pub session_id: String,
    pub session_name: String,
    pub workspace_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    pub workspace_label: String,
    pub last_activity_at: u64,
}

fn default_tools() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletePersistedSessionRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchSessionActivityRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadPersistedSessionMetadataRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionUsageReportRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default = "default_include_hidden_subagents")]
    pub include_hidden_subagents: bool,
}

fn default_include_hidden_subagents() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkSessionRequest {
    pub source_session_id: String,
    pub source_turn_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

pub type ForkSessionResponse = SessionBranchResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveSessionRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnarchiveSessionRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveAllSessionsRequest {
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteAllArchivedSessionsRequest {
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[tauri::command]
pub async fn list_persisted_sessions(
    request: ListPersistedSessionsRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<Vec<SessionMetadata>, String> {
    runtime
        .session_application()
        .list_persisted_sessions_with_options(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.include_hidden,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to list persisted sessions: {}",
                desktop_session_error(error)
            )
        })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDeletedSessionIdsRequest {
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

/// List session ids recorded in the workspace deletion tombstone registry.
/// The frontend initialization path pulls this registry to guard against
/// ghost resurrection of deleted subagent sessions after a restart.
#[tauri::command]
pub async fn list_deleted_session_ids(
    request: ListDeletedSessionIdsRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<Vec<String>, String> {
    runtime
        .session_application()
        .list_deleted_session_ids(desktop_session_scope(
            request.workspace_path,
            request.remote_connection_id,
            request.remote_ssh_host,
        ))
        .await
        .map_err(|error| {
            format!(
                "Failed to list deleted session ids: {}",
                desktop_session_error(error)
            )
        })
}

/// Search lightweight persisted metadata across open local and SSH
/// workspaces. This deliberately never loads dialog turns or generates a
/// transcript; that work happens only when the selected message is dispatched.
#[tauri::command]
pub async fn search_referenceable_sessions(
    request: SearchReferenceableSessionsRequest,
    runtime: State<'_, DesktopRuntimeContext>,
    app_state: State<'_, AppState>,
) -> Result<Vec<SessionReferenceCandidate>, String> {
    let query = request.query.trim().to_lowercase();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let limit = request.limit.clamp(1, 30);
    let scheduler = get_global_scheduler();
    let mut workspaces = app_state.workspace_service.get_opened_workspaces().await;
    workspaces.sort_by_key(|workspace| std::cmp::Reverse(workspace.last_accessed));

    let mut candidates = Vec::new();
    for workspace in workspaces {
        let remote_connection_id = workspace.remote_ssh_connection_id().map(ToOwned::to_owned);
        let remote_ssh_host = workspace
            .metadata
            .get("sshHost")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let workspace_path = if workspace.workspace_kind == WorkspaceKind::Remote {
            normalize_remote_workspace_path(&workspace.root_path.to_string_lossy())
        } else {
            workspace.root_path.to_string_lossy().to_string()
        };
        let metadata = runtime
            .session_application()
            .list_persisted_sessions(desktop_session_scope(
                workspace_path.clone(),
                remote_connection_id.clone(),
                remote_ssh_host.clone(),
            ))
            .await
            .map_err(|error| {
                format!(
                    "Failed to list sessions for workspace {}: {}",
                    workspace.name,
                    desktop_session_error(error)
                )
            })?;

        for session in metadata {
            if session.status == SessionStatus::Archived
                || !matches!(session.session_kind, SessionKind::Standard)
                || scheduler.as_ref().is_some_and(|scheduler| {
                    scheduler.is_session_busy_or_queued(&session.session_id)
                })
                || !session.session_name.to_lowercase().contains(&query)
            {
                continue;
            }
            candidates.push(SessionReferenceCandidate {
                session_id: session.session_id,
                session_name: session.session_name,
                workspace_path: workspace_path.clone(),
                remote_connection_id: remote_connection_id.clone(),
                remote_ssh_host: remote_ssh_host.clone(),
                workspace_label: workspace.name.clone(),
                last_activity_at: session.last_active_at,
            });
        }
    }

    candidates.sort_by_key(|right| std::cmp::Reverse(right.last_activity_at));
    candidates.truncate(limit);
    Ok(candidates)
}

#[tauri::command]
pub async fn list_persisted_sessions_page(
    request: ListPersistedSessionsPageRequest,
    runtime: State<'_, DesktopRuntimeContext>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<SessionMetadataPage, String> {
    let trace_started = Instant::now();
    let result = runtime
        .session_application()
        .list_persisted_sessions_page_with_options(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.cursor.as_deref(),
            request.limit,
            request.include_hidden,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to list persisted session page: {}",
                desktop_session_error(error)
            )
        });
    startup_trace.record_tauri_command_elapsed("list_persisted_sessions_page", None, trace_started);
    result
}

#[tauri::command]
pub async fn get_session_lineage(
    request: GetSessionLineageRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<Option<AgentSessionLineageSnapshot>, String> {
    runtime
        .session_application()
        .get_session_lineage(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            &request.session_id,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to load session lineage: {}",
                desktop_session_error(error)
            )
        })
}

#[tauri::command]
pub async fn load_session_turns(
    request: LoadSessionTurnsRequest,
    runtime: State<'_, DesktopRuntimeContext>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<Vec<DialogTurnData>, String> {
    let trace_started = Instant::now();
    let trace_target = if request.limit.is_some() {
        "recent"
    } else {
        "full"
    };
    let result = runtime
        .session_application()
        .load_session_turns(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            &request.session_id,
            request.limit,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to load session turns: {}",
                desktop_session_error(error)
            )
        });
    startup_trace.record_tauri_command_elapsed(
        "load_session_turns",
        Some(trace_target),
        trace_started,
    );
    result
}

#[tauri::command]
pub async fn get_session_usage_report(
    request: GetSessionUsageReportRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<SessionUsageReport, String> {
    runtime
        .session_application()
        .generate_usage_report(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.session_id,
            request.include_hidden_subagents,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to generate session usage report: {}",
                desktop_session_error(error)
            )
        })
}

#[tauri::command]
pub async fn save_session_turn(
    request: SaveSessionTurnRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<(), String> {
    runtime
        .session_application()
        .save_session_turn(
            desktop_session_scope(
                request.workspace_path.clone(),
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            &request.turn_data,
        )
        .await
        .map_err(|error| format!("Failed to save session turn: {error}"))?;

    // Notify the auto-sync background task (debounced upload to relay)
    crate::api::remote_connect_api::notify_session_changed(
        &request.turn_data.session_id,
        &request.workspace_path,
    );
    Ok(())
}

#[tauri::command]
pub async fn save_session_metadata(
    request: SaveSessionMetadataRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<(), String> {
    runtime
        .session_application()
        .save_ui_metadata(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.metadata,
            request.fields,
        )
        .await
        .map_err(|error| match error {
            DesktopSessionApplicationError::Validation(message) => message,
            error => format!(
                "Failed to save session metadata: {}",
                desktop_session_error(error)
            ),
        })
}

#[tauri::command]
pub async fn export_session_transcript(
    request: ExportSessionTranscriptRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<SessionTranscriptExport, String> {
    runtime
        .session_application()
        .export_session_transcript(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            &request.session_id,
            &SessionTranscriptExportOptions {
                tools: request.tools,
                tool_inputs: request.tool_inputs,
                thinking: request.thinking,
                turns: request.turns,
            },
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to export session transcript: {}",
                desktop_session_error(error)
            )
        })
}

#[tauri::command]
pub async fn delete_persisted_session(
    request: DeletePersistedSessionRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<(), String> {
    // 单会话删除（L4-P2-E 确认合理）：归档会话按定义是顶层（archived
    // 会话不可运行、无活跃子树），单会话 delete_session 足够，无需
    // delete_session_tree 级联。前端 ArchivedSessionsConfig 删除单条归档
    // 走此命令；后端 tombstone 落盘 + 列表过滤兜底防重启复活。
    runtime
        .session_application()
        .delete_session(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.session_id,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to delete persisted session: {}",
                desktop_session_error(error)
            )
        })
}

#[tauri::command]
pub async fn touch_session_activity(
    request: TouchSessionActivityRequest,
    runtime: State<'_, DesktopRuntimeContext>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<(), String> {
    let trace_started = Instant::now();
    let result = runtime
        .session_application()
        .touch_session(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            &request.session_id,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to update session activity: {}",
                desktop_session_error(error)
            )
        });
    startup_trace.record_tauri_command_elapsed("touch_session_activity", None, trace_started);
    result
}

#[tauri::command]
pub async fn load_persisted_session_metadata(
    request: LoadPersistedSessionMetadataRequest,
    runtime: State<'_, DesktopRuntimeContext>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<Option<SessionMetadata>, String> {
    let trace_started = Instant::now();
    // Direct metadata lookups are used by persistence flows that must be able
    // to read hidden subagent sessions without list-level visibility filtering.
    let result = runtime
        .session_application()
        .load_session_metadata(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            &request.session_id,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to load persisted session metadata: {}",
                desktop_session_error(error)
            )
        });
    startup_trace.record_tauri_command_elapsed(
        "load_persisted_session_metadata",
        None,
        trace_started,
    );
    result
}

#[tauri::command]
pub async fn fork_session(
    request: ForkSessionRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<ForkSessionResponse, String> {
    runtime
        .session_application()
        .fork_session(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.source_session_id,
            request.source_turn_id,
        )
        .await
        .map_err(|error| format!("Failed to fork session: {}", desktop_session_error(error)))
}

#[tauri::command]
pub async fn archive_session(
    request: ArchiveSessionRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<(), String> {
    runtime
        .session_application()
        .set_session_archived(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.session_id,
            true,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to save session metadata: {}",
                desktop_session_error(error)
            )
        })
}

#[tauri::command]
pub async fn unarchive_session(
    request: UnarchiveSessionRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<(), String> {
    runtime
        .session_application()
        .set_session_archived(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.session_id,
            false,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to save session metadata: {}",
                desktop_session_error(error)
            )
        })
}

#[tauri::command]
pub async fn archive_all_sessions(
    request: ArchiveAllSessionsRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<u32, String> {
    let scope = desktop_session_scope(
        request.workspace_path,
        request.remote_connection_id,
        request.remote_ssh_host,
    );
    let sessions = runtime
        .session_application()
        .list_persisted_sessions(scope.clone())
        .await
        .map_err(|error| format!("Failed to list sessions: {}", desktop_session_error(error)))?;

    let mut archived_count: u32 = 0;

    for metadata in sessions {
        if metadata.status != SessionStatus::Archived
            && metadata.session_kind == SessionKind::Standard
        {
            runtime
                .session_application()
                .set_session_archived(scope.clone(), metadata.session_id, true)
                .await
                .map_err(|error| {
                    format!(
                        "Failed to save session metadata: {}",
                        desktop_session_error(error)
                    )
                })?;
            archived_count += 1;
        }
    }

    Ok(archived_count)
}

#[tauri::command]
pub async fn list_archived_sessions(
    request: ListPersistedSessionsRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<Vec<SessionMetadata>, String> {
    runtime
        .session_application()
        .list_archived_sessions(desktop_session_scope(
            request.workspace_path,
            request.remote_connection_id,
            request.remote_ssh_host,
        ))
        .await
        .map_err(|error| format!("Failed to list sessions: {}", desktop_session_error(error)))
}

#[tauri::command]
pub async fn delete_all_archived_sessions(
    request: DeleteAllArchivedSessionsRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<u32, String> {
    let scope = desktop_session_scope(
        request.workspace_path,
        request.remote_connection_id,
        request.remote_ssh_host,
    );
    let sessions = runtime
        .session_application()
        .list_archived_sessions(scope.clone())
        .await
        .map_err(|error| format!("Failed to list sessions: {}", desktop_session_error(error)))?;

    let mut deleted_count: u32 = 0;

    for metadata in sessions {
        // 归档会话按定义无活跃子树（L4-P2-E），逐个单会话删除而非
        // delete_session_tree 级联；任一删除失败即中止（全有或全无语义）。
        runtime
            .session_application()
            .delete_session(scope.clone(), metadata.session_id)
            .await
            .map_err(|error| {
                format!("Failed to delete session: {}", desktop_session_error(error))
            })?;
        deleted_count += 1;
    }

    Ok(deleted_count)
}

// ---------------------------------------------------------------------------
// Group chat commands (R-GC-12, P2-1: 11 commands unified naming)
// ---------------------------------------------------------------------------
// P0-2/P1-4: every command is a thin wrapper over the shared GroupChatTool
// pipeline (create_room_impl / join_room_impl / leave_room_impl /
// delete_room_impl / set_mode_impl / send_message_impl), so the UI path shares
// validation, back-index (S-38), dispatch routing, and error codes with the
// tool path — no parallel implementation.

use crate::api::remote_workspace_policy::GroupChatWorkspaceScope;
use bitfun_core::agentic::coordination::{get_global_coordinator, ConversationCoordinator};
use bitfun_core::agentic::session::session_store_port::CoreSessionStorePort;
use bitfun_core::agentic::tools::implementations::group_chat_tool::{
    parse_group_chat_error_code, GroupChatTool,
};
use bitfun_core::service::session::GroupChatStore;
use bitfun_core::util::errors::BitFunError;
use bitfun_runtime_ports::{
    GroupChatActor, GroupChatError, GroupChatErrorCode, GroupChatMember, GroupChatMessagesResponse,
    GroupChatMode, GroupChatRoom, GroupChatSendResult, SessionStoragePathRequest, SessionStorePort,
};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Request DTOs (E-1: flat Tauri args → nested `{ request: Dto }` so serde
// owns snake_case deserialization — mirrors the GetSessionLineageRequest /
// LoadSessionTurnsRequest pattern above; the flat-arg camelCase problem
// disappears because the frontend now passes the whole nested object).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupChatListRequest {
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupChatLoadRequest {
    pub workspace_path: String,
    pub room_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupChatMembersRequest {
    pub workspace_path: String,
    pub room_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupChatCreateRequest {
    pub workspace_path: String,
    pub name: String,
    pub owner: GroupChatActor,
    pub members: Vec<String>,
    #[serde(default)]
    pub mode: Option<GroupChatMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupChatJoinRequest {
    pub workspace_path: String,
    pub room_id: String,
    pub session_id: String,
    pub actor: GroupChatActor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupChatLeaveRequest {
    pub workspace_path: String,
    pub room_id: String,
    pub session_id: String,
    pub actor: GroupChatActor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupChatDeleteRequest {
    pub workspace_path: String,
    pub room_id: String,
    pub actor: GroupChatActor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupChatSetModeRequest {
    pub workspace_path: String,
    pub room_id: String,
    pub mode: GroupChatMode,
    pub actor: GroupChatActor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupChatSendRequest {
    pub workspace_path: String,
    pub room_id: String,
    pub author: GroupChatActor,
    pub content: String,
    pub mention_targets: Vec<GroupChatActor>,
    pub urgent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupChatMessagesRequest {
    pub workspace_path: String,
    pub room_id: String,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub cursor: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupChatIngestReplyRequest {
    pub workspace_path: String,
    pub room_id: String,
    pub message_id: String,
    pub reply_content: String,
    pub author: GroupChatActor,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupChatScanTimeoutsRequest {
    pub workspace_path: String,
    pub reply_timeout_secs: u64,
    #[serde(default)]
    pub room_id: Option<String>,
}

/// Resolves the group-chats root (sibling of the sessions root) for a workspace.
/// `scope` carries the real `remote_connection_id`/`remote_ssh_host` (F-3: the
/// 12 group-chat commands declare `RemoteRouted`; a hardcoded `None` connection
/// id made remote workspaces opened at the same path on different hosts
/// resolve against the wrong — or `_unresolved` — mirror tree).
async fn group_chats_root(scope: &GroupChatWorkspaceScope) -> Result<std::path::PathBuf, String> {
    let request = SessionStoragePathRequest {
        workspace_path: std::path::PathBuf::from(&scope.workspace_path),
        remote_connection_id: scope.remote_connection_id.clone(),
        remote_ssh_host: scope.remote_ssh_host.clone(),
    };
    let resolution = CoreSessionStorePort::default()
        .resolve_session_storage_path(request)
        .await
        .map_err(|error| format!("Failed to resolve sessions root: {error}"))?;
    let sessions_root = resolution.effective_storage_path;
    let parent = sessions_root
        .parent()
        .ok_or_else(|| "sessions root has no parent directory".to_string())?;
    Ok(parent.join("group-chats"))
}

async fn group_chat_store(scope: &GroupChatWorkspaceScope) -> Result<GroupChatStore, String> {
    let root = group_chats_root(scope).await?;
    Ok(GroupChatStore::new(root))
}

/// Converts a tool error into a structured `GroupChatError` so the frontend
/// can branch on the contract error code (P1-5). Legacy plain errors degrade
/// to a generic message with no code.
fn group_chat_command_error(error: BitFunError) -> GroupChatError {
    let message = error.to_string();
    let code = parse_group_chat_error_code(&message).unwrap_or(GroupChatErrorCode::NotFound);
    GroupChatError { code, message }
}

/// Resolves the global coordinator (the shared pipeline entry). Falls back to
/// an error string when the coordinator is not initialized.
fn require_coordinator() -> Result<Arc<ConversationCoordinator>, GroupChatError> {
    get_global_coordinator().ok_or_else(|| GroupChatError {
        code: GroupChatErrorCode::NotFound,
        message: "coordinator not initialized".to_string(),
    })
}

#[tauri::command]
pub async fn group_chat_list(
    request: GroupChatListRequest,
) -> Result<Vec<GroupChatRoom>, GroupChatError> {
    let scope = GroupChatWorkspaceScope::new(request.workspace_path.clone())
        .resolve()
        .await;
    let store = group_chat_store(&scope)
        .await
        .map_err(|message| GroupChatError {
            code: GroupChatErrorCode::NotFound,
            message,
        })?;
    let (rooms, _) = store.list_rooms().await.map_err(|error| GroupChatError {
        code: group_chat_store_error_code(&error),
        message: error.to_string(),
    })?;
    Ok(rooms)
}

#[tauri::command]
pub async fn group_chat_load(
    request: GroupChatLoadRequest,
) -> Result<GroupChatRoom, GroupChatError> {
    let scope = GroupChatWorkspaceScope::new(request.workspace_path.clone())
        .resolve()
        .await;
    let store = group_chat_store(&scope)
        .await
        .map_err(|message| GroupChatError {
            code: GroupChatErrorCode::NotFound,
            message,
        })?;
    store
        .load_room(&request.room_id)
        .await
        .map_err(|error| GroupChatError {
            code: group_chat_store_error_code(&error),
            message: error.to_string(),
        })
}

#[tauri::command]
pub async fn group_chat_members(
    request: GroupChatMembersRequest,
) -> Result<Vec<GroupChatMember>, GroupChatError> {
    let scope = GroupChatWorkspaceScope::new(request.workspace_path.clone())
        .resolve()
        .await;
    let store = group_chat_store(&scope)
        .await
        .map_err(|message| GroupChatError {
            code: GroupChatErrorCode::NotFound,
            message,
        })?;
    store
        .list_members(&request.room_id)
        .await
        .map_err(|error| GroupChatError {
            code: group_chat_store_error_code(&error),
            message: error.to_string(),
        })
}

#[tauri::command]
pub async fn group_chat_create(
    request: GroupChatCreateRequest,
) -> Result<GroupChatRoom, GroupChatError> {
    if request.workspace_path.trim().is_empty() {
        return Err(GroupChatError {
            code: GroupChatErrorCode::NotFound,
            message: "group_chat_create: workspace_path is empty".to_string(),
        });
    }
    let scope = GroupChatWorkspaceScope::new(request.workspace_path.clone())
        .resolve()
        .await;
    let coordinator = require_coordinator()?;
    GroupChatTool::create_room_impl(
        &coordinator,
        &scope.workspace_path,
        &request.name,
        request.owner,
        &request.members,
        request.mode.unwrap_or(GroupChatMode::Free),
    )
    .await
    .map_err(group_chat_command_error)
}

#[tauri::command]
pub async fn group_chat_join(
    request: GroupChatJoinRequest,
) -> Result<GroupChatRoom, GroupChatError> {
    let scope = GroupChatWorkspaceScope::new(request.workspace_path.clone())
        .resolve()
        .await;
    let coordinator = require_coordinator()?;
    GroupChatTool::join_room_impl(
        &coordinator,
        &scope.workspace_path,
        &request.room_id,
        &request.session_id,
        request.actor,
    )
    .await
    .map_err(group_chat_command_error)
}

#[tauri::command]
pub async fn group_chat_leave(
    request: GroupChatLeaveRequest,
) -> Result<GroupChatRoom, GroupChatError> {
    let scope = GroupChatWorkspaceScope::new(request.workspace_path.clone())
        .resolve()
        .await;
    let coordinator = require_coordinator()?;
    GroupChatTool::leave_room_impl(
        &coordinator,
        &scope.workspace_path,
        &request.room_id,
        &request.session_id,
        request.actor,
    )
    .await
    .map_err(group_chat_command_error)
}

#[tauri::command]
pub async fn group_chat_delete(
    request: GroupChatDeleteRequest,
) -> Result<(), GroupChatError> {
    let scope = GroupChatWorkspaceScope::new(request.workspace_path.clone())
        .resolve()
        .await;
    let coordinator = require_coordinator()?;
    GroupChatTool::delete_room_impl(&coordinator, &scope.workspace_path, &request.room_id, request.actor)
        .await
        .map_err(group_chat_command_error)
}

#[tauri::command]
pub async fn group_chat_set_mode(
    request: GroupChatSetModeRequest,
) -> Result<GroupChatRoom, GroupChatError> {
    let scope = GroupChatWorkspaceScope::new(request.workspace_path.clone())
        .resolve()
        .await;
    GroupChatTool::set_mode_impl(&scope.workspace_path, &request.room_id, request.mode, request.actor)
        .await
        .map_err(group_chat_command_error)
}

#[tauri::command]
pub async fn group_chat_send(
    request: GroupChatSendRequest,
) -> Result<GroupChatSendResult, GroupChatError> {
    let scope = GroupChatWorkspaceScope::new(request.workspace_path.clone())
        .resolve()
        .await;
    let coordinator = require_coordinator()?;
    let (message_id, delivered_to, failed_to) = GroupChatTool::send_message_impl(
        &coordinator,
        &scope.workspace_path,
        &request.room_id,
        &request.author,
        &request.content,
        &request.mention_targets,
        request.urgent,
    )
    .await
    .map_err(group_chat_command_error)?;
    let failed_to = failed_to
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect();
    Ok(GroupChatSendResult {
        message_id,
        delivered_to,
        failed_to,
    })
}

#[tauri::command]
pub async fn group_chat_messages(
    request: GroupChatMessagesRequest,
) -> Result<GroupChatMessagesResponse, GroupChatError> {
    let scope = GroupChatWorkspaceScope::new(request.workspace_path.clone())
        .resolve()
        .await;
    let store = group_chat_store(&scope)
        .await
        .map_err(|message| GroupChatError {
            code: GroupChatErrorCode::NotFound,
            message,
        })?;
    let window = store
        .list_messages(&request.room_id, request.limit, request.cursor)
        .await
        .map_err(|error| GroupChatError {
            code: group_chat_store_error_code(&error),
            message: error.to_string(),
        })?;
    // P2-6: no string bridge — the contract and the store share the usize
    // cursor domain (next page start index).
    Ok(GroupChatMessagesResponse {
        messages: window.messages,
        next_cursor: window.next_cursor,
    })
}

#[tauri::command]
pub async fn group_chat_ingest_reply(
    request: GroupChatIngestReplyRequest,
) -> Result<(), GroupChatError> {
    // F-2 convergence: thin wrapper over the authority router core so the
    // command layer, the GroupChatPort adapter, and the reply router share
    // one behavior (P2-2 no-op on deleted message + deterministic reply id).
    let scope = GroupChatWorkspaceScope::new(request.workspace_path.clone())
        .resolve()
        .await;
    let store = group_chat_store(&scope)
        .await
        .map_err(|message| GroupChatError {
            code: GroupChatErrorCode::NotFound,
            message,
        })?;
    bitfun_core::agentic::tools::implementations::group_chat_router::ingest_reply_core(
        &store,
        &request.room_id,
        &request.message_id,
        &request.reply_content,
        &request.author,
        request.timestamp,
    )
    .await
    .map_err(|error| {
        let text = error.to_string();
        GroupChatError {
            code: parse_group_chat_error_code(&text).unwrap_or(GroupChatErrorCode::NotFound),
            message: text,
        }
    })
}

/// Maps a store error to the closest contract error code (P1-5).
fn group_chat_store_error_code(
    error: &bitfun_core::service::session::GroupChatStoreError,
) -> GroupChatErrorCode {
    use bitfun_core::service::session::GroupChatStoreError;
    match error {
        GroupChatStoreError::RoomNotFound(_) => GroupChatErrorCode::NotFound,
        GroupChatStoreError::MessageNotFound(_) => GroupChatErrorCode::NotFound,
        _ => GroupChatErrorCode::NotFound,
    }
}

/// 超时提醒消费端（P2-3/P2-4）：只扫描 `room_id`（传入时）或全表（None），
/// 消费 group_chat.reply_timeout_secs（R-GC-26），返回超时提醒列表。
/// room_id 参数让每个 Pane 只扫自己的房间，避免 N 个 Pane = N 倍全表 IO。
#[tauri::command]
pub async fn group_chat_scan_timeouts(
    request: GroupChatScanTimeoutsRequest,
) -> Result<Vec<serde_json::Value>, String> {
    let scope = GroupChatWorkspaceScope::new(request.workspace_path.clone())
        .resolve()
        .await;
    let store = group_chat_store(&scope).await?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let mut reminders = Vec::new();
    let rooms = match &request.room_id {
        Some(room_id) => {
            let room = store.load_room(room_id).await.map_err(|e| e.to_string())?;
            vec![room]
        }
        None => {
            let (rooms, _) = store.list_rooms().await.map_err(|e| e.to_string())?;
            rooms
        }
    };
    for room in rooms {
        let timed_out = store
            .scan_timed_out_messages(&room.room_id, request.reply_timeout_secs, now)
            .await
            .map_err(|error| error.to_string())?;
        for message in timed_out {
            reminders.push(serde_json::json!({
                "roomId": room.room_id,
                "messageId": message.message_id,
                "content": message.content,
                "status": "failed",
            }));
        }
    }
    Ok(reminders)
}
