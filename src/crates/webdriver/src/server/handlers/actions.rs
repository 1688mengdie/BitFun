use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

use super::{ensure_session, run_script};
use crate::server::response::{WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct PerformActionsRequest {
    actions: Vec<Value>,
}

pub async fn perform(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<PerformActionsRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        session.action_state.pressed_keys.clear();
        session.action_state.pressed_buttons.clear();
    }
    run_script(
        state,
        &session_id,
        "(actions) => { window.__bitfunWd.performActions(actions); return null; }",
        vec![Value::Array(request.actions)],
        false,
    )
    .await?;
    Ok(WebDriverResponse::null())
}

pub async fn release(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id)?;
    session.action_state.pressed_keys.clear();
    session.action_state.pressed_buttons.clear();
    Ok(WebDriverResponse::null())
}
