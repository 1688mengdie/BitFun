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
/// OpenAPI host used by the device-token poll, PAT exchange and refresh
/// endpoints. Matches the Qoder CN CLI (`@qodercn-ai/qoderclicn`) which
/// targets `qoder.com.cn` for China-region accounts. The international
/// production host (`openapi.qoder.sh`) is used by the global `qodercli`;
/// this adapter is aligned with the CN variant.
const OPENAPI_URL: &str = "https://openapi.qoder.com.cn";
const CLIENT_ID: &str = "e883ade2-e6e3-4d6d-adf7-f92ceff5fdcb";
/// Production inference host for China-region accounts (Qoder CN CLI
/// endpoint cache: `gateway.qoder.com.cn`).
const MODEL_BASE_URL: &str = "https://gateway.qoder.com.cn";
const MODEL_REQUEST_URL: &str = "https://gateway.qoder.com.cn/model/v1/chat/completions";
const DEFAULT_MODEL: &str = "auto";
const STORE_KEY: &str = "qoder";
/// Marker prefix stored in the credential `refresh` field for PAT-based
/// logins, mirroring pi-free's `pat|...` encoding. The PAT itself is a
/// long-lived personal access token that must be exchanged for a short-lived
/// job token before inference; on expiry the PAT is re-exchanged.
const PAT_REFRESH_PREFIX: &str = "pat";
const REFRESH_LEEWAY_MS: i64 = 5 * 60 * 1000;
const POLL_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const POLL_RETRY_MS: Duration = Duration::from_secs(1);

/// Response of the PAT → job-token exchange endpoint.
///
/// Mirrors the Qoder CN CLI (`exchangePersonalToken`): the response carries
/// the job token as `token` (or `device_token`/`access_token`), plus expiry
/// fields. The PAT itself is retained for re-exchange on expiry, so the
/// exchange's own `refresh_token` is intentionally not stored.
#[derive(Debug, Deserialize)]
struct JobTokenResponse {
    #[serde(rename = "token", default)]
    token: Option<String>,
    #[serde(rename = "device_token", default)]
    device_token: Option<String>,
    #[serde(rename = "access_token", default)]
    access_token: Option<String>,
    #[serde(rename = "expires_at", default)]
    expires_at: Option<RefreshExpiry>,
    #[serde(rename = "expires_in", default)]
    expires_in: Option<i64>,
}

impl JobTokenResponse {
    fn access(&self) -> Option<String> {
        self.token
            .clone()
            .or_else(|| self.device_token.clone())
            .or_else(|| self.access_token.clone())
    }

    /// Absolute expiry in epoch milliseconds.
    fn expires_ms(&self) -> i64 {
        if let Some(expires_at) = self.expires_at.as_ref() {
            return refresh_expiry_to_ms(expires_at);
        }
        match self.expires_in {
            Some(seconds) if seconds > 0 => now_ms() + seconds * 1000,
            _ => now_ms() + 3600 * 1000,
        }
    }
}

/// Response of the device-token poll endpoint.
#[derive(Debug, Deserialize)]
struct DeviceTokenResponse {
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    /// Relative expiry in seconds (fallback when `expires_at` is absent).
    #[serde(default)]
    expires_in: Option<i64>,
    /// Absolute expiry timestamp (RFC 3339 string or epoch ms), preferred over
    /// `expires_in` when present.
    #[serde(default)]
    expires_at: Option<RefreshExpiry>,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    user_name: Option<String>,
}

/// Response of the device-token refresh endpoint.
///
/// The CLI (`refreshDeviceCredential`) maps the refresh response directly:
/// `security_oauth_token = device_token`, `refresh_token`, and the expiry
/// timestamps are read from `expires_at` / `refresh_token_expires_at`.
#[derive(Debug, Deserialize)]
struct RefreshTokenResponse {
    #[serde(rename = "device_token")]
    device_token: String,
    #[serde(rename = "refresh_token", default)]
    refresh_token: Option<String>,
    #[serde(rename = "expires_at", default)]
    expires_at: Option<RefreshExpiry>,
}

/// Absolute expiry timestamp returned by the refresh endpoint. The CLI's
/// `vq()` accepts an RFC 3339 string, an epoch-seconds number, or an
/// epoch-milliseconds number.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RefreshExpiry {
    Str(String),
    Num(i64),
}

impl RefreshTokenResponse {
    /// Converts the access-token expiry to absolute epoch milliseconds.
    fn expires_at_ms(&self) -> i64 {
        self.expires_at
            .as_ref()
            .map(refresh_expiry_to_ms)
            .unwrap_or_else(|| now_ms() + 3600 * 1000)
    }
}

/// Normalizes a refresh expiry value to absolute epoch milliseconds, mirroring
/// the CLI's `vq()`: RFC 3339 strings are parsed to epoch seconds; numbers
/// larger than `1e12` are treated as milliseconds, otherwise as seconds.
fn refresh_expiry_to_ms(value: &RefreshExpiry) -> i64 {
    match value {
        RefreshExpiry::Str(text) => {
            let seconds = chrono::DateTime::parse_from_rfc3339(text)
                .map(|date| date.timestamp())
                .unwrap_or_else(|_| now_ms() / 1000);
            seconds * 1000
        }
        RefreshExpiry::Num(number) => {
            if *number > 1_000_000_000_000 {
                *number
            } else {
                *number * 1000
            }
        }
    }
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

/// Builds the `selectAccounts` authorization URL.
///
/// `nonce` is shared with the device-token poll: the server associates the
/// browser authorization with this nonce, and the client polls using the same
/// value. The CLI (`rVa`) keeps one nonce throughout both phases.
fn authorization_url(pkce: &Pkce, machine_id: &str, nonce: &str) -> String {
    format!(
        "{BASE_URL}/device/selectAccounts?challenge={}&challenge_method=S256&nonce={}&machine_id={}&client_id={}",
        pkce.challenge, nonce, machine_id, CLIENT_ID
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
    let authorization_url = authorization_url(&pkce, &machine_id, &nonce);
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

/// Resolves the poll-response expiry to absolute epoch milliseconds. Prefers
/// the absolute `expires_at` timestamp (RFC 3339 string or epoch ms), falling
/// back to `expires_in` relative seconds, then a 1-hour default.
fn token_expiry(tokens: &DeviceTokenResponse) -> i64 {
    if let Some(expires_at) = tokens.expires_at.as_ref() {
        return refresh_expiry_to_ms(expires_at);
    }
    match tokens.expires_in {
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
    let expires = token_expiry(&tokens);
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

fn openapi_base_url() -> &'static str {
    // Test override hook: the refresh integration test points the refresh
    // endpoint at a local mock server. In production builds the override is
    // never set, so the hard-coded production endpoint is always used.
    if let Some(override_url) = openapi_base_override() {
        return override_url;
    }
    OPENAPI_URL
}

fn openapi_base_override() -> Option<&'static str> {
    *openapi_base_override_slot().lock().unwrap()
}

#[cfg_attr(not(test), allow(dead_code))]
fn set_openapi_base_override(url: Option<String>) {
    let leaked = url.map(|value| Box::leak(value.into_boxed_str()) as &'static str);
    *openapi_base_override_slot().lock().unwrap() = leaked;
}

fn openapi_base_override_slot() -> &'static std::sync::Mutex<Option<&'static str>> {
    static OVERRIDE: std::sync::OnceLock<std::sync::Mutex<Option<&'static str>>> =
        std::sync::OnceLock::new();
    OVERRIDE.get_or_init(|| std::sync::Mutex::new(None))
}

async fn refresh(
    refresh_token: &str,
    options: &SubscriptionHttpOptions,
) -> Result<RefreshTokenResponse> {
    let client = http_client(options)?;
    let resp = client
        .post(format!("{}/api/v1/deviceToken/refresh", openapi_base_url()))
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

/// Exchanges a Qoder Personal Access Token for a short-lived job token.
///
/// Mirrors the Qoder CN CLI (`exchangePersonalToken`): `POST
/// {openapi}/api/v1/jobToken/exchange` with `{ "personal_token": pat }`.
/// The returned job token is what inference requests use as Bearer — the PAT
/// itself is not accepted by the inference gateway.
async fn exchange_pat(pat: &str, options: &SubscriptionHttpOptions) -> Result<JobTokenResponse> {
    let client = http_client(options)?;
    let resp = client
        .post(format!("{}/api/v1/jobToken/exchange", openapi_base_url()))
        .json(&serde_json::json!({ "personal_token": pat }))
        .send()
        .await
        .context("call qoder job token exchange endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "qoder job token exchange failed: HTTP {status}: {body}"
        ));
    }
    resp.json()
        .await
        .context("parse qoder job token exchange response")
}

/// Logs in with a Qoder Personal Access Token: exchanges it for a job token
/// and persists the credential. The PAT itself is kept in the `refresh`
/// field (prefixed with `PAT_REFRESH_PREFIX`) so expiry can re-exchange it.
pub(crate) async fn pat_login(
    pat: &str,
    expected_revision: u64,
    options: &SubscriptionHttpOptions,
) -> Result<()> {
    let pat = pat.trim();
    if pat.is_empty() {
        return Err(anyhow!("Qoder personal access token is empty"));
    }
    let exchanged = exchange_pat(pat, options).await?;
    let access = exchanged
        .access()
        .ok_or_else(|| anyhow!("qoder job token exchange response missing token"))?;
    let refresh = format!("{PAT_REFRESH_PREFIX}|{pat}");
    let expires = exchanged.expires_ms();
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        expected_revision,
        StoredCredential::Oauth {
            refresh,
            access,
            expires,
            account_id: None,
            metadata: None,
        },
    )
    .await?;
    super::require_current_store_revision(super::SubscriptionProvider::Qoder, outcome)?;
    log::info!("qoder PAT login saved (job token exchanged)");
    Ok(())
}

/// Returns `(pat, job_token)` when the stored credential is PAT-based, or
/// `None` for device-flow credentials.
fn pat_parts(refresh: &str) -> Option<(&str, &str)> {
    refresh.strip_prefix(PAT_REFRESH_PREFIX).and_then(|rest| {
        let rest = rest.strip_prefix('|')?;
        let (pat, job_token) = rest.split_once('|')?;
        Some((pat, job_token))
    })
}

/// Loads the stored credential, refreshing the access token when it is about
/// to expire or when `force` is set. `force` mirrors the CLI's
/// `forceRefreshToken`: it is used after a 401/403 response so the request can
/// be retried with a fresh token. Returns `(access, expires_ms)`.
async fn ensure_fresh(options: &SubscriptionHttpOptions, force: bool) -> Result<(String, i64)> {
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

    if !force && expires > now_ms() + REFRESH_LEEWAY_MS {
        return Ok((access, expires));
    }

    // PAT-based login refreshes by re-exchanging the stored PAT (the Qoder CN
    // CLI `refreshPatCredential`); device-flow credentials use the
    // device-token refresh endpoint.
    if let Some((pat, _old_job_token)) = pat_parts(&refresh_token) {
        if pat.is_empty() {
            return Err(anyhow!("Qoder credential has no personal access token"));
        }
        let exchanged = exchange_pat(pat, options).await?;
        let new_access = exchanged
            .access()
            .ok_or_else(|| anyhow!("qoder job token exchange response missing token"))?;
        let new_expires = exchanged.expires_ms();
        let outcome = store::upsert_if_revision(
            STORE_KEY,
            snapshot.revision,
            StoredCredential::Oauth {
                refresh: format!("{PAT_REFRESH_PREFIX}|{pat}|{new_access}"),
                access: new_access.clone(),
                expires: new_expires,
                account_id,
                metadata,
            },
        )
        .await?;
        return match outcome {
            store::ConditionalCommitOutcome::Committed { .. } => {
                log::info!("qoder PAT credential re-exchanged");
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
                        log::info!(
                            "qoder PAT refresh reused tokens committed by a concurrent refresh"
                        );
                        Ok((access, expires))
                    }
                    _ => Err(super::store_revision_conflict(
                        super::SubscriptionProvider::Qoder,
                        current_revision,
                    )),
                }
            }
        };
    }

    if refresh_token.is_empty() {
        return Err(anyhow!("Qoder credential has no refresh token"));
    }

    let refreshed = refresh(&refresh_token, options).await?;
    let new_access = refreshed.device_token.clone();
    let new_refresh = refreshed.refresh_token.clone().unwrap_or(refresh_token);
    let new_expires = refreshed.expires_at_ms();
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        snapshot.revision,
        StoredCredential::Oauth {
            refresh: new_refresh,
            access: new_access.clone(),
            expires: new_expires,
            account_id,
            metadata,
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
///
/// The `gateway.qoder.com.cn` inference gateway is standard OpenAI-compatible
/// and authenticates with a plain `Authorization: Bearer {job token}` (the
/// Qoder CN CLI `openResponse`). No `Cosy-*` client-attribution headers are
/// required on this endpoint.
pub(crate) async fn resolve(options: &SubscriptionHttpOptions) -> Result<ResolvedCredential> {
    let (access, expires) = ensure_fresh(options, false).await?;
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

/// Forces a token refresh (equivalent to the CLI's `forceRefreshToken`) and
/// persists the rotated credential. Called after a 401/403 inference response
/// so the next request retries with a fresh token.
pub(crate) async fn refresh_profile(options: &SubscriptionHttpOptions) -> Result<()> {
    ensure_fresh(options, true).await?;
    Ok(())
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
        let url = authorization_url(&pkce, "machine-1", "nonce-1");
        assert!(url.starts_with("https://qoder.cn/device/selectAccounts?"));
        assert!(url.contains("challenge_method=S256"));
        assert!(url.contains("nonce=nonce-1"));
        assert!(url.contains("machine_id=machine-1"));
        assert!(
            url.contains("client_id=e883ade2-e6e3-4d6d-adf7-f92ceff5fdcb"),
            "production client id must be used"
        );
    }

    #[test]
    fn authorization_url_and_poll_share_the_same_nonce() {
        // The device flow associates the browser authorization with a nonce
        // and polls using that same nonce (CLI `rVa` keeps one nonce across
        // both phases). The URL must carry exactly the nonce the runner polls
        // with, otherwise the server never matches the token to this client.
        let pkce = Pkce::generate();
        let nonce = Uuid::new_v4().to_string();
        let url = authorization_url(&pkce, "machine-1", &nonce);
        let poll_url = format!(
            "{OPENAPI_URL}/api/v1/deviceToken/poll?nonce={nonce}&verifier={}&challenge_method=S256",
            pkce.verifier
        );
        assert!(url.contains(&format!("nonce={nonce}")));
        assert!(poll_url.contains(&format!("nonce={nonce}")));
        assert_eq!(
            url.split("nonce=")
                .nth(1)
                .unwrap()
                .split('&')
                .next()
                .unwrap(),
            nonce
        );
        assert_eq!(
            poll_url
                .split("nonce=")
                .nth(1)
                .unwrap()
                .split('&')
                .next()
                .unwrap(),
            nonce
        );
    }

    #[test]
    fn refresh_response_uses_cli_device_token_fields() {
        // CLI `refreshDeviceCredential` maps the refresh response as
        // `security_oauth_token = device_token`, `refresh_token`, and reads
        // expiries from `expires_at` / `refresh_token_expires_at`.
        let payload = serde_json::json!({
            "device_token": "device-token-1",
            "refresh_token": "refresh-token-1",
            "expires_at": "2026-09-01T00:00:00+00:00",
            "refresh_token_expires_at": "2026-12-01T00:00:00+00:00"
        });
        let parsed: RefreshTokenResponse = serde_json::from_value(payload).unwrap();
        assert_eq!(parsed.device_token, "device-token-1");
        assert_eq!(parsed.refresh_token.as_deref(), Some("refresh-token-1"));
        let ms = parsed.expires_at_ms();
        assert!(ms > now_ms());
    }

    #[test]
    fn refresh_expiry_normalizes_seconds_and_milliseconds() {
        let seconds = RefreshExpiry::Num(1_800_000_000);
        assert_eq!(refresh_expiry_to_ms(&seconds), 1_800_000_000_000);
        let milliseconds = RefreshExpiry::Num(1_800_000_000_000);
        assert_eq!(refresh_expiry_to_ms(&milliseconds), 1_800_000_000_000);
    }

    #[test]
    fn ensure_fresh_without_force_reuses_a_valid_credential() {
        // Contract guard: without `force`, an unexpired credential must not
        // trigger a network refresh (401/403 force-refresh only runs after a
        // failed inference attempt).
        let _guard = super::super::tests::test_lock().blocking_lock();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            store::set_store_path_for_test(
                std::env::temp_dir()
                    .join(format!(
                        "bitfun-subauth-qoder-fresh-{}",
                        uuid::Uuid::new_v4()
                    ))
                    .join("subscription_auth.json"),
            );
            store::upsert(
                STORE_KEY,
                StoredCredential::Oauth {
                    refresh: "r".to_string(),
                    access: "fresh-access".to_string(),
                    expires: now_ms() + 3_600_000,
                    account_id: None,
                    metadata: None,
                },
            )
            .await
            .unwrap();
            // `refresh()` would fail against the real endpoint; reaching it
            // here would make this test error. Reusing the stored token is the
            // expected outcome.
            let (access, _) = ensure_fresh(&SubscriptionHttpOptions::default(), false)
                .await
                .expect("fresh credential reused without refresh");
            assert_eq!(access, "fresh-access");
        });
    }

    #[test]
    fn ensure_fresh_with_force_attempts_a_network_refresh() {
        // Contract guard: `force` must bypass the freshness check and call the
        // refresh endpoint even for an unexpired credential. The refresh call
        // targets the real openapi host, which is unreachable in unit tests, so
        // the expected outcome is a transport error (proving the force path
        // attempted the refresh) rather than a silent reuse of the stored token.
        let _guard = super::super::tests::test_lock().blocking_lock();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            store::set_store_path_for_test(
                std::env::temp_dir()
                    .join(format!(
                        "bitfun-subauth-qoder-forced-{}",
                        uuid::Uuid::new_v4()
                    ))
                    .join("subscription_auth.json"),
            );
            store::upsert(
                STORE_KEY,
                StoredCredential::Oauth {
                    refresh: "r".to_string(),
                    access: "stale-access".to_string(),
                    expires: now_ms() + 3_600_000,
                    account_id: None,
                    metadata: None,
                },
            )
            .await
            .unwrap();
            let outcome = ensure_fresh(&SubscriptionHttpOptions::default(), true).await;
            match outcome {
                Err(error) => {
                    let text = error.to_string();
                    assert!(
                        text.contains("refresh") || text.contains("send request"),
                        "force refresh must reach the network refresh path, got: {text}"
                    );
                }
                Ok(_) => panic!("force refresh must not reuse the stored token"),
            }
        });
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
            assert!(!resolved.extra_headers.contains_key("Cosy-ClientType"));
            assert!(!resolved.extra_headers.contains_key("Cosy-Version"));
            assert!(!resolved.extra_headers.contains_key("Cosy-MachineOS"));
            assert!(!resolved.extra_headers.contains_key("X-Qoder-Model"));
            assert_eq!(
                resolved.request_url.as_deref(),
                Some("https://gateway.qoder.com.cn/model/v1/chat/completions")
            );
        });
    }

    #[tokio::test]
    async fn force_refresh_rotates_credential_and_resolve_returns_new_token() {
        // Integration contract: after a 401/403 the force-refresh must rotate
        // the credential in the store, and the next resolve (which backs the
        // rebuilt client's Authorization header) must return the new token.
        let _guard = super::super::tests::test_lock().lock().await;
        store::set_store_path_for_test(
            std::env::temp_dir()
                .join(format!(
                    "bitfun-subauth-qoder-rotate-{}",
                    uuid::Uuid::new_v4()
                ))
                .join("subscription_auth.json"),
        );

        // Local mock of the device-token refresh endpoint returning a rotated
        // device_token (CLI field shape: device_token/refresh_token/expires_at).
        let app = axum::Router::new().route(
            "/api/v1/deviceToken/refresh",
            axum::routing::post(|| async {
                axum::Json(serde_json::json!({
                    "device_token": "rotated-token-9",
                    "refresh_token": "rotated-refresh-9",
                    "expires_at": "2099-01-01T00:00:00+00:00"
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind qoder refresh fixture");
        let address = listener
            .local_addr()
            .expect("qoder refresh fixture address");
        let server_task = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("qoder refresh fixture should run");
        });
        set_openapi_base_override(Some(format!("http://{address}")));

        store::upsert(
            STORE_KEY,
            StoredCredential::Oauth {
                refresh: "old-refresh".to_string(),
                access: "old-token".to_string(),
                expires: now_ms() + 3_600_000,
                account_id: None,
                metadata: None,
            },
        )
        .await
        .unwrap();

        // force refresh must rotate the stored credential.
        refresh_profile(&SubscriptionHttpOptions::default())
            .await
            .expect("force refresh should succeed against the mock");
        let stored = store::load_entry(STORE_KEY)
            .await
            .unwrap()
            .expect("credential present");
        match stored {
            StoredCredential::Oauth {
                access, refresh, ..
            } => {
                assert_eq!(access, "rotated-token-9");
                assert_eq!(refresh, "rotated-refresh-9");
            }
            _ => panic!("expected oauth credential"),
        }

        // The next resolve (backing a rebuilt client's Authorization header)
        // must return the rotated token.
        let resolved = resolve(&SubscriptionHttpOptions::default())
            .await
            .expect("resolve rotated credential");
        assert_eq!(resolved.api_key, "rotated-token-9");

        server_task.abort();
        set_openapi_base_override(None);
    }
}
