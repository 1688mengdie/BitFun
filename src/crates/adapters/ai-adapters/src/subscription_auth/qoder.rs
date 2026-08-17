//! Qoder subscription login and credential resolution.
//!
//! Aligned with the official Qoder CLI (`@qodercn-ai/qoderclicn`): a device
//! flow (RFC 8628 style) with PKCE S256. The client constructs a
//! `selectAccounts` authorization URL for the user's browser, then polls the
//! device-token endpoint until the user approves. Unlike a standard device
//! grant there is no separate device-code endpoint, and the endpoints are
//! hard-coded (OIDC discovery is only used by Qoder's MCP servers).
//!
//! Inference requests authenticate with `Authorization: Bearer {token}` plus
//! `X-Request-ID`/`X-Session-ID`. There is no `X-Qoder-*` authentication
//! header family on the inference gateway.

use super::store::{self, StoredCredential};
use super::{pkce::Pkce, ResolvedCredential, StartedLogin, SubscriptionHttpOptions};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const BASE_URL: &str = "https://qoder.cn";
const OPENAPI_URL: &str = "https://openapi.qoder.com.cn";
const CLIENT_ID: &str = "e883ade2-e6e3-4d6d-adf7-f92ceff5fdcb";
const MODEL_BASE_URL: &str = "https://api2-v2.qoder.sh";
const MODEL_REQUEST_URL: &str = "https://api2-v2.qoder.sh/model/v1/chat/completions";
const DEFAULT_MODEL: &str = "auto";
const STORE_KEY: &str = "qoder";
const REFRESH_LEEWAY_MS: i64 = 5 * 60 * 1000;
const POLL_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const POLL_RETRY_MS: Duration = Duration::from_secs(1);

/// Response of the device-token poll endpoint.
#[derive(Debug, Deserialize)]
struct DeviceTokenResponse {
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    user_name: Option<String>,
}

/// A poll result: either an error code the CLI keeps retrying, or a complete
/// token payload.
#[derive(Debug, Deserialize)]
struct PollError {
    code: Option<String>,
}

fn http_client(options: &SubscriptionHttpOptions) -> Result<reqwest::Client> {
    super::build_http_client(options, "Qoder")
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn authorization_url(pkce: &Pkce, machine_id: &str) -> String {
    format!(
        "{BASE_URL}/device/selectAccounts?challenge={}&challenge_method=S256&nonce={}&machine_id={}&client_id={}",
        pkce.challenge, Uuid::new_v4(), machine_id, CLIENT_ID
    )
}

/// Recovers the machine id the same way the Qoder CLI does: reuse the
/// persisted machine id, or fall back to a fresh UUID. BitFun does not
/// persist a Qoder machine id, so this always falls back to a fresh UUID.
pub(crate) fn recover_machine_id() -> String {
    Uuid::new_v4().to_string()
}

/// One device-token poll. A `404` (or a 200 JSON body carrying an error code
/// the CLI keeps retrying) means the user has not approved yet.
enum PollOutcome {
    Pending,
    Authorized(DeviceTokenResponse),
}

async fn poll_once(
    nonce: &str,
    verifier: &str,
    options: &SubscriptionHttpOptions,
) -> Result<PollOutcome> {
    let client = http_client(options)?;
    let url = format!(
        "{OPENAPI_URL}/api/v1/deviceToken/poll?nonce={nonce}&verifier={verifier}&challenge_method=S256"
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .context("call qoder device token poll endpoint")?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(PollOutcome::Pending);
    }
    if let Ok(payload) = serde_json::from_str::<DeviceTokenResponse>(&body) {
        if payload.token.is_some() {
            return Ok(PollOutcome::Authorized(payload));
        }
    }
    if let Ok(payload) = serde_json::from_str::<PollError>(&body) {
        // 200 + JSON errorCode keeps polling (the CLI treats a transient
        // error code the same as a pending state).
        if payload.code.is_some() {
            return Ok(PollOutcome::Pending);
        }
    }
    if !status.is_success() {
        return Err(anyhow!(
            "qoder device token poll failed: HTTP {status}: {body}"
        ));
    }
    Err(anyhow!(
        "qoder device token poll response unrecognized: {body}"
    ))
}

/// Starts the device flow. The `selectAccounts` URL is returned immediately;
/// the runner polls the device-token endpoint in the background.
pub(crate) async fn begin_login(
    cancel: CancellationToken,
    expected_revision: u64,
    options: SubscriptionHttpOptions,
) -> Result<StartedLogin> {
    let pkce = Pkce::generate();
    let nonce = Uuid::new_v4().to_string();
    let machine_id = recover_machine_id();
    let authorization_url = authorization_url(&pkce, &machine_id);
    let verifier = pkce.verifier.clone();

    let runner = async move {
        let cancel = cancel.clone();
        super::authorize_then_persist(
            super::SubscriptionProvider::Qoder,
            cancel.clone(),
            async {
                let started = tokio::time::Instant::now();
                loop {
                    match poll_once(&nonce, &verifier, &options).await? {
                        PollOutcome::Pending => {
                            if started.elapsed() > POLL_TIMEOUT {
                                return Err(anyhow!("Login timed out"));
                            }
                            tokio::select! {
                                _ = cancel.cancelled() => return Err(anyhow!("login cancelled")),
                                _ = tokio::time::sleep(POLL_RETRY_MS) => {}
                            }
                        }
                        PollOutcome::Authorized(tokens) => {
                            return Ok((tokens, nonce));
                        }
                    }
                }
            },
            move |(tokens, _nonce)| persist_tokens(tokens, expected_revision),
        )
        .await
    };

    Ok(StartedLogin {
        authorization_url,
        user_code: None,
        instructions: "Open the authorization link in your browser, then return to BitFun."
            .to_string(),
        runner: Box::pin(runner),
    })
}

fn token_expiry(expires_in: Option<i64>) -> i64 {
    match expires_in {
        Some(seconds) if seconds > 0 => now_ms() + seconds * 1000,
        _ => now_ms() + 3600 * 1000,
    }
}

fn account_metadata(tokens: &DeviceTokenResponse) -> Option<serde_json::Value> {
    let uid = tokens.user_id.clone();
    let name = tokens.user_name.clone();
    if uid.is_none() && name.is_none() {
        return None;
    }
    let mut object = serde_json::Map::new();
    if let Some(uid) = uid {
        object.insert("uid".to_string(), serde_json::Value::String(uid));
    }
    if let Some(name) = name {
        object.insert("name".to_string(), serde_json::Value::String(name));
    }
    Some(serde_json::Value::Object(object))
}

async fn persist_tokens(tokens: DeviceTokenResponse, expected_revision: u64) -> Result<()> {
    let access = tokens
        .token
        .clone()
        .ok_or_else(|| anyhow!("qoder device token response missing token"))?;
    let refresh = tokens.refresh_token.clone().unwrap_or_default();
    let expires = token_expiry(tokens.expires_in);
    let account_id = tokens.user_id.clone();
    let metadata = account_metadata(&tokens);
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        expected_revision,
        StoredCredential::Oauth {
            refresh,
            access,
            expires,
            account_id,
            metadata,
        },
    )
    .await?;
    super::require_current_store_revision(super::SubscriptionProvider::Qoder, outcome)?;
    log::info!("qoder subscription tokens saved");
    Ok(())
}

async fn refresh(
    refresh_token: &str,
    options: &SubscriptionHttpOptions,
) -> Result<DeviceTokenResponse> {
    let client = http_client(options)?;
    let resp = client
        .post(format!("{OPENAPI_URL}/api/v1/deviceToken/refresh"))
        .json(&serde_json::json!({ "refresh_token": refresh_token }))
        .send()
        .await
        .context("call qoder device token refresh endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("qoder token refresh failed: HTTP {status}: {body}"));
    }
    resp.json().await.context("parse qoder refresh response")
}

/// Loads the stored credential, refreshing the access token when it is about
/// to expire. Returns `(access, expires_ms)`.
async fn ensure_fresh(options: &SubscriptionHttpOptions) -> Result<(String, i64)> {
    let snapshot = store::load_entry_with_revision(STORE_KEY).await?;
    let entry = snapshot
        .credential
        .ok_or_else(|| anyhow!("Qoder is not connected; sign in first"))?;
    let StoredCredential::Oauth {
        refresh: refresh_token,
        access,
        expires,
        account_id,
        metadata,
    } = entry
    else {
        return Err(anyhow!("Qoder credential is not an OAuth login"));
    };

    if expires > now_ms() + REFRESH_LEEWAY_MS {
        return Ok((access, expires));
    }
    if refresh_token.is_empty() {
        return Err(anyhow!("Qoder credential has no refresh token"));
    }

    let refreshed = refresh(&refresh_token, options).await?;
    let new_access = refreshed
        .token
        .clone()
        .ok_or_else(|| anyhow!("qoder refresh response missing token"))?;
    let new_refresh = refreshed.refresh_token.clone().unwrap_or(refresh_token);
    let new_expires = token_expiry(refreshed.expires_in);
    let new_account_id = refreshed.user_id.clone().or(account_id);
    let new_metadata = refreshed
        .user_id
        .as_ref()
        .and_then(|_| account_metadata(&refreshed))
        .or(metadata);
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        snapshot.revision,
        StoredCredential::Oauth {
            refresh: new_refresh,
            access: new_access.clone(),
            expires: new_expires,
            account_id: new_account_id,
            metadata: new_metadata,
        },
    )
    .await?;
    match outcome {
        store::ConditionalCommitOutcome::Committed { .. } => {
            log::info!("qoder subscription tokens refreshed");
            Ok((new_access, new_expires))
        }
        store::ConditionalCommitOutcome::Conflict { current_revision } => {
            let current = super::load_current_store_after_conflict(
                super::SubscriptionProvider::Qoder,
                current_revision,
            )
            .await?;
            match current.credential {
                Some(StoredCredential::Oauth {
                    access, expires, ..
                }) if expires > now_ms() => {
                    log::info!("qoder refresh reused tokens committed by a concurrent refresh");
                    Ok((access, expires))
                }
                _ => Err(super::store_revision_conflict(
                    super::SubscriptionProvider::Qoder,
                    current_revision,
                )),
            }
        }
    }
}

/// Resolves the runtime credential, injecting the Qoder inference headers.
pub(crate) async fn resolve(options: &SubscriptionHttpOptions) -> Result<ResolvedCredential> {
    let (access, expires) = ensure_fresh(options).await?;
    let mut headers = HashMap::new();
    headers.insert("X-Request-ID".to_string(), Uuid::new_v4().to_string());
    headers.insert("X-Session-ID".to_string(), Uuid::new_v4().to_string());
    headers.insert("Accept".to_string(), "text/event-stream".to_string());
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    Ok(ResolvedCredential {
        api_key: access,
        base_url: Some(MODEL_BASE_URL.to_string()),
        request_url: Some(MODEL_REQUEST_URL.to_string()),
        format: Some("openai".to_string()),
        extra_headers: headers,
        expires_at: Some(expires / 1000),
    })
}

/// Provider metadata used to seed a new model entry.
///
/// Qoder's catalog decides the default model server-side; `auto` is what the
/// official client sends when no explicit model is selected.
pub(crate) fn suggested() -> (&'static str, &'static str, &'static str) {
    ("openai", MODEL_BASE_URL, DEFAULT_MODEL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_defaults_to_auto_model() {
        let (format, base_url, model) = suggested();
        assert_eq!(format, "openai");
        assert_eq!(base_url, MODEL_BASE_URL);
        assert_eq!(model, "auto");
    }

    #[test]
    fn suggested_never_uses_lowercase_deepseek_alias() {
        let (_, _, model) = suggested();
        assert!(!model.contains("deepseek"));
    }

    #[test]
    fn builds_select_accounts_url_with_prod_client_id() {
        let pkce = Pkce::generate();
        let url = authorization_url(&pkce, "machine-1");
        assert!(url.starts_with("https://qoder.cn/device/selectAccounts?"));
        assert!(url.contains("challenge_method=S256"));
        assert!(url.contains("nonce="));
        assert!(url.contains("machine_id=machine-1"));
        assert!(
            url.contains("client_id=e883ade2-e6e3-4d6d-adf7-f92ceff5fdcb"),
            "production client id must be used"
        );
    }

    #[test]
    fn machine_id_falls_back_to_uuid() {
        let first = recover_machine_id();
        let second = recover_machine_id();
        assert!(!first.is_empty());
        assert_ne!(first, second);
    }

    #[test]
    fn resolve_headers_match_inference_gateway_contract() {
        let _guard = super::super::tests::test_lock().blocking_lock();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            store::set_store_path_for_test(
                std::env::temp_dir()
                    .join(format!("bitfun-subauth-qoder-{}", uuid::Uuid::new_v4()))
                    .join("subscription_auth.json"),
            );
            store::upsert(
                STORE_KEY,
                StoredCredential::Oauth {
                    refresh: "r".to_string(),
                    access: "a".to_string(),
                    expires: now_ms() + 3_600_000,
                    account_id: Some("u-9".to_string()),
                    metadata: Some(serde_json::json!({ "uid": "u-9", "name": "qoder-user" })),
                },
            )
            .await
            .unwrap();
            let resolved = resolve(&SubscriptionHttpOptions::default())
                .await
                .expect("resolve credential");
            assert_eq!(resolved.api_key, "a");
            assert_eq!(resolved.extra_headers["Accept"], "text/event-stream");
            assert_eq!(resolved.extra_headers["Content-Type"], "application/json");
            assert!(resolved.extra_headers.contains_key("X-Request-ID"));
            assert!(resolved.extra_headers.contains_key("X-Session-ID"));
            assert!(!resolved.extra_headers.contains_key("X-Qoder-Model"));
            assert_eq!(
                resolved.request_url.as_deref(),
                Some("https://api2-v2.qoder.sh/model/v1/chat/completions")
            );
        });
    }
}
