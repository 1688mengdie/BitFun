use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ensure_session, run_script};
use crate::server::response::{WebDriverErrorResponse, WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default)]
    pub secure: bool,
    #[serde(default, rename = "httpOnly")]
    pub http_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "sameSite")]
    pub same_site: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddCookieRequest {
    cookie: Cookie,
}

pub async fn get_all(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let cookies = run_script(
        state,
        &session_id,
        "() => window.__bitfunWd.getAllCookies()",
        Vec::new(),
        false,
    )
    .await?;
    Ok(WebDriverResponse::success(cookies))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path((session_id, name)): Path<(String, String)>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let cookie = run_script(
        state,
        &session_id,
        "(name) => window.__bitfunWd.getCookie(name)",
        vec![Value::String(name.clone())],
        false,
    )
    .await?;

    if cookie.is_null() {
        return Err(WebDriverErrorResponse::no_such_cookie(format!(
            "Cookie '{name}' not found"
        )));
    }

    Ok(WebDriverResponse::success(cookie))
}

pub async fn add(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<AddCookieRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    run_script(
        state,
        &session_id,
        "(cookie) => window.__bitfunWd.addCookie(cookie)",
        vec![serde_json::to_value(request.cookie).unwrap_or(Value::Null)],
        false,
    )
    .await?;
    Ok(WebDriverResponse::null())
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path((session_id, name)): Path<(String, String)>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    run_script(
        state,
        &session_id,
        "(name) => window.__bitfunWd.deleteCookie(name)",
        vec![Value::String(name)],
        false,
    )
    .await?;
    Ok(WebDriverResponse::null())
}

pub async fn delete_all(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    run_script(
        state,
        &session_id,
        "() => window.__bitfunWd.deleteAllCookies()",
        Vec::new(),
        false,
    )
    .await?;
    Ok(WebDriverResponse::null())
}
