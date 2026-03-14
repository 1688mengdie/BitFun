//! BTW (side question) API
//!
//! Desktop adapter for the core side-question service:
//! - Reads current session context without mutating the parent session
//! - Streams answer via `btw://...` events
//! - Supports cancellation by request id

use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::api::app_state::AppState;

use bitfun_core::agentic::coordination::ConversationCoordinator;
use bitfun_core::agentic::side_question::{
    SideQuestionPersistTarget, SideQuestionService, SideQuestionStreamEvent,
    SideQuestionStreamRequest,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BtwAskRequest {
    pub session_id: String,
    pub question: String,
    /// Optional model id override. Supports "fast"/"primary" aliases.
    pub model_id: Option<String>,
    /// Limit how many context messages are included (from the end).
    pub max_context_messages: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BtwAskResponse {
    pub answer: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BtwAskStreamRequest {
    pub request_id: String,
    pub session_id: String,
    pub question: String,
    /// Optional model id override. Supports "fast"/"primary" aliases.
    pub model_id: Option<String>,
    /// Limit how many context messages are included (from the end).
    pub max_context_messages: Option<usize>,
    pub child_session_id: Option<String>,
    pub workspace_path: Option<String>,
    pub parent_dialog_turn_id: Option<String>,
    pub parent_turn_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BtwAskStreamResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BtwCancelRequest {
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BtwTextChunkEvent {
    pub request_id: String,
    pub session_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BtwCompletedEvent {
    pub request_id: String,
    pub session_id: String,
    pub full_text: String,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BtwErrorEvent {
    pub request_id: String,
    pub session_id: String,
    pub error: String,
}

fn side_question_service(
    state: &AppState,
    coordinator: Arc<ConversationCoordinator>,
) -> SideQuestionService {
    SideQuestionService::new(
        coordinator,
        state.ai_client_factory.clone(),
        state.side_question_runtime.clone(),
    )
}

#[tauri::command]
pub async fn btw_cancel(
    state: State<'_, AppState>,
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: BtwCancelRequest,
) -> Result<(), String> {
    if request.request_id.trim().is_empty() {
        return Err("requestId is required".to_string());
    }

    let svc = side_question_service(&state, coordinator.inner().clone());
    svc.cancel(&request.request_id).await;
    Ok(())
}

#[tauri::command]
pub async fn btw_ask_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: BtwAskStreamRequest,
) -> Result<BtwAskStreamResponse, String> {
    if request.request_id.trim().is_empty() {
        return Err("requestId is required".to_string());
    }
    if request.session_id.trim().is_empty() {
        return Err("sessionId is required".to_string());
    }
    if request.question.trim().is_empty() {
        return Err("question is required".to_string());
    }

    let svc = side_question_service(&state, coordinator.inner().clone());

    let rx = svc
        .start_stream(SideQuestionStreamRequest {
            request_id: request.request_id.clone(),
            session_id: request.session_id.clone(),
            question: request.question.clone(),
            model_id: request.model_id.clone(),
            max_context_messages: request.max_context_messages,
            persist_target: match (&request.child_session_id, &request.workspace_path) {
                (Some(child_session_id), Some(workspace_path))
                    if !child_session_id.trim().is_empty() && !workspace_path.trim().is_empty() =>
                {
                    Some(SideQuestionPersistTarget {
                        child_session_id: child_session_id.clone(),
                        workspace_path: PathBuf::from(workspace_path),
                        parent_session_id: request.session_id.clone(),
                        parent_dialog_turn_id: request.parent_dialog_turn_id.clone(),
                        parent_turn_index: request.parent_turn_index,
                    })
                }
                _ => None,
            },
        })
        .await
        .map_err(|e| e.to_string())?;

    let app_handle = app.clone();
    tokio::spawn(async move {
        let mut rx = rx;
        while let Some(evt) = rx.recv().await {
            match evt {
                SideQuestionStreamEvent::TextChunk {
                    request_id,
                    session_id,
                    text,
                } => {
                    let payload = BtwTextChunkEvent {
                        request_id,
                        session_id,
                        text,
                    };
                    if let Err(e) = app_handle.emit("btw://text-chunk", payload) {
                        warn!("Failed to emit btw text chunk: {}", e);
                    }
                }
                SideQuestionStreamEvent::Completed {
                    request_id,
                    session_id,
                    full_text,
                    finish_reason,
                } => {
                    let payload = BtwCompletedEvent {
                        request_id,
                        session_id,
                        full_text,
                        finish_reason,
                    };
                    if let Err(e) = app_handle.emit("btw://completed", payload) {
                        warn!("Failed to emit btw completed: {}", e);
                    }
                }
                SideQuestionStreamEvent::Error {
                    request_id,
                    session_id,
                    error: err,
                } => {
                    let payload = BtwErrorEvent {
                        request_id,
                        session_id,
                        error: err,
                    };
                    if let Err(e) = app_handle.emit("btw://error", payload) {
                        warn!("Failed to emit btw error: {}", e);
                    }
                }
            }
        }
    });

    Ok(BtwAskStreamResponse { ok: true })
}

#[tauri::command]
pub async fn btw_ask(
    state: State<'_, AppState>,
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: BtwAskRequest,
) -> Result<BtwAskResponse, String> {
    let svc = side_question_service(&state, coordinator.inner().clone());

    let answer = svc
        .ask(
            &request.session_id,
            &request.question,
            request.model_id.as_deref(),
            request.max_context_messages,
        )
        .await
        .map_err(|e| {
            error!("BTW ask failed: {}", e);
            e.to_string()
        })?;

    info!(
        "BTW ask completed: session_id={}, answer_len={}",
        request.session_id,
        answer.len()
    );

    Ok(BtwAskResponse { answer })
}
