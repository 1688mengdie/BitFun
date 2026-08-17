//! CodeBuddy subscription login and credential resolution.
//!
//! Aligned with the official CodeBuddy desktop client: a private auth API flow
//! against `copilot.tencent.com`. The client asks the server for an auth
//! state, opens the login page in the browser (which internally redirects to
//! Keycloak with `client_id=console`), and polls for the resulting tokens.
//! The Keycloak token endpoint is never called directly; external clients get
//! a permanent `401 unauthorized_client` there. Gateway requests authenticate
//! with `Authorization: Bearer {accessToken}` plus a set of identity headers.

use super::store::{self, StoredCredential};
use super::{ResolvedCredential, StartedLogin, SubscriptionHttpOptions};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

const API_BASE_URL: &str = "https://copilot.tencent.com";
const PLATFORM: &str = "CodeBuddyIDE";
const DOMAIN: &str = "copilot.tencent.com";
const STORE_KEY: &str = "codebuddy";
const REFRESH_LEEWAY_MS: i64 = 5 * 60 * 1000;
const POLL_INTERVAL_MS: u64 = 2000;

/// Token payload returned by the CodeBuddy private auth API. The desktop
/// client reads `data.data` from every response; the same nesting applies
/// here.
#[derive(Debug, Deserialize)]
struct AuthTokenResponse {
    data: AuthTokenData,
}

#[derive(Debug, Deserialize)]
struct AuthTokenData {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "refreshToken")]
    refresh_token: String,
    #[serde(rename = "expiresIn", default)]
    expires_in: Option<i64>,
}

/// Account payload returned by `GET /v2/plugin/login/account?state=`.
#[derive(Debug, Deserialize)]
struct AuthAccountResponse {
    data: AuthAccountData,
}

#[derive(Debug, Deserialize)]
struct AuthAccountData {
    #[serde(default)]
    uid: Option<String>,
    #[serde(default)]
    nickname: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(rename = "enterpriseId", default)]
    enterprise_id: Option<String>,
    #[serde(rename = "departmentFullName", default)]
    department_full_name: Option<String>,
}

/// Response of `GET /v2/plugin/auth/token` while the user has not finished
/// logging in. The official client keeps polling on these codes.
#[derive(Debug, Deserialize)]
struct TokenPendingError {
    code: Option<i64>,
}

fn http_client(options: &SubscriptionHttpOptions) -> Result<reqwest::Client> {
    super::build_http_client(options, "CodeBuddy")
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Step 1: request an auth state and the browser login URL.
async fn request_auth_state(options: &SubscriptionHttpOptions) -> Result<(String, String)> {
    let client = http_client(options)?;
    let resp = client
        .post(format!(
            "{API_BASE_URL}/v2/plugin/auth/state?platform={PLATFORM}"
        ))
        .send()
        .await
        .context("call codebuddy auth state endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "codebuddy auth state request failed: HTTP {status}: {body}"
        ));
    }
    let payload: AuthStateResponse = resp
        .json()
        .await
        .context("parse codebuddy auth state response")?;
    Ok((payload.data.state, payload.data.auth_url))
}

#[derive(Debug, Deserialize)]
struct AuthStateResponse {
    data: AuthStateData,
}

#[derive(Debug, Deserialize)]
struct AuthStateData {
    state: String,
    #[serde(rename = "authUrl")]
    auth_url: String,
}

/// Step 3: poll the private token endpoint until the user finishes the login.
async fn poll_for_token(
    state: &str,
    cancel: &CancellationToken,
    options: &SubscriptionHttpOptions,
) -> Result<AuthTokenData> {
    let client = http_client(options)?;
    loop {
        let resp = client
            .get(format!("{API_BASE_URL}/v2/plugin/auth/token?state={state}"))
            .send()
            .await
            .context("call codebuddy auth token endpoint")?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::BAD_REQUEST {
            // The login is not complete yet; the official client keeps
            // polling until its deadline.
            tokio::select! {
                _ = cancel.cancelled() => return Err(anyhow!("login cancelled")),
                _ = tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)) => {}
            }
            continue;
        }
        if let Ok(payload) = serde_json::from_str::<TokenPendingError>(&body) {
            // The official client (`RetryFetchToken = 11217`) keeps polling
            // while the login is still in progress.
            if matches!(payload.code, Some(11217)) {
                tokio::select! {
                    _ = cancel.cancelled() => return Err(anyhow!("login cancelled")),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)) => {}
                }
                continue;
            }
        }
        if !status.is_success() {
            return Err(anyhow!(
                "codebuddy auth token request failed: HTTP {status}: {body}"
            ));
        }
        let payload: AuthTokenResponse =
            serde_json::from_str(&body).context("parse codebuddy auth token response")?;
        return Ok(payload.data);
    }
}

/// Step 4: fetch the signed-in account so identity headers can be resolved.
async fn fetch_account(
    state: &str,
    access_token: &str,
    options: &SubscriptionHttpOptions,
) -> Result<AuthAccountData> {
    let client = http_client(options)?;
    let resp = client
        .get(format!(
            "{API_BASE_URL}/v2/plugin/login/account?state={state}"
        ))
        .bearer_auth(access_token)
        .send()
        .await
        .context("call codebuddy login account endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "codebuddy login account request failed: HTTP {status}: {body}"
        ));
    }
    let payload: AuthAccountResponse = resp
        .json()
        .await
        .context("parse codebuddy login account response")?;
    Ok(payload.data)
}

fn account_metadata(account: &AuthAccountData) -> Option<serde_json::Value> {
    let uid = account.uid.clone();
    let nickname = account.nickname.clone();
    let email = account.email.clone();
    let enterprise_id = account.enterprise_id.clone();
    let department = account.department_full_name.clone();
    if uid.is_none() && nickname.is_none() && email.is_none() && enterprise_id.is_none() {
        return None;
    }
    let mut object = serde_json::Map::new();
    if let Some(uid) = uid {
        object.insert("uid".to_string(), serde_json::Value::String(uid));
    }
    if let Some(nickname) = nickname {
        object.insert("nickname".to_string(), serde_json::Value::String(nickname));
    }
    if let Some(email) = email {
        object.insert("email".to_string(), serde_json::Value::String(email));
    }
    if let Some(enterprise_id) = enterprise_id {
        object.insert(
            "enterprise_id".to_string(),
            serde_json::Value::String(enterprise_id),
        );
    }
    if let Some(department) = department {
        object.insert(
            "department_full_name".to_string(),
            serde_json::Value::String(department),
        );
    }
    Some(serde_json::Value::Object(object))
}

async fn persist_tokens(
    tokens: AuthTokenData,
    account: AuthAccountData,
    expected_revision: u64,
) -> Result<()> {
    let expires = now_ms() + tokens.expires_in.unwrap_or(3600) * 1000;
    let account_id = account.uid.clone();
    let metadata = account_metadata(&account);
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        expected_revision,
        StoredCredential::Oauth {
            refresh: tokens.refresh_token,
            access: tokens.access_token,
            expires,
            account_id,
            metadata,
        },
    )
    .await?;
    super::require_current_store_revision(super::SubscriptionProvider::CodeBuddy, outcome)?;
    log::info!("codebuddy subscription tokens saved");
    Ok(())
}

/// Starts the private auth API login flow. The browser URL is returned
/// immediately; the runner polls for the token in the background.
pub(crate) async fn begin_login(
    cancel: CancellationToken,
    expected_revision: u64,
    options: SubscriptionHttpOptions,
) -> Result<StartedLogin> {
    let (state, authorization_url) = request_auth_state(&options).await?;

    let runner = async move {
        let cancel = cancel.clone();
        super::authorize_then_persist(
            super::SubscriptionProvider::CodeBuddy,
            cancel.clone(),
            async {
                let tokens = poll_for_token(&state, &cancel, &options).await?;
                // Account lookup is best-effort; identity headers are only
                // emitted when metadata is present, and the account is
                // fetched again lazily during refresh.
                let account = fetch_account(&state, &tokens.access_token, &options).await;
                let account = account.unwrap_or(AuthAccountData {
                    uid: None,
                    nickname: None,
                    email: None,
                    enterprise_id: None,
                    department_full_name: None,
                });
                Ok((tokens, account))
            },
            move |(tokens, account)| persist_tokens(tokens, account, expected_revision),
        )
        .await
    };

    Ok(StartedLogin {
        authorization_url,
        user_code: None,
        instructions: "Complete authorization in your browser, then return to BitFun.".to_string(),
        runner: Box::pin(runner),
    })
}

/// Loads the stored credential, refreshing the access token when it is about
/// to expire. Returns `(access, account_id, expires_ms)`.
async fn ensure_fresh(options: &SubscriptionHttpOptions) -> Result<(String, Option<String>, i64)> {
    let snapshot = store::load_entry_with_revision(STORE_KEY).await?;
    let entry = snapshot
        .credential
        .ok_or_else(|| anyhow!("CodeBuddy is not connected; sign in first"))?;
    let StoredCredential::Oauth {
        refresh: refresh_token,
        access,
        expires,
        account_id,
        metadata,
    } = entry
    else {
        return Err(anyhow!("CodeBuddy credential is not an OAuth login"));
    };

    if expires > now_ms() + REFRESH_LEEWAY_MS {
        return Ok((access, account_id, expires));
    }

    let refreshed = refresh(&refresh_token, options).await?;
    let new_access = refreshed.access_token;
    let new_refresh = refreshed.refresh_token;
    let new_expires = now_ms() + refreshed.expires_in.unwrap_or(3600) * 1000;
    let new_account_id = account_id;
    let new_metadata = metadata;
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        snapshot.revision,
        StoredCredential::Oauth {
            refresh: new_refresh,
            access: new_access.clone(),
            expires: new_expires,
            account_id: new_account_id.clone(),
            metadata: new_metadata,
        },
    )
    .await?;
    match outcome {
        store::ConditionalCommitOutcome::Committed { .. } => {
            log::info!("codebuddy subscription tokens refreshed");
            Ok((new_access, new_account_id, new_expires))
        }
        store::ConditionalCommitOutcome::Conflict { current_revision } => {
            let current = super::load_current_store_after_conflict(
                super::SubscriptionProvider::CodeBuddy,
                current_revision,
            )
            .await?;
            match current.credential {
                Some(StoredCredential::Oauth {
                    access,
                    expires,
                    account_id,
                    ..
                }) if expires > now_ms() => {
                    log::info!("codebuddy refresh reused tokens committed by a concurrent refresh");
                    Ok((access, account_id, expires))
                }
                _ => Err(super::store_revision_conflict(
                    super::SubscriptionProvider::CodeBuddy,
                    current_revision,
                )),
            }
        }
    }
}

/// Refreshes the CodeBuddy credential through the private refresh endpoint.
async fn refresh(refresh_token: &str, options: &SubscriptionHttpOptions) -> Result<AuthTokenData> {
    let client = http_client(options)?;
    let resp = client
        .post(format!("{API_BASE_URL}/v2/plugin/auth/token/refresh"))
        .header("X-Refresh-Token", refresh_token)
        .header("X-Auth-Refresh-Source", "plugin")
        .send()
        .await
        .context("call codebuddy auth token refresh endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "codebuddy token refresh failed: HTTP {status}: {body}"
        ));
    }
    let payload: AuthTokenResponse = resp
        .json()
        .await
        .context("parse codebuddy refresh response")?;
    Ok(payload.data)
}

/// Resolves the runtime credential, injecting the CodeBuddy identity headers.
///
/// Mirrors the official desktop client's `buildAuthHeaders`: `X-User-Id` is
/// the signed-in account's `uid`, `X-Enterprise-Id` + `X-Tenant-Id` are the
/// account's `enterpriseId` (same value), `X-Department-Info` is the account's
/// `departmentFullName`, and `X-Domain` is the product domain. Conditional
/// headers are only emitted when the corresponding account metadata exists.
pub(crate) async fn resolve(options: &SubscriptionHttpOptions) -> Result<ResolvedCredential> {
    let (access, _account_id, expires) = ensure_fresh(options).await?;
    let mut headers = HashMap::new();
    let metadata = store::load_entry(STORE_KEY)
        .await?
        .and_then(|entry| match entry {
            StoredCredential::Oauth { metadata, .. } => metadata,
            StoredCredential::Api { metadata, .. } => metadata,
        });
    let metadata_map = metadata.and_then(|value| value.as_object().cloned());
    // X-User-Id: account.uid (stored from the login account fetch).
    if let Some(uid) = metadata_map
        .as_ref()
        .and_then(|map| map.get("uid"))
        .and_then(|value| value.as_str())
    {
        headers.insert("X-User-Id".to_string(), uid.to_string());
    }
    // X-Enterprise-Id + X-Tenant-Id: account.enterpriseId, both set to the
    // same value when present (official `buildAuthHeaders`).
    if let Some(enterprise_id) = metadata_map
        .as_ref()
        .and_then(|map| map.get("enterprise_id"))
        .and_then(|value| value.as_str())
    {
        headers.insert("X-Enterprise-Id".to_string(), enterprise_id.to_string());
        headers.insert("X-Tenant-Id".to_string(), enterprise_id.to_string());
    }
    // X-Department-Info: account.departmentFullName when present.
    if let Some(department) = metadata_map
        .as_ref()
        .and_then(|map| map.get("department_full_name"))
        .and_then(|value| value.as_str())
    {
        headers.insert("X-Department-Info".to_string(), department.to_string());
    }
    // X-Domain: always the codebuddy product domain.
    headers.insert("X-Domain".to_string(), DOMAIN.to_string());

    Ok(ResolvedCredential {
        api_key: access,
        base_url: Some(API_BASE_URL.to_string()),
        request_url: None,
        format: None,
        extra_headers: headers,
        expires_at: Some(expires / 1000),
    })
}

/// Provider metadata used to seed a new model entry.
pub(crate) fn suggested() -> (&'static str, &'static str, &'static str) {
    ("openai", API_BASE_URL, "codebuddy")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_model_and_format_are_stable() {
        let (format, base_url, model) = suggested();
        assert_eq!(format, "openai");
        assert_eq!(base_url, API_BASE_URL);
        assert!(!model.is_empty());
    }

    #[test]
    fn account_metadata_keeps_only_present_fields() {
        let account = AuthAccountData {
            uid: Some("u-123".to_string()),
            nickname: Some("coder".to_string()),
            email: None,
            enterprise_id: Some("ent-9".to_string()),
            department_full_name: Some("R&D".to_string()),
        };
        let metadata = account_metadata(&account).expect("metadata present");
        assert_eq!(metadata["uid"], "u-123");
        assert_eq!(metadata["enterprise_id"], "ent-9");
        assert_eq!(metadata["department_full_name"], "R&D");
        assert!(metadata.get("email").is_none());
    }

    #[test]
    fn resolve_headers_use_metadata_conditions() {
        let _guard = super::super::tests::test_lock().blocking_lock();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            store::set_store_path_for_test(
                std::env::temp_dir()
                    .join(format!("bitfun-subauth-codebuddy-{}", uuid::Uuid::new_v4()))
                    .join("subscription_auth.json"),
            );
            store::upsert(
                STORE_KEY,
                StoredCredential::Oauth {
                    refresh: "r".to_string(),
                    access: "a".to_string(),
                    expires: now_ms() + 3_600_000,
                    account_id: Some("u-123".to_string()),
                    metadata: Some(serde_json::json!({
                        "uid": "u-123",
                        "enterprise_id": "ent-9",
                        "department_full_name": "R&D"
                    })),
                },
            )
            .await
            .unwrap();
            let resolved = resolve(&SubscriptionHttpOptions::default())
                .await
                .expect("resolve credential");
            assert_eq!(resolved.api_key, "a");
            assert_eq!(resolved.extra_headers["X-User-Id"], "u-123");
            assert_eq!(resolved.extra_headers["X-Enterprise-Id"], "ent-9");
            assert_eq!(resolved.extra_headers["X-Tenant-Id"], "ent-9");
            assert_eq!(resolved.extra_headers["X-Domain"], DOMAIN);
            assert_eq!(resolved.extra_headers["X-Department-Info"], "R&D");
        });
    }

    #[test]
    fn resolve_skips_absent_enterprise_headers() {
        let _guard = super::super::tests::test_lock().blocking_lock();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            store::set_store_path_for_test(
                std::env::temp_dir()
                    .join(format!(
                        "bitfun-subauth-codebuddy-nope-{}",
                        uuid::Uuid::new_v4()
                    ))
                    .join("subscription_auth.json"),
            );
            store::upsert(
                STORE_KEY,
                StoredCredential::Oauth {
                    refresh: "r".to_string(),
                    access: "a".to_string(),
                    expires: now_ms() + 3_600_000,
                    account_id: None,
                    metadata: Some(serde_json::json!({ "uid": "u-1" })),
                },
            )
            .await
            .unwrap();
            let resolved = resolve(&SubscriptionHttpOptions::default())
                .await
                .expect("resolve credential");
            assert_eq!(resolved.extra_headers["X-User-Id"], "u-1");
            assert!(!resolved.extra_headers.contains_key("X-Enterprise-Id"));
            assert!(!resolved.extra_headers.contains_key("X-Tenant-Id"));
            assert!(!resolved.extra_headers.contains_key("X-Department-Info"));
            assert_eq!(resolved.extra_headers["X-Domain"], DOMAIN);
        });
    }
}
