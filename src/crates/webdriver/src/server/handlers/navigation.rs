use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use tauri::Manager;

use super::{ensure_session, get_session, run_script};
use crate::server::response::{WebDriverErrorResponse, WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct UrlRequest {
    url: String,
}

pub async fn get_url(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let session = get_session(&state, &session_id).await?;
    let webview = state
        .app
        .get_webview(&session.current_window)
        .ok_or_else(|| {
            WebDriverErrorResponse::no_such_window(format!(
                "Webview not found: {}",
                session.current_window
            ))
        })?;

    let url = webview.url().map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to read URL: {error}"))
    })?;

    Ok(WebDriverResponse::success(url.to_string()))
}

pub async fn navigate(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<UrlRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        session.action_state = Default::default();
    }
    run_script(
        state,
        &session_id,
        "(url) => { window.location.href = url; return null; }",
        vec![request.url.into()],
        false,
    )
    .await?;
    Ok(WebDriverResponse::null())
}

pub async fn back(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        session.action_state = Default::default();
    }
    run_script(
        state,
        &session_id,
        "() => { window.history.back(); return null; }",
        Vec::new(),
        false,
    )
    .await?;
    Ok(WebDriverResponse::null())
}

pub async fn forward(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        session.action_state = Default::default();
    }
    run_script(
        state,
        &session_id,
        "() => { window.history.forward(); return null; }",
        Vec::new(),
        false,
    )
    .await?;
    Ok(WebDriverResponse::null())
}

pub async fn refresh(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        session.action_state = Default::default();
    }
    run_script(
        state,
        &session_id,
        "() => { window.location.reload(); return null; }",
        Vec::new(),
        false,
    )
    .await?;
    Ok(WebDriverResponse::null())
}

pub async fn get_title(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let title = run_script(state, &session_id, "() => document.title || ''", Vec::new(), false)
        .await?;
    Ok(WebDriverResponse::success(title))
}

pub async fn get_source(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let source = run_script(
        state,
        &session_id,
        "() => document.documentElement ? document.documentElement.outerHTML : ''",
        Vec::new(),
        false,
    )
    .await?;
    Ok(WebDriverResponse::success(source))
}
