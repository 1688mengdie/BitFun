//! Snapshot Service API

use bitfun_core::infrastructure::{get_workspace_path, try_get_path_manager_arc};
use bitfun_core::service::snapshot::{
    ensure_global_snapshot_manager, initialize_global_snapshot_manager, OperationType,
    SnapshotConfig,
};
use log::warn;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInitRequest {
    pub workspace_path: String,
    pub config: Option<SnapshotConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordFileChangeRequest {
    pub session_id: String,
    pub turn_index: usize,
    pub file_path: String,
    pub operation_type: String, // "Create", "Modify", "Delete", "Rename"
    pub tool_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackSessionRequest {
    pub session_id: String,
    #[serde(default)]
    pub delete_session: bool, // Whether to also delete the session (default false)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackTurnRequest {
    pub session_id: String,
    pub turn_index: usize,
    #[serde(default)]
    pub delete_turns: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptFileRequest {
    pub session_id: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionFilesRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionTurnsRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTurnFilesRequest {
    pub session_id: String,
    pub turn_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFileDiffRequest {
    pub session_id: String,
    pub file_path: String,
    #[serde(default)]
    pub operation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBaselineSnapshotDiffRequest {
    #[serde(rename = "filePath")]
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOperationDiffRequest {
    pub sessionId: String,
    pub filePath: String,
    #[serde(default)]
    pub operationId: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOperationSummaryRequest {
    pub sessionId: String,
    pub operationId: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionStatsRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFileChangeHistoryRequest {
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAllModifiedFilesRequest {}

#[tauri::command]
pub async fn initialize_snapshot(
    app_handle: AppHandle,
    request: SnapshotInitRequest,
) -> Result<serde_json::Value, String> {
    let workspace_dir = PathBuf::from(&request.workspace_path);

    if !workspace_dir.exists() {
        return Err(format!(
            "Workspace directory does not exist: {}",
            request.workspace_path
        ));
    }

    initialize_global_snapshot_manager(workspace_dir, request.config)
        .await
        .map_err(|e| format!("Failed to initialize snapshot system: {}", e))?;

    let _ = app_handle.emit(
        "snapshot_initialized",
        serde_json::json!({
            "workspace_path": request.workspace_path,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }),
    );

    Ok(serde_json::json!({
        "success": true,
        "message": "Snapshot system initialized"
    }))
}

#[tauri::command]
pub async fn record_file_change(
    app_handle: AppHandle,
    request: RecordFileChangeRequest,
) -> Result<String, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let operation_type = match request.operation_type.as_str() {
        "Create" => OperationType::Create,
        "Modify" => OperationType::Modify,
        "Delete" => OperationType::Delete,
        "Rename" => OperationType::Rename,
        _ => {
            return Err(format!(
                "Unknown operation type: {}",
                request.operation_type
            ))
        }
    };

    let snapshot_id = manager
        .record_file_change(
            &request.session_id,
            request.turn_index,
            PathBuf::from(&request.file_path),
            operation_type,
            request.tool_name.clone(),
        )
        .await
        .map_err(|e| format!("Failed to record file change: {}", e))?;

    let _ = app_handle.emit(
        "file_change_recorded",
        serde_json::json!({
            "session_id": request.session_id,
            "turn_index": request.turn_index,
            "file_path": request.file_path,
            "snapshot_id": snapshot_id,
        }),
    );

    Ok(snapshot_id)
}

#[tauri::command]
pub async fn rollback_session(
    app_handle: AppHandle,
    request: RollbackSessionRequest,
) -> Result<Vec<String>, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let restored_files = manager
        .rollback_session(&request.session_id)
        .await
        .map_err(|e| format!("Failed to rollback session: {}", e))?;

    let restored_files_str: Vec<String> = restored_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let _ = app_handle.emit(
        "session_rolled_back",
        serde_json::json!({
            "session_id": request.session_id,
            "files_count": restored_files_str.len(),
            "session_deleted": request.delete_session,
        }),
    );

    Ok(restored_files_str)
}

#[tauri::command]
pub async fn rollback_to_turn(
    app_handle: AppHandle,
    request: RollbackTurnRequest,
) -> Result<Vec<String>, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let restored_files = manager
        .rollback_to_turn(&request.session_id, request.turn_index)
        .await
        .map_err(|e| format!("Failed to rollback turn: {}", e))?;

    let restored_files_str: Vec<String> = restored_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let mut deleted_turns_count = 0;
    if request.delete_turns {
        {
            use bitfun_core::agentic::coordination::get_global_coordinator;

            if let Some(coordinator) = get_global_coordinator() {
                if let Err(e) = coordinator
                    .get_session_manager()
                    .rollback_context_to_turn_start(&request.session_id, request.turn_index)
                    .await
                {
                    warn!(
                        "Rollback agentic context failed: session_id={}, turn_index={}, error={}",
                        request.session_id, request.turn_index, e
                    );
                }
            } else {
                warn!("Global coordinator not initialized, skipping agentic context rollback");
            }
        }

        use bitfun_core::service::conversation::persistence_manager::ConversationPersistenceManager;

        if let Some(workspace_path) = get_workspace_path() {
            match try_get_path_manager_arc() {
                Ok(path_manager) => {
                    match ConversationPersistenceManager::new(path_manager, workspace_path).await {
                        Ok(conversation_manager) => {
                            match conversation_manager
                                .delete_turns_from(&request.session_id, request.turn_index)
                                .await
                            {
                                Ok(count) => {
                                    deleted_turns_count = count;
                                }
                                Err(e) => {
                                    warn!("Failed to delete conversation turns: session_id={}, turn_index={}, error={}", request.session_id, request.turn_index, e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!(
                                "Failed to create ConversationPersistenceManager: error={}",
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to create PathManager: error={}", e);
                }
            }

            let _ = app_handle.emit(
                "conversation_turns_deleted",
                serde_json::json!({
                    "session_id": request.session_id,
                    "remaining_turns": request.turn_index,
                    "deleted_count": deleted_turns_count,
                }),
            );
        }
    }

    let _ = app_handle.emit(
        "turn_rolled_back",
        serde_json::json!({
            "session_id": request.session_id,
            "turn_index": request.turn_index,
            "files_count": restored_files_str.len(),
            "deleted_turns": request.delete_turns,
            "deleted_turns_count": deleted_turns_count,
        }),
    );

    Ok(restored_files_str)
}

#[tauri::command]
pub async fn accept_session(
    app_handle: AppHandle,
    request: AcceptSessionRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    manager
        .accept_session(&request.session_id)
        .await
        .map_err(|e| format!("Failed to accept session: {}", e))?;

    let _ = app_handle.emit(
        "session_accepted",
        serde_json::json!({
            "session_id": request.session_id,
        }),
    );

    Ok(serde_json::json!({
        "success": true,
        "message": "Session changes accepted"
    }))
}

#[tauri::command]
pub async fn accept_file(
    app_handle: AppHandle,
    request: AcceptFileRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    manager
        .accept_file(&request.session_id, &request.file_path)
        .await
        .map_err(|e| format!("Failed to accept file: {}", e))?;

    let _ = app_handle.emit(
        "file_accepted",
        serde_json::json!({
            "session_id": request.session_id,
            "file_path": request.file_path,
        }),
    );

    Ok(serde_json::json!({
        "success": true,
        "message": "File changes accepted"
    }))
}

#[tauri::command]
pub async fn get_session_files(request: GetSessionFilesRequest) -> Result<Vec<String>, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let files = manager
        .get_session_files(&request.session_id)
        .await
        .map_err(|e| format!("Failed to get session files: {}", e))?;

    Ok(files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
pub async fn get_session_turns(
    _app_handle: AppHandle,
    request: GetSessionTurnsRequest,
) -> Result<Vec<usize>, String> {
    use bitfun_core::service::conversation::ConversationPersistenceManager;

    if let Some(workspace_path) = get_workspace_path() {
        if let Ok(path_manager) = try_get_path_manager_arc() {
            match ConversationPersistenceManager::new(path_manager, workspace_path).await {
                Ok(conversation_manager) => {
                    match conversation_manager
                        .load_session_metadata(&request.session_id)
                        .await
                    {
                        Ok(Some(metadata)) => {
                            let turns: Vec<usize> = (0..metadata.turn_count).collect();
                            return Ok(turns);
                        }
                        Ok(None) => {}
                        Err(e) => {
                            warn!("Failed to load conversation metadata: session_id={}, error={}, falling back to snapshot", request.session_id, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to create ConversationPersistenceManager: error={}, falling back to snapshot", e);
                }
            }
        }
    }

    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let turns = manager
        .get_session_turns(&request.session_id)
        .await
        .map_err(|e| format!("Failed to get session turns: {}", e))?;

    Ok(turns)
}

#[tauri::command]
pub async fn get_turn_files(request: GetTurnFilesRequest) -> Result<Vec<String>, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let files = manager
        .get_turn_files(&request.session_id, request.turn_index)
        .await
        .map_err(|e| format!("Failed to get turn files: {}", e))?;

    Ok(files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
pub async fn get_file_diff(request: GetFileDiffRequest) -> Result<serde_json::Value, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let diff = manager
        .get_file_diff(
            &request.session_id,
            &request.file_path,
            request.operation_id.as_deref(),
        )
        .await
        .map_err(|e| format!("Failed to get file diff: {}", e))?;

    Ok(diff)
}

#[tauri::command]
pub async fn get_operation_diff(
    request: GetOperationDiffRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let diff = manager
        .get_file_diff(
            &request.sessionId,
            &request.filePath,
            request.operationId.as_deref(),
        )
        .await
        .map_err(|e| format!("Failed to get file diff: {}", e))?;

    Ok(serde_json::json!({
        "filePath": diff.get("file_path").and_then(|v| v.as_str()).unwrap_or(&request.filePath),
        "originalContent": diff.get("original_content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        "modifiedContent": diff.get("modified_content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        "anchorLine": diff.get("anchor_line").and_then(|v| v.as_u64()),
    }))
}

#[tauri::command]
pub async fn get_operation_summary(
    request: GetOperationSummaryRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let summary = manager
        .get_operation_summary(&request.sessionId, &request.operationId)
        .await
        .map_err(|e| format!("Failed to get operation summary: {}", e))?;

    Ok(serde_json::json!({
        "operationId": summary.get("operation_id").and_then(|v| v.as_str()).unwrap_or(&request.operationId),
        "sessionId": summary.get("session_id").and_then(|v| v.as_str()).unwrap_or(&request.sessionId),
        "turnIndex": summary.get("turn_index").and_then(|v| v.as_u64()),
        "seqInTurn": summary.get("seq_in_turn").and_then(|v| v.as_u64()),
        "filePath": summary.get("file_path").and_then(|v| v.as_str()),
        "operationType": summary.get("operation_type").and_then(|v| v.as_str()),
        "toolName": summary.get("tool_name").and_then(|v| v.as_str()),
        "linesAdded": summary.get("lines_added").and_then(|v| v.as_u64()),
        "linesRemoved": summary.get("lines_removed").and_then(|v| v.as_u64()),
    }))
}

#[tauri::command]
pub async fn get_session_stats(
    request: GetSessionStatsRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let stats = manager
        .get_session_stats(&request.session_id)
        .await
        .map_err(|e| format!("Failed to get session stats: {}", e))?;

    Ok(stats)
}

#[tauri::command]
pub async fn get_snapshot_system_stats() -> Result<serde_json::Value, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let stats = manager
        .get_system_stats()
        .await
        .map_err(|e| format!("Failed to get system stats: {}", e))?;

    Ok(stats)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupSnapshotDataRequest {
    #[serde(rename = "maxAgeDays")]
    pub max_age_days: u64,
}

#[tauri::command]
pub async fn get_snapshot_sessions() -> Result<Vec<String>, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    manager
        .list_sessions()
        .await
        .map_err(|e| format!("Failed to list snapshot sessions: {}", e))
}

#[tauri::command]
pub async fn cleanup_snapshot_data(
    request: CleanupSnapshotDataRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    manager
        .cleanup_snapshot_data(request.max_age_days)
        .await
        .map_err(|e| format!("Failed to cleanup snapshot data: {}", e))?;

    Ok(serde_json::json!({
        "success": true,
        "message": "Snapshot data cleanup completed",
        "keep_recent_days": request.max_age_days,
    }))
}

#[tauri::command]
pub async fn check_git_isolation() -> Result<serde_json::Value, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let is_isolated = manager
        .check_git_isolation()
        .await
        .map_err(|e| format!("Failed to check git isolation: {}", e))?;

    Ok(serde_json::json!({
        "git_isolated": is_isolated,
        "message": if is_isolated { "Git repository is safely isolated" } else { "Git isolation status abnormal" }
    }))
}

#[tauri::command]
pub async fn get_file_change_history(
    request: GetFileChangeHistoryRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let file_path = PathBuf::from(&request.file_path);
    let changes = manager
        .get_file_change_history(&file_path)
        .await
        .map_err(|e| format!("Failed to get file change history: {}", e))?;

    Ok(serde_json::to_value(changes).map_err(|e| format!("Serialization failed: {}", e))?)
}

#[tauri::command]
pub async fn get_all_modified_files(
    _request: GetAllModifiedFilesRequest,
) -> Result<Vec<String>, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let files = manager
        .get_all_modified_files()
        .await
        .map_err(|e| format!("Failed to get modified files: {}", e))?;

    Ok(files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
pub async fn get_baseline_snapshot_diff(
    request: GetBaselineSnapshotDiffRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_global_snapshot_manager()
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;

    let file_path = PathBuf::from(&request.file_path);

    let (baseline_content, current_content) = {
        let snapshot_service = manager.get_snapshot_service();
        let snapshot_service = snapshot_service.read().await;

        match snapshot_service
            .get_baseline_snapshot_diff(&file_path)
            .await
        {
            Ok(diff) => diff,
            Err(e) => {
                warn!(
                    "Failed to get baseline diff: file_path={}, error={}",
                    request.file_path, e
                );
                (String::new(), String::new())
            }
        }
    };

    Ok(serde_json::json!({
        "filePath": request.file_path,
        "originalContent": baseline_content,
        "modifiedContent": current_content,
    }))
}
