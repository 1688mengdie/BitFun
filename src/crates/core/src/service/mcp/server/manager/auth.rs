use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use reqwest::Url;
use tokio::sync::{oneshot, Mutex};
use tokio::time::{timeout, Duration};

use crate::service::mcp::auth::{
    clear_stored_oauth_credentials, map_auth_error, prepare_remote_oauth_authorization,
    MCPRemoteOAuthSessionSnapshot, MCPRemoteOAuthStatus,
};
use crate::service::mcp::server::MCPServerType;
use crate::util::errors::{BitFunError, BitFunResult};

use super::{ActiveRemoteOAuthSession, MCPServerManager};

const OAUTH_CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug)]
struct OAuthCallbackPayload {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn render_oauth_callback_page(payload: &OAuthCallbackPayload) -> String {
    let (badge, badge_class, title, message, detail_title, detail_body, icon_label) =
        if let Some(error) = payload.error.as_deref() {
            let description = payload
                .error_description
                .as_deref()
                .unwrap_or("The provider rejected the authorization request.");
            (
                "Authorization failed",
                "is-error",
                "BitFun could not finish the OAuth handoff",
                "Return to BitFun and restart the OAuth flow after checking the provider response below.",
                "Provider response",
                format!("{}: {}", escape_html(error), escape_html(description)),
                "!",
            )
        } else if payload.code.is_some() && payload.state.is_some() {
            (
                "Authorization received",
                "is-success",
                "BitFun has the callback",
                "You can switch back to the app now. BitFun is exchanging the authorization code and reconnecting the MCP server.",
                "What happens next",
                "This tab can be closed. If BitFun does not finish reconnecting automatically, reopen the MCP settings and retry OAuth.".to_string(),
                "OK",
            )
        } else {
            let mut missing = Vec::new();
            if payload.code.is_none() {
                missing.push("code");
            }
            if payload.state.is_none() {
                missing.push("state");
            }
            (
                "Callback incomplete",
                "is-warning",
                "BitFun received an incomplete OAuth redirect",
                "The provider redirected back, but required OAuth parameters were missing. Return to BitFun and start the sign-in flow again.",
                "Missing parameters",
                escape_html(&missing.join(", ")),
                "?",
            )
        };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>BitFun OAuth Callback</title>
    <style>
      :root {{
        color-scheme: light;
        --bg-0: #f3efe5;
        --bg-1: #dbe7ff;
        --bg-2: #f8c98b;
        --panel: rgba(255, 252, 246, 0.88);
        --panel-border: rgba(53, 66, 97, 0.14);
        --text-strong: #172033;
        --text-muted: #5c6474;
        --shadow: 0 24px 80px rgba(23, 32, 51, 0.16);
        --success: #176b52;
        --success-soft: rgba(23, 107, 82, 0.12);
        --warning: #9a5a00;
        --warning-soft: rgba(154, 90, 0, 0.14);
        --error: #a63232;
        --error-soft: rgba(166, 50, 50, 0.12);
      }}

      * {{
        box-sizing: border-box;
      }}

      body {{
        margin: 0;
        min-height: 100vh;
        font-family: "Segoe UI Variable Display", "Aptos", "Trebuchet MS", sans-serif;
        color: var(--text-strong);
        background:
          radial-gradient(circle at top left, rgba(255, 255, 255, 0.72), transparent 34%),
          radial-gradient(circle at bottom right, rgba(255, 230, 202, 0.9), transparent 30%),
          linear-gradient(135deg, var(--bg-0) 0%, var(--bg-1) 52%, var(--bg-2) 100%);
        overflow: hidden;
      }}

      .orb {{
        position: fixed;
        border-radius: 999px;
        filter: blur(12px);
        opacity: 0.56;
        pointer-events: none;
      }}

      .orb-a {{
        width: 320px;
        height: 320px;
        top: -96px;
        right: -48px;
        background: rgba(126, 159, 255, 0.34);
      }}

      .orb-b {{
        width: 260px;
        height: 260px;
        bottom: -84px;
        left: -40px;
        background: rgba(255, 193, 118, 0.38);
      }}

      .shell {{
        position: relative;
        min-height: 100vh;
        display: grid;
        place-items: center;
        padding: 28px;
      }}

      .panel {{
        width: min(100%, 720px);
        padding: 32px;
        border: 1px solid var(--panel-border);
        border-radius: 28px;
        background: var(--panel);
        backdrop-filter: blur(18px);
        box-shadow: var(--shadow);
      }}

      .brand {{
        display: flex;
        align-items: center;
        gap: 16px;
        margin-bottom: 24px;
      }}

      .brand-mark {{
        width: 52px;
        height: 52px;
        border-radius: 16px;
        display: grid;
        place-items: center;
        font-weight: 700;
        letter-spacing: 0.08em;
        color: #fffaf0;
        background: linear-gradient(135deg, #172033 0%, #335c95 100%);
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.16);
      }}

      .eyebrow {{
        display: block;
        margin-bottom: 4px;
        font-size: 12px;
        font-weight: 700;
        letter-spacing: 0.16em;
        text-transform: uppercase;
        color: var(--text-muted);
      }}

      h1 {{
        margin: 0;
        font-size: clamp(28px, 5vw, 44px);
        line-height: 1.04;
        letter-spacing: -0.04em;
      }}

      .badge {{
        display: inline-flex;
        align-items: center;
        gap: 8px;
        padding: 8px 14px;
        border-radius: 999px;
        font-size: 13px;
        font-weight: 700;
      }}

      .badge.is-success {{
        color: var(--success);
        background: var(--success-soft);
      }}

      .badge.is-warning {{
        color: var(--warning);
        background: var(--warning-soft);
      }}

      .badge.is-error {{
        color: var(--error);
        background: var(--error-soft);
      }}

      .content {{
        display: grid;
        gap: 20px;
      }}

      .lead {{
        margin: 0;
        max-width: 58ch;
        font-size: 17px;
        line-height: 1.7;
        color: var(--text-muted);
      }}

      .status-card {{
        display: grid;
        grid-template-columns: auto 1fr;
        gap: 18px;
        padding: 20px;
        border-radius: 22px;
        background: rgba(255, 255, 255, 0.58);
        border: 1px solid rgba(53, 66, 97, 0.08);
      }}

      .status-icon {{
        width: 52px;
        height: 52px;
        border-radius: 18px;
        display: grid;
        place-items: center;
        font-weight: 800;
        font-size: 16px;
        color: var(--text-strong);
        background: linear-gradient(135deg, rgba(255, 255, 255, 0.94), rgba(227, 235, 255, 0.92));
        border: 1px solid rgba(53, 66, 97, 0.08);
      }}

      .status-title {{
        margin: 0 0 8px;
        font-size: 15px;
        font-weight: 700;
        letter-spacing: 0.01em;
      }}

      .status-body {{
        margin: 0;
        font-family: "Cascadia Code", "Consolas", monospace;
        font-size: 13px;
        line-height: 1.7;
        color: var(--text-muted);
        word-break: break-word;
      }}

      .actions {{
        display: flex;
        flex-wrap: wrap;
        gap: 12px;
        margin-top: 4px;
      }}

      button {{
        appearance: none;
        border: 0;
        border-radius: 999px;
        padding: 12px 18px;
        font: inherit;
        font-weight: 700;
        cursor: pointer;
        transition: transform 140ms ease, opacity 140ms ease, background 140ms ease;
      }}

      button:hover {{
        transform: translateY(-1px);
      }}

      .primary {{
        color: #fffaf0;
        background: linear-gradient(135deg, #172033 0%, #335c95 100%);
      }}

      .secondary {{
        color: var(--text-strong);
        background: rgba(23, 32, 51, 0.08);
      }}

      .footnote {{
        margin: 2px 0 0;
        font-size: 13px;
        line-height: 1.7;
        color: var(--text-muted);
      }}

      @media (max-width: 640px) {{
        .panel {{
          padding: 24px;
          border-radius: 24px;
        }}

        .brand,
        .status-card {{
          grid-template-columns: 1fr;
        }}

        .status-card {{
          gap: 14px;
        }}
      }}
    </style>
  </head>
  <body>
    <div class="orb orb-a"></div>
    <div class="orb orb-b"></div>
    <main class="shell">
      <section class="panel">
        <div class="brand">
          <div class="brand-mark">BF</div>
          <div>
            <span class="eyebrow">BitFun Desktop</span>
            <h1>{title}</h1>
          </div>
        </div>
        <div class="content">
          <div class="badge {badge_class}">{badge}</div>
          <p class="lead">{message}</p>
          <div class="status-card">
            <div class="status-icon">{icon_label}</div>
            <div>
              <p class="status-title">{detail_title}</p>
              <p class="status-body">{detail_body}</p>
            </div>
          </div>
          <div class="actions">
            <button class="primary" type="button" onclick="window.close()">Close this tab</button>
            <button class="secondary" type="button" onclick="location.reload()">Refresh page</button>
          </div>
          <p class="footnote">
            This page will try to close automatically in <span id="countdown">4</span>s. If your browser blocks that action, you can close it manually and return to BitFun.
          </p>
        </div>
      </section>
    </main>
    <script>
      let secondsRemaining = 4;
      const countdown = document.getElementById('countdown');
      const intervalId = window.setInterval(() => {{
        secondsRemaining -= 1;
        if (countdown && secondsRemaining >= 0) {{
          countdown.textContent = String(secondsRemaining);
        }}
        if (secondsRemaining <= 0) {{
          window.clearInterval(intervalId);
        }}
      }}, 1000);
      window.setTimeout(() => window.close(), 4000);
    </script>
  </body>
</html>"#,
        title = title,
        badge = badge,
        badge_class = badge_class,
        message = message,
        detail_title = detail_title,
        detail_body = detail_body,
        icon_label = icon_label,
    )
}

#[derive(Clone)]
struct OAuthCallbackAppState {
    callback_tx: Arc<Mutex<Option<oneshot::Sender<OAuthCallbackPayload>>>>,
}

impl MCPServerManager {
    pub(super) async fn set_oauth_snapshot(
        session: &Arc<ActiveRemoteOAuthSession>,
        snapshot: MCPRemoteOAuthSessionSnapshot,
    ) {
        *session.snapshot.write().await = snapshot;
    }

    pub(super) async fn update_oauth_snapshot<F>(
        session: &Arc<ActiveRemoteOAuthSession>,
        update: F,
    ) -> MCPRemoteOAuthSessionSnapshot
    where
        F: FnOnce(&mut MCPRemoteOAuthSessionSnapshot),
    {
        let mut snapshot = session.snapshot.write().await;
        update(&mut snapshot);
        snapshot.clone()
    }

    pub(super) async fn insert_oauth_session(
        &self,
        server_id: &str,
        session: Arc<ActiveRemoteOAuthSession>,
    ) -> Option<Arc<ActiveRemoteOAuthSession>> {
        self.oauth_sessions
            .write()
            .await
            .insert(server_id.to_string(), session)
    }

    pub(super) async fn shutdown_oauth_session(session: &Arc<ActiveRemoteOAuthSession>) {
        if let Some(shutdown_tx) = session.shutdown_tx.lock().await.take() {
            let _ = shutdown_tx.send(());
        }
    }

    async fn fail_oauth_session(
        session: &Arc<ActiveRemoteOAuthSession>,
        message: String,
    ) -> MCPRemoteOAuthSessionSnapshot {
        let snapshot = MCPServerManager::update_oauth_snapshot(session, |snapshot| {
            snapshot.status = MCPRemoteOAuthStatus::Failed;
            snapshot.message = Some(message);
        })
        .await;
        Self::shutdown_oauth_session(session).await;
        snapshot
    }

    pub async fn start_remote_oauth_authorization(
        &self,
        server_id: &str,
    ) -> BitFunResult<MCPRemoteOAuthSessionSnapshot> {
        let config = self
            .config_service
            .get_server_config(server_id)
            .await?
            .ok_or_else(|| {
                BitFunError::NotFound(format!("MCP server config not found: {}", server_id))
            })?;

        if config.server_type != MCPServerType::Remote {
            return Err(BitFunError::Validation(format!(
                "MCP server '{}' is not a remote server",
                server_id
            )));
        }

        if let Some(existing) = self.oauth_sessions.write().await.remove(server_id) {
            Self::shutdown_oauth_session(&existing).await;
        }

        let prepared = prepare_remote_oauth_authorization(&config).await?;
        let callback_path = Url::parse(&prepared.redirect_uri)
            .map_err(|error| {
                BitFunError::MCPError(format!(
                    "Invalid OAuth redirect URI for server '{}': {}",
                    server_id, error
                ))
            })?
            .path()
            .to_string();

        let initial_snapshot = MCPRemoteOAuthSessionSnapshot::new(
            server_id.to_string(),
            MCPRemoteOAuthStatus::AwaitingBrowser,
            Some(prepared.authorization_url.clone()),
            Some(prepared.redirect_uri.clone()),
            Some("Open the authorization URL to continue OAuth sign-in.".to_string()),
        );

        let (callback_tx, callback_rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let session = Arc::new(ActiveRemoteOAuthSession {
            snapshot: Arc::new(tokio::sync::RwLock::new(initial_snapshot.clone())),
            shutdown_tx: Mutex::new(Some(shutdown_tx)),
        });

        if let Some(previous) = self.insert_oauth_session(server_id, session.clone()).await {
            Self::shutdown_oauth_session(&previous).await;
        }

        let callback_state = OAuthCallbackAppState {
            callback_tx: Arc::new(Mutex::new(Some(callback_tx))),
        };
        let router = Router::new()
            .route(&callback_path, get(handle_oauth_callback))
            .with_state(callback_state);
        let callback_server_session = session.clone();
        let callback_server_id = server_id.to_string();
        tokio::spawn(async move {
            let server =
                axum::serve(prepared.listener, router).with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                });

            if let Err(error) = server.await {
                let _ =
                    MCPServerManager::update_oauth_snapshot(&callback_server_session, |snapshot| {
                        if matches!(
                            snapshot.status,
                            MCPRemoteOAuthStatus::Authorized | MCPRemoteOAuthStatus::Cancelled
                        ) {
                            return;
                        }
                        snapshot.status = MCPRemoteOAuthStatus::Failed;
                        snapshot.message = Some(format!(
                            "OAuth callback listener failed for server '{}': {}",
                            callback_server_id, error
                        ));
                    })
                    .await;
            }
        });

        let manager = self.clone();
        let callback_session = session.clone();
        let callback_server_id = server_id.to_string();
        let authorization_url = prepared.authorization_url.clone();
        let redirect_uri = prepared.redirect_uri.clone();
        let mut oauth_state = prepared.state;
        tokio::spawn(async move {
            let _ = MCPServerManager::update_oauth_snapshot(&callback_session, |snapshot| {
                snapshot.status = MCPRemoteOAuthStatus::AwaitingCallback;
                snapshot.message =
                    Some("Waiting for the OAuth provider to redirect back to BitFun.".to_string());
            })
            .await;

            let callback = match timeout(OAUTH_CALLBACK_TIMEOUT, callback_rx).await {
                Ok(Ok(callback)) => callback,
                Ok(Err(_)) => {
                    let _ =
                        MCPServerManager::update_oauth_snapshot(&callback_session, |snapshot| {
                            snapshot.status = MCPRemoteOAuthStatus::Cancelled;
                            snapshot.message =
                                Some("OAuth authorization was cancelled.".to_string());
                        })
                        .await;
                    Self::shutdown_oauth_session(&callback_session).await;
                    return;
                }
                Err(_) => {
                    let _ = MCPServerManager::fail_oauth_session(
                        &callback_session,
                        "OAuth authorization timed out before the provider redirected back."
                            .to_string(),
                    )
                    .await;
                    return;
                }
            };

            if let Some(error) = callback.error {
                let description = callback
                    .error_description
                    .map(|value| format!(": {}", value))
                    .unwrap_or_default();
                let _ = MCPServerManager::fail_oauth_session(
                    &callback_session,
                    format!("OAuth provider returned '{}{}'", error, description),
                )
                .await;
                return;
            }

            let code = match callback.code {
                Some(code) => code,
                None => {
                    let _ = MCPServerManager::fail_oauth_session(
                        &callback_session,
                        "OAuth callback did not include an authorization code.".to_string(),
                    )
                    .await;
                    return;
                }
            };

            let state = match callback.state {
                Some(state) => state,
                None => {
                    let _ = MCPServerManager::fail_oauth_session(
                        &callback_session,
                        "OAuth callback did not include a state token.".to_string(),
                    )
                    .await;
                    return;
                }
            };

            let _ = MCPServerManager::update_oauth_snapshot(&callback_session, |snapshot| {
                snapshot.status = MCPRemoteOAuthStatus::ExchangingToken;
                snapshot.message =
                    Some("Exchanging the authorization code for an access token.".to_string());
            })
            .await;

            match oauth_state.handle_callback(&code, &state).await {
                Ok(_) => {
                    let _ = MCPServerManager::set_oauth_snapshot(
                        &callback_session,
                        MCPRemoteOAuthSessionSnapshot::new(
                            callback_server_id.clone(),
                            MCPRemoteOAuthStatus::Authorized,
                            Some(authorization_url.clone()),
                            Some(redirect_uri.clone()),
                            Some(
                                "OAuth authorization completed. Reconnecting MCP server."
                                    .to_string(),
                            ),
                        ),
                    )
                    .await;

                    if let Some(shutdown_tx) = callback_session.shutdown_tx.lock().await.take() {
                        let _ = shutdown_tx.send(());
                    }

                    manager.clear_reconnect_state(&callback_server_id).await;
                    let _ = manager.stop_server(&callback_server_id).await;
                    if let Err(error) = manager.start_server(&callback_server_id).await {
                        let _ = MCPServerManager::update_oauth_snapshot(
                            &callback_session,
                            |snapshot| {
                                snapshot.message = Some(format!(
                                    "OAuth token saved, but reconnect failed: {}",
                                    error
                                ));
                            },
                        )
                        .await;
                    }
                }
                Err(error) => {
                    let _ = MCPServerManager::fail_oauth_session(
                        &callback_session,
                        map_auth_error(error).to_string(),
                    )
                    .await;
                }
            }
        });

        Ok(initial_snapshot)
    }

    pub async fn get_remote_oauth_session(
        &self,
        server_id: &str,
    ) -> Option<MCPRemoteOAuthSessionSnapshot> {
        let session = self.oauth_sessions.read().await.get(server_id).cloned()?;
        let snapshot = session.snapshot.read().await.clone();
        Some(snapshot)
    }

    pub async fn cancel_remote_oauth_authorization(&self, server_id: &str) -> BitFunResult<()> {
        let session = self.oauth_sessions.write().await.remove(server_id);
        if let Some(session) = session {
            let _ = MCPServerManager::update_oauth_snapshot(&session, |snapshot| {
                snapshot.status = MCPRemoteOAuthStatus::Cancelled;
                snapshot.message = Some("OAuth authorization was cancelled.".to_string());
            })
            .await;
            Self::shutdown_oauth_session(&session).await;
        }
        Ok(())
    }

    pub async fn clear_remote_oauth_credentials(&self, server_id: &str) -> BitFunResult<()> {
        self.cancel_remote_oauth_authorization(server_id).await?;
        clear_stored_oauth_credentials(server_id).await
    }
}

async fn handle_oauth_callback(
    State(state): State<OAuthCallbackAppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let payload = OAuthCallbackPayload {
        code: params.get("code").cloned(),
        state: params.get("state").cloned(),
        error: params.get("error").cloned(),
        error_description: params.get("error_description").cloned(),
    };
    let page = render_oauth_callback_page(&payload);

    if let Some(callback_tx) = state.callback_tx.lock().await.take() {
        let _ = callback_tx.send(payload);
    }

    Html(page)
}
