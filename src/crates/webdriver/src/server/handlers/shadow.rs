use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

use super::{ensure_session, run_script};
use crate::server::response::{WebDriverErrorResponse, WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct FindShadowRequest {
    using: String,
    value: String,
}

pub async fn get_shadow_root(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let value = run_script(
        state,
        &session_id,
        "(elementId) => window.__bitfunWd.getShadowRoot(elementId)",
        vec![Value::String(element_id)],
        false,
    )
    .await
    .map_err(|_| {
        WebDriverErrorResponse::no_such_shadow_root("Element does not have a shadow root")
    })?;

    if value.is_null() {
        return Err(WebDriverErrorResponse::no_such_shadow_root(
            "Element does not have a shadow root",
        ));
    }

    Ok(WebDriverResponse::success(value))
}

pub async fn find_element_in_shadow(
    State(state): State<Arc<AppState>>,
    Path((session_id, shadow_id)): Path<(String, String)>,
    Json(request): Json<FindShadowRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let value = run_script(
        state,
        &session_id,
        "(shadowId, using, value) => { const results = window.__bitfunWd.findElementsFromShadow(shadowId, using, value); return results.length ? results[0] : null; }",
        vec![
            Value::String(shadow_id),
            Value::String(request.using),
            Value::String(request.value),
        ],
        false,
    )
    .await?;

    if value.is_null() {
        return Err(WebDriverErrorResponse::no_such_element(
            "No shadow child element matched the selector",
        ));
    }

    Ok(WebDriverResponse::success(value))
}

pub async fn find_elements_in_shadow(
    State(state): State<Arc<AppState>>,
    Path((session_id, shadow_id)): Path<(String, String)>,
    Json(request): Json<FindShadowRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let value = run_script(
        state,
        &session_id,
        "(shadowId, using, value) => window.__bitfunWd.findElementsFromShadow(shadowId, using, value)",
        vec![
            Value::String(shadow_id),
            Value::String(request.using),
            Value::String(request.value),
        ],
        false,
    )
    .await?;
    Ok(WebDriverResponse::success(value))
}
