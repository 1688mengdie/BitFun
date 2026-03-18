use std::sync::Arc;

use axum::extract::State;
use serde_json::{json, Value};

use crate::executor::BridgeExecutor;
use crate::server::response::{WebDriverErrorResponse, WebDriverResponse, WebDriverResult};
use crate::server::AppState;
use crate::webdriver::Session;

pub mod actions;
pub mod alert;
pub mod cookie;
pub mod element;
pub mod frame;
pub mod logs;
pub mod navigation;
pub mod print;
pub mod screenshot;
pub mod shadow;
pub mod script;
pub mod session;
pub mod timeouts;
pub mod window;

pub async fn status(State(state): State<Arc<AppState>>) -> WebDriverResponse {
    WebDriverResponse::success(json!({
        "ready": state.initial_window_label().is_some(),
        "message": "BitFun embedded WebDriver is ready",
        "build": {
            "version": env!("CARGO_PKG_VERSION"),
            "name": "bitfun-embedded-webdriver"
        }
    }))
}

pub(crate) async fn get_session(
    state: &Arc<AppState>,
    session_id: &str,
) -> Result<Session, WebDriverErrorResponse> {
    state.sessions.read().await.get_cloned(session_id)
}

pub(crate) async fn ensure_session(
    state: &Arc<AppState>,
    session_id: &str,
) -> Result<(), WebDriverErrorResponse> {
    let _ = get_session(state, session_id).await?;
    Ok(())
}

pub(crate) async fn run_script(
    state: Arc<AppState>,
    session_id: &str,
    script: &str,
    args: Vec<Value>,
    async_mode: bool,
) -> Result<Value, WebDriverErrorResponse> {
    BridgeExecutor::from_session_id(state, session_id)
        .await?
        .run_script(script, args, async_mode)
        .await
}

pub(crate) async fn find_elements(
    state: Arc<AppState>,
    session_id: &str,
    root_element_id: Option<String>,
    using: String,
    value: String,
) -> Result<Vec<Value>, WebDriverErrorResponse> {
    BridgeExecutor::from_session_id(state, session_id)
        .await?
        .find_elements(root_element_id, &using, &value)
        .await
}

pub(crate) async fn element_boolean_response(
    state: Arc<AppState>,
    session_id: &str,
    element_id: &str,
    script: &str,
) -> WebDriverResult {
    ensure_session(&state, session_id).await?;
    let result = run_script(
        state,
        session_id,
        script,
        vec![Value::String(element_id.to_string())],
        false,
    )
    .await?;
    Ok(WebDriverResponse::success(result))
}

pub(crate) async fn element_value_response(
    state: Arc<AppState>,
    session_id: &str,
    element_id: &str,
    script: &str,
    mut extra_args: Vec<Value>,
) -> WebDriverResult {
    ensure_session(&state, session_id).await?;
    let mut args = vec![Value::String(element_id.to_string())];
    args.append(&mut extra_args);
    let result = run_script(state, session_id, script, args, false).await?;
    Ok(WebDriverResponse::success(result))
}

pub(crate) async fn element_action_response(
    state: Arc<AppState>,
    session_id: &str,
    element_id: &str,
    script: &str,
    mut extra_args: Vec<Value>,
) -> WebDriverResult {
    ensure_session(&state, session_id).await?;
    let mut args = vec![Value::String(element_id.to_string())];
    args.append(&mut extra_args);
    run_script(state, session_id, script, args, false).await?;
    Ok(WebDriverResponse::null())
}
