use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use tauri::{Manager, PhysicalPosition, PhysicalSize, Position, Size};

use super::{ensure_session, get_session};
use crate::server::response::{WebDriverErrorResponse, WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct SwitchWindowRequest {
    handle: String,
}

#[derive(Debug, Deserialize)]
pub struct NewWindowRequest {
    #[allow(dead_code)]
    #[serde(rename = "type", default)]
    window_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WindowRectRequest {
    #[serde(default)]
    x: Option<i32>,
    #[serde(default)]
    y: Option<i32>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
}

pub async fn get_window_handle(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let session = get_session(&state, &session_id).await?;
    Ok(WebDriverResponse::success(session.current_window))
}

pub async fn switch_to_window(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<SwitchWindowRequest>,
) -> WebDriverResult {
    if !state.has_window(&request.handle) {
        return Err(WebDriverErrorResponse::no_such_window(format!(
            "Unknown window handle: {}",
            request.handle
        )));
    }

    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id)?;
    session.current_window = request.handle;

    Ok(WebDriverResponse::null())
}

pub async fn get_window_handles(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    Ok(WebDriverResponse::success(state.window_labels()))
}

pub async fn close_window(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let current_window = get_session(&state, &session_id).await?.current_window;
    let window = state
        .app
        .get_webview_window(&current_window)
        .ok_or_else(|| {
            WebDriverErrorResponse::no_such_window(format!("Window not found: {current_window}"))
        })?;
    window.destroy().map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to close window: {error}"))
    })?;

    let handles = state.window_labels();
    let next_handle = handles.first().cloned();
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id)?;
    if let Some(next_handle) = next_handle {
        session.current_window = next_handle;
    }
    Ok(WebDriverResponse::success(handles))
}

pub async fn new_window(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(_request): Json<NewWindowRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    Err(WebDriverErrorResponse::unsupported_operation(
        "Creating new windows is not supported in this context",
    ))
}

pub async fn get_window_rect(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let session = get_session(&state, &session_id).await?;
    let window = state
        .app
        .get_webview_window(&session.current_window)
        .ok_or_else(|| {
            WebDriverErrorResponse::no_such_window(format!(
                "Window not found: {}",
                session.current_window
            ))
        })?;

    let position = window.outer_position().map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to read window position: {error}"))
    })?;
    let size = window.outer_size().map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to read window size: {error}"))
    })?;

    Ok(WebDriverResponse::success(json!({
        "x": position.x,
        "y": position.y,
        "width": size.width,
        "height": size.height
    })))
}

pub async fn set_window_rect(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<WindowRectRequest>,
) -> WebDriverResult {
    let session = get_session(&state, &session_id).await?;
    let window = state
        .app
        .get_webview_window(&session.current_window)
        .ok_or_else(|| {
            WebDriverErrorResponse::no_such_window(format!(
                "Window not found: {}",
                session.current_window
            ))
        })?;

    let current_position = window.outer_position().map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to read window position: {error}"))
    })?;
    let current_size = window.outer_size().map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to read window size: {error}"))
    })?;

    let x = request.x.unwrap_or(current_position.x);
    let y = request.y.unwrap_or(current_position.y);
    let width = request.width.unwrap_or(current_size.width);
    let height = request.height.unwrap_or(current_size.height);

    window
        .set_position(Position::Physical(PhysicalPosition::new(x, y)))
        .map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to set window position: {error}"))
        })?;
    window
        .set_size(Size::Physical(PhysicalSize::new(width, height)))
        .map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to set window size: {error}"))
        })?;

    Ok(WebDriverResponse::success(json!({
        "x": x,
        "y": y,
        "width": width,
        "height": height
    })))
}

pub async fn maximize(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let session = get_session(&state, &session_id).await?;
    let window = state
        .app
        .get_webview_window(&session.current_window)
        .ok_or_else(|| {
            WebDriverErrorResponse::no_such_window(format!(
                "Window not found: {}",
                session.current_window
            ))
        })?;
    window.maximize().map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to maximize window: {error}"))
    })?;
    get_window_rect(State(state), Path(session_id)).await
}

pub async fn minimize(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let session = get_session(&state, &session_id).await?;
    let window = state
        .app
        .get_webview_window(&session.current_window)
        .ok_or_else(|| {
            WebDriverErrorResponse::no_such_window(format!(
                "Window not found: {}",
                session.current_window
            ))
        })?;
    window.minimize().map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to minimize window: {error}"))
    })?;
    Ok(WebDriverResponse::null())
}

pub async fn fullscreen(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let session = get_session(&state, &session_id).await?;
    let window = state
        .app
        .get_webview_window(&session.current_window)
        .ok_or_else(|| {
            WebDriverErrorResponse::no_such_window(format!(
                "Window not found: {}",
                session.current_window
            ))
        })?;
    window.set_fullscreen(true).map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to fullscreen window: {error}"))
    })?;
    get_window_rect(State(state), Path(session_id)).await
}
