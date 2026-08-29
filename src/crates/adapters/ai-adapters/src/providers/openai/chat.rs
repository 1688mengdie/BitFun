use super::{common, OpenAIMessageConverter};
use crate::client::quirks::should_append_tool_stream;
use crate::client::sse::execute_sse_request;
#[cfg(feature = "subscription-auth")]
use crate::client::sse::execute_sse_request_with_raw_body;
use crate::client::{AIClient, StreamResponse};
use crate::providers::shared;
use crate::stream::handle_openai_stream;
#[cfg(feature = "subscription-auth")]
use crate::stream::handle_qoder_stream;
use crate::trace::ModelExchangeTraceConfig;
use crate::types::{Message, ModelRequestContext, ToolDefinition};
#[cfg(not(feature = "subscription-auth"))]
use anyhow::anyhow;
use anyhow::Result;
use log::{debug, warn};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

fn try_build_request_body_with_context(
    client: &AIClient,
    url: &str,
    openai_messages: Vec<serde_json::Value>,
    openai_tools: Option<Vec<serde_json::Value>>,
    extra_body: Option<serde_json::Value>,
    request_context: Option<&ModelRequestContext>,
) -> Result<serde_json::Value> {
    let mut request_body = serde_json::json!({
        "model": client.config.model,
        "messages": openai_messages,
        "stream": true
    });

    let model_name = client.config.model.to_lowercase();

    if should_append_tool_stream(url, &model_name) {
        request_body["tool_stream"] = serde_json::Value::Bool(true);
    }

    let base_reasoning_fields = shared::capture_reasoning_fields(
        &request_body,
        &["thinking", "enable_thinking", "reasoning_effort"],
        &[],
    );

    if let Some(max_tokens) = client.config.max_tokens {
        request_body["max_tokens"] = serde_json::json!(max_tokens);
    }

    let protected_keys = &[
        "model",
        "messages",
        "stream",
        "max_tokens",
        "tool_stream",
        "tools",
    ];
    if let Some(preset) = client.model_reasoning_preset.as_ref() {
        shared::apply_reasoning_actions(
            preset,
            &mut request_body,
            protected_keys,
            &[],
            |action, body| {
                common::compile_chat_reasoning_action(
                    preset,
                    action,
                    body,
                    url,
                    &client.config.model,
                )
            },
        )?;
    }

    let protected_body = shared::protect_request_body(
        client,
        &mut request_body,
        &["model", "messages", "stream", "max_tokens", "tool_stream"],
        &[],
    );

    if let Some(extra) = extra_body {
        if let Some(extra_obj) = extra.as_object() {
            shared::merge_extra_body(&mut request_body, extra_obj);
            shared::log_extra_body_keys("ai::openai_stream_request", extra_obj);
        }
    }

    shared::restore_protected_body(&mut request_body, protected_body);
    if let Some(preset) = client.selected_reasoning_preset.as_ref() {
        shared::reset_reasoning_fields(
            &mut request_body,
            base_reasoning_fields.as_ref(),
            &["thinking", "enable_thinking", "reasoning_effort"],
            &[],
        );
        shared::apply_reasoning_actions(
            preset,
            &mut request_body,
            protected_keys,
            &[],
            |action, body| {
                common::compile_chat_reasoning_action(
                    preset,
                    action,
                    body,
                    url,
                    &client.config.model,
                )
            },
        )?;
    }

    if let Some(request_obj) = request_body.as_object_mut() {
        if let Some(existing_n) = request_obj.remove("n") {
            warn!(
                target: "ai::openai_stream_request",
                "Removed custom request field n={} because the stream processor only handles the first choice",
                existing_n
            );
        }
    }
    if let Some(schema) = request_context.and_then(|context| context.output_schema.as_ref()) {
        request_body["response_format"] = serde_json::json!({
            "type": "json_schema",
            "json_schema": {
                "name": "bitfun_output",
                "strict": true,
                "schema": schema
            }
        });
    }

    shared::log_request_body(
        "ai::openai_stream_request",
        "OpenAI stream request body (excluding tools):",
        &request_body,
    );

    common::attach_tools(&mut request_body, openai_tools, "ai::openai_stream_request");

    Ok(request_body)
}

pub(crate) fn try_build_request_body(
    client: &AIClient,
    url: &str,
    openai_messages: Vec<serde_json::Value>,
    openai_tools: Option<Vec<serde_json::Value>>,
    extra_body: Option<serde_json::Value>,
) -> Result<serde_json::Value> {
    try_build_request_body_with_context(
        client,
        url,
        openai_messages,
        openai_tools,
        extra_body,
        None,
    )
}

#[cfg(test)]
pub(crate) fn build_request_body(
    client: &AIClient,
    url: &str,
    openai_messages: Vec<serde_json::Value>,
    openai_tools: Option<Vec<serde_json::Value>>,
    extra_body: Option<serde_json::Value>,
) -> serde_json::Value {
    try_build_request_body(client, url, openai_messages, openai_tools, extra_body)
        .expect("request body should compile")
}

/// Generates a 32-character hex string suitable for CodeBuddy request IDs.
/// Uses a monotonic counter + timestamp hashed with SHA-256 for uniqueness.
static HEX32_COUNTER: AtomicU64 = AtomicU64::new(0);

fn generate_hex32() -> String {
    let count = HEX32_COUNTER.fetch_add(1, Ordering::Relaxed);
    let time_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut hasher = Sha256::new();
    hasher.update(count.to_le_bytes());
    hasher.update(time_nanos.to_le_bytes());
    let hash = hasher.finalize();
    hex::encode(&hash[..16])
}

#[cfg(test)]
pub(crate) fn build_request_body_with_context(
    client: &AIClient,
    url: &str,
    openai_messages: Vec<serde_json::Value>,
    openai_tools: Option<Vec<serde_json::Value>>,
    extra_body: Option<serde_json::Value>,
    request_context: Option<&ModelRequestContext>,
) -> serde_json::Value {
    try_build_request_body_with_context(
        client,
        url,
        openai_messages,
        openai_tools,
        extra_body,
        request_context,
    )
    .expect("request body should compile")
}

/// Collects the official CodeBuddy (`copilot.tencent.com`) conversation
/// fingerprint headers (mirrors the official CLI request assembly, see recon
/// report R-CB-CONVID 2026-08-21):
/// - `X-Conversation-ID`: session-stable (BitFun `session_id`).
/// - `X-Conversation-Request-ID`: turn-stable (one value per user prompt,
///   shared by every request/retry of that turn); falls back to a per-request
///   value only when the turn-level ID is unavailable.
/// - `X-Agent-Intent`/`X-Agent-Purpose`/`X-IDE-*`: official client fingerprint.
/// - `X-Private-Data`: model-optimization switch, always `"false"` (主人定标:
///   enableModelOptimization 必须关, never configurable).
/// - `X-IDE-Version`: defaults to the official CodeBuddy CLI version 2.141.0;
///   a configured `custom_headers` value overrides it so the version follows
///   the channel (CLI vs Workbuddy vs desktop).
///
/// Request-unique IDs (`X-Request-ID`/`X-Conversation-Message-ID`) are
/// appended by the caller so this function stays deterministic for tests.
fn codebuddy_fingerprint_headers(
    client: &AIClient,
    request_context: Option<&ModelRequestContext>,
) -> Vec<(&'static str, String)> {
    let mut headers: Vec<(&'static str, String)> = Vec::new();
    if let Some(ctx) = request_context {
        if let Some(sid) = &ctx.session_id {
            headers.push(("X-Conversation-ID", sid.clone()));
        }
        headers.push((
            "X-Conversation-Request-ID",
            ctx.conversation_request_id
                .clone()
                .unwrap_or_else(generate_hex32),
        ));
    } else {
        headers.push(("X-Conversation-Request-ID", generate_hex32()));
    }
    headers.push(("X-Agent-Intent", "craft".to_string()));
    // Official semantics: `X-Agent-Purpose` is only injected for
    // `person_agent` purpose; ordinary requests omit it. BitFun has no
    // person_agent mode, so the header is emitted only when explicitly
    // configured via `custom_headers`.
    if let Some(purpose) = configured_header(client, "X-Agent-Purpose") {
        headers.push(("X-Agent-Purpose", purpose));
    }
    // Official CLI client info (codebuddy.js module 33387 + clientInfoProvider):
    // PRODUCT_TYPE="CLI"; platform defaults to PRODUCT_TYPE; ideType follows
    // platform; ideName is an empty string in the CLI environment (recon
    // CB-DIAG-R3 §4.1 #6); version = CLI package version 2.141.0.
    headers.push(("X-IDE-Type", "CLI".to_string()));
    let ide_name = configured_header(client, "X-IDE-Name").unwrap_or_default();
    headers.push(("X-IDE-Name", ide_name));
    // Version follows the channel: default to the official CodeBuddy CLI
    // version, overridable per model entry via `custom_headers` (e.g.
    // Workbuddy 2.115.0 or a desktop version).
    let ide_version =
        configured_header(client, "X-IDE-Version").unwrap_or_else(|| "2.141.0".to_string());
    headers.push(("X-IDE-Version", ide_version));
    // Model-optimization switch: 主人定标 enableModelOptimization 必须关,
    // always send "false" — never configurable, never "true".
    headers.push(("X-Private-Data", "false".to_string()));
    // Identity headers resolved by the subscription auth layer (`X-User-Id`,
    // `X-Enterprise-Id`, `X-Tenant-Id`, `X-Department-Info`, `X-Userinfo`,
    // `X-Domain`). The official CLI injects them into every inference request
    // via `runIdentityHeaders()`; without them the gateway sees an anonymous
    // client (recon CB-DIAG-R3 §4.1 #7-#9). Emitted after the fixed headers so
    // they are appended last, matching the official assembly order.
    if let Some(custom_headers) = client.config.custom_headers.as_ref() {
        for name in CODEBUDDY_IDENTITY_HEADER_NAMES {
            if let Some(value) = custom_headers.get(*name) {
                headers.push((*name, value.clone()));
            }
        }
    }
    headers
}

/// Identity header names produced by the subscription auth layer
/// (`subscription_auth::codebuddy::resolve`) and consumed here for the
/// inference request. `custom_headers` is the transport the factory uses to
/// carry resolved credentials onto the runtime `AIConfig` (client_factory.rs).
const CODEBUDDY_IDENTITY_HEADER_NAMES: &[&str] = &[
    "X-User-Id",
    "X-Userinfo",
    "X-Enterprise-Id",
    "X-Tenant-Id",
    "X-Department-Info",
    "X-Domain",
];

/// Reads a CodeBuddy fingerprint header override from the model entry's
/// `custom_headers` (app.json `ai.models[].custom_headers`), so fingerprint
/// values stay runtime-configurable and never hard-coded.
fn configured_header(client: &AIClient, name: &str) -> Option<String> {
    client
        .config
        .custom_headers
        .as_ref()
        .and_then(|headers| headers.get(name).cloned())
}

/// Credential-bearing header names whose values must never reach the logs in
/// full; the log line keeps the first 8 characters and masks the rest.
const CODEBUDDY_SECRET_HEADER_NAMES: &[&str] = &[
    "Authorization",
    "X-API-Key",
    "X-Refresh-Token",
    "X-Verification-Code",
];

/// True when this header carries a credential value that must be masked.
fn is_secret_header_name(name: &str) -> bool {
    CODEBUDDY_SECRET_HEADER_NAMES
        .iter()
        .any(|secret| secret.eq_ignore_ascii_case(name))
}

/// Masks a credential header value: keeps the first 8 characters, replaces
/// everything beyond them with `***`. Values of 8 characters or fewer (and
/// empty values) are masked entirely so no full credential ever leaks.
fn mask_secret_header_value(name: &str, value: &str) -> String {
    if !is_secret_header_name(name) {
        return value.to_string();
    }
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 8 {
        return "***".to_string();
    }
    let prefix: String = chars[..8].iter().collect();
    format!("{prefix}***")
}

#[cfg(test)]
mod mask_header_tests {
    use super::{is_secret_header_name, mask_secret_header_value};

    #[test]
    fn codebuddy_mask_keeps_first_eight_chars_and_masks_rest() {
        assert_eq!(
            mask_secret_header_value("Authorization", "Bearer abcdefghSECRET"),
            "Bearer a***"
        );
        assert_eq!(
            mask_secret_header_value("X-API-Key", "ck_abcdefghSECRET"),
            "ck_abcde***"
        );
    }

    #[test]
    fn codebuddy_mask_hides_short_values_completely() {
        // A value of 8 characters or fewer must not keep its prefix: keeping
        // it would leak the full credential. A longer value keeps exactly the
        // first 8 characters per the masking rule.
        assert_eq!(mask_secret_header_value("X-API-Key", "ck_12345"), "***");
        assert_eq!(
            mask_secret_header_value("X-API-Key", "ck_12345678"),
            "ck_12345***"
        );
        assert_eq!(mask_secret_header_value("Authorization", ""), "***");
    }

    #[test]
    fn codebuddy_mask_matches_names_case_insensitively() {
        assert!(is_secret_header_name("authorization"));
        assert!(is_secret_header_name("x-api-key"));
        assert_eq!(
            mask_secret_header_value("x-api-key", "ck_abcdefghSECRET"),
            "ck_abcde***"
        );
    }

    #[test]
    fn codebuddy_mask_leaves_non_secret_values_untouched() {
        assert_eq!(mask_secret_header_value("X-IDE-Name", ""), "");
        assert_eq!(
            mask_secret_header_value("X-Conversation-ID", "sess-abc"),
            "sess-abc"
        );
    }
}

pub(crate) async fn send_stream(
    client: &AIClient,
    messages: Vec<Message>,
    tools: Option<Vec<ToolDefinition>>,
    extra_body: Option<serde_json::Value>,
    max_tries: usize,
    trace: Option<ModelExchangeTraceConfig>,
    request_context: Option<ModelRequestContext>,
) -> Result<StreamResponse> {
    let url = client.config.request_url.clone();
    debug!(
        "OpenAI config: model={}, request_url={}, max_tries={}",
        client.config.model, client.config.request_url, max_tries
    );

    let openai_messages = OpenAIMessageConverter::convert_messages(messages);
    let openai_tools = OpenAIMessageConverter::convert_tools(tools);
    let request_body = try_build_request_body_with_context(
        client,
        &url,
        openai_messages,
        openai_tools,
        extra_body,
        request_context.as_ref(),
    )?;
    let inline_think_in_text = client.config.inline_think_in_text;
    let idle_timeout = client.stream_options.idle_timeout;
    let ttft_timeout = client.stream_options.ttft_timeout;

    // Qoder's CN gateway rejects plain-Bearer requests (ALB 503); every
    // inference request must be signed by the embedded wasm
    // (`prepareInferRequest`), which returns a rewritten URL, COSY signature
    // headers, and an encrypted body. The response is a gateway envelope
    // (`data:{"body":"<OpenAI chunk>"}`) that `handle_qoder_stream` unwraps.
    if is_qoder_gateway(&url) {
        return send_qoder_signed_stream(
            client,
            url,
            request_body,
            max_tries,
            ttft_timeout,
            trace,
            inline_think_in_text,
            idle_timeout,
        )
        .await;
    }

    let header_url = url.clone();
    execute_sse_request(
        "OpenAI Streaming API",
        &url,
        &request_body,
        max_tries,
        ttft_timeout,
        trace,
        move || {
            let mut builder = common::apply_headers(client, client.client.post(&header_url));
            if header_url.contains("copilot.tencent.com") {
                let fingerprint = codebuddy_fingerprint_headers(client, request_context.as_ref());
                for (name, value) in &fingerprint {
                    builder = builder.header(*name, value);
                }
                let req_id = generate_hex32();
                builder = builder.header("X-Request-ID", req_id.clone());
                builder = builder.header("X-Conversation-Message-ID", req_id.clone());
                // Diagnostic header log (recon CB-DIAG-R4 gap #6): the final
                // assembled header set was never observed at runtime. Log the
                // complete header face — shared transport headers plus this
                // closure's fingerprint/request-id additions — with credential
                // values masked, only for the CodeBuddy gateway domain.
                let mut logged: Vec<(String, String)> = common::log_header_face(&builder);
                for (name, value) in &fingerprint {
                    logged.push(((*name).to_string(), value.clone()));
                }
                logged.push(("X-Request-ID".to_string(), req_id.clone()));
                logged.push(("X-Conversation-Message-ID".to_string(), req_id));
                for (name, value) in &logged {
                    debug!(
                        "CodeBuddy request header: {}={}",
                        name,
                        mask_secret_header_value(name, value)
                    );
                }
            }
            builder
        },
        move |response, tx, tx_raw, remaining_ttft_timeout| {
            handle_openai_stream(
                response,
                tx,
                tx_raw,
                inline_think_in_text,
                remaining_ttft_timeout,
                idle_timeout,
            )
        },
    )
    .await
}

/// True when the request URL targets the Qoder CN or international gateway,
/// which requires wasm-signed inference requests.
fn is_qoder_gateway(url: &str) -> bool {
    url.contains("gateway.qoder.com.cn") || url.contains("api2-v2.qoder.sh")
}

/// Sends a Qoder inference request through the wasm-signed channel.
#[cfg(feature = "subscription-auth")]
#[allow(clippy::too_many_arguments)]
async fn send_qoder_signed_stream(
    client: &AIClient,
    _url: String,
    request_body: serde_json::Value,
    max_tries: usize,
    ttft_timeout: Option<Duration>,
    trace: Option<ModelExchangeTraceConfig>,
    inline_think_in_text: bool,
    idle_timeout: Option<Duration>,
) -> Result<StreamResponse> {
    let options = crate::subscription_auth::SubscriptionHttpOptions::default();
    let model_key = &client.config.model;
    let (signed_url, signed_headers, signed_body) =
        crate::subscription_auth::sign_qoder_infer_request(&options, &request_body, model_key)
            .await?;
    debug!("Qoder signed infer url: {}", signed_url);

    let url = signed_url;
    let trace_url = url.clone();
    execute_sse_request_with_raw_body(
        "Qoder Streaming API",
        &trace_url,
        &request_body,
        Some(signed_body),
        max_tries,
        ttft_timeout,
        trace,
        move || {
            let mut builder = client.client.post(&url);
            for (name, value) in &signed_headers {
                builder = builder.header(name, value);
            }
            builder
        },
        move |response, tx, tx_raw, remaining_ttft_timeout| {
            handle_qoder_stream(
                response,
                tx,
                tx_raw,
                inline_think_in_text,
                remaining_ttft_timeout,
                idle_timeout,
            )
        },
    )
    .await
}

#[cfg(not(feature = "subscription-auth"))]
#[allow(clippy::too_many_arguments)]
async fn send_qoder_signed_stream(
    _client: &AIClient,
    _url: String,
    _request_body: serde_json::Value,
    _max_tries: usize,
    _ttft_timeout: Option<Duration>,
    _trace: Option<ModelExchangeTraceConfig>,
    _inline_think_in_text: bool,
    _idle_timeout: Option<Duration>,
) -> Result<StreamResponse> {
    Err(anyhow!(
        "Qoder inference requires the subscription-auth feature"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AIConfig;
    #[cfg(feature = "subscription-auth")]
    use base64::Engine as _;

    #[test]
    fn generate_hex32_produces_32_char_hex() {
        let id = generate_hex32();
        assert_eq!(id.len(), 32, "hex32 must be exactly 32 characters");
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit()),
            "hex32 must contain only hex digits, got: {id}"
        );
    }

    #[test]
    fn generate_hex32_unique_per_call() {
        let a = generate_hex32();
        let b = generate_hex32();
        assert_ne!(a, b, "consecutive hex32 calls must produce distinct values");
    }

    #[test]
    fn codebuddy_url_detection() {
        assert!("https://copilot.tencent.com/v1/chat/completions".contains("copilot.tencent.com"));
        assert!(!"https://api.openai.com/v1/chat/completions".contains("copilot.tencent.com"));
        assert!(!"https://gateway.qoder.com.cn/v1".contains("copilot.tencent.com"));
    }

    fn ctx_with_ids() -> ModelRequestContext {
        ModelRequestContext {
            prompt_cache_route_key: Some("route-1".to_string()),
            session_id: Some("sess-abc".to_string()),
            conversation_request_id: Some("turn-xyz".to_string()),
            output_schema: None,
        }
    }

    fn test_client() -> AIClient {
        AIClient::new(AIConfig {
            name: "codebuddy-test".to_string(),
            base_url: "https://copilot.tencent.com/v1".to_string(),
            request_url: "https://copilot.tencent.com/v1/chat/completions".to_string(),
            api_key: "test-key".to_string(),
            model: "deepseek-v4-flash".to_string(),
            format: "openai".to_string(),
            context_window: 128000,
            max_tokens: None,
            temperature: None,
            top_p: None,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            custom_request_body: None,
            custom_request_body_mode: None,
        })
    }

    #[test]
    fn codebuddy_fingerprint_headers_conversation_request_id_is_turn_stable() {
        let client = test_client();
        let ctx = ctx_with_ids();
        let first = codebuddy_fingerprint_headers(&client, Some(&ctx));
        let second = codebuddy_fingerprint_headers(&client, Some(&ctx));
        let get = |headers: &[(&'static str, String)], name: &str| {
            headers
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.clone())
                .unwrap()
        };
        // Same turn -> identical X-Conversation-Request-ID across requests.
        assert_eq!(
            get(&first, "X-Conversation-Request-ID"),
            get(&second, "X-Conversation-Request-ID")
        );
        assert_eq!(get(&first, "X-Conversation-Request-ID"), "turn-xyz");
        // Different turns -> different values.
        let other = ModelRequestContext {
            conversation_request_id: Some("turn-other".to_string()),
            ..ctx
        };
        assert_ne!(
            get(&first, "X-Conversation-Request-ID"),
            get(
                &codebuddy_fingerprint_headers(&client, Some(&other)),
                "X-Conversation-Request-ID"
            )
        );
    }

    #[test]
    fn codebuddy_fingerprint_headers_includes_full_official_set() {
        let client = test_client();
        let headers = codebuddy_fingerprint_headers(&client, Some(&ctx_with_ids()));
        let names: Vec<&str> = headers.iter().map(|(n, _)| *n).collect();
        for expected in [
            "X-Conversation-ID",
            "X-Conversation-Request-ID",
            "X-Agent-Intent",
            "X-IDE-Type",
            "X-IDE-Name",
            "X-IDE-Version",
            "X-Private-Data",
        ] {
            assert!(
                names.contains(&expected),
                "missing fingerprint header: {expected}"
            );
        }
        let get = |name: &str| {
            headers
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.as_str())
                .unwrap()
        };
        assert_eq!(get("X-Conversation-ID"), "sess-abc");
        assert_eq!(get("X-Agent-Intent"), "craft");
        // X-Agent-Purpose is a conditional header: not emitted for ordinary
        // requests, only when explicitly configured via custom_headers.
        assert!(
            !names.contains(&"X-Agent-Purpose"),
            "X-Agent-Purpose must be omitted by default"
        );
        assert_eq!(get("X-IDE-Type"), "CLI");
        // X-IDE-Name is an empty string in the official CLI environment
        // (recon CB-DIAG-R3 §4.1 #6); overridable via custom_headers.
        assert_eq!(get("X-IDE-Name"), "");
        // Official CodeBuddy CLI version (package.json of
        // @tencent-ai/codebuddy-code 2.141.0), NOT BitFun's own version.
        assert_eq!(get("X-IDE-Version"), "2.141.0");
        // X-Product/X-Product-Version are NOT sent: the official CLI /v2
        // inference assembly never emits them (recon CB-DIAG-R3 §4.1 #3-#4).
        assert!(
            !names.contains(&"X-Product"),
            "X-Product must not be sent on /v2 inference"
        );
        assert!(
            !names.contains(&"X-Product-Version"),
            "X-Product-Version must not be sent on /v2 inference"
        );
        // Model-optimization switch defaults to "false" (official default).
        assert_eq!(get("X-Private-Data"), "false");
        // X-Requested-With is NOT sent: the official /v2 inference assembly
        // never emits it (recon CB-DIAG-R3 §4.1 #5).
        assert!(
            !names.contains(&"X-Requested-With"),
            "X-Requested-With must not be sent on /v2 inference"
        );
        // Identity headers come only from the subscription auth layer; with a
        // bare custom_headers config they stay absent.
        assert!(headers.iter().all(|(n, _)| *n != "X-User-Id"));
        assert!(headers.iter().all(|(n, _)| *n != "X-Userinfo"));
        assert!(headers.iter().all(|(n, _)| *n != "X-Domain"));
        // No duplicate header names.
        let mut sorted = names.clone();
        sorted.sort_unstable();
        let mut deduped = sorted.clone();
        deduped.dedup();
        assert_eq!(sorted, deduped, "header names must not repeat");
    }

    #[test]
    fn codebuddy_fingerprint_headers_custom_headers_override_fingerprint_values() {
        let mut client = test_client();
        client.config.custom_headers = Some(
            [
                ("X-IDE-Version".to_string(), "2.115.0".to_string()),
                ("X-Agent-Purpose".to_string(), "person_agent".to_string()),
            ]
            .into_iter()
            .collect(),
        );
        let headers = codebuddy_fingerprint_headers(&client, Some(&ctx_with_ids()));
        let get = |name: &str| {
            headers
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.as_str())
                .unwrap()
        };
        // Channel-scoped version override (Workbuddy) wins over the CLI default.
        assert_eq!(get("X-IDE-Version"), "2.115.0");
        // X-Agent-Purpose is injected only when configured.
        assert_eq!(get("X-Agent-Purpose"), "person_agent");
    }

    #[test]
    fn codebuddy_fingerprint_headers_custom_headers_override_ide_name() {
        let mut client = test_client();
        client.config.custom_headers = Some(
            [("X-IDE-Name".to_string(), "Workbuddy".to_string())]
                .into_iter()
                .collect(),
        );
        let headers = codebuddy_fingerprint_headers(&client, Some(&ctx_with_ids()));
        let get = |name: &str| {
            headers
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.as_str())
                .unwrap()
        };
        // Channel-scoped IDE name override wins over the empty CLI default.
        assert_eq!(get("X-IDE-Name"), "Workbuddy");
    }

    #[test]
    #[cfg(feature = "subscription-auth")]
    fn codebuddy_fingerprint_headers_include_identity_headers_from_auth_layer() {
        // E3: the subscription auth layer resolves identity headers
        // (X-User-Id etc.) and the factory carries them on custom_headers; the
        // fingerprint layer must append them to the inference request.
        let mut client = test_client();
        client.config.custom_headers = Some(
            [
                ("X-User-Id".to_string(), "u-123".to_string()),
                (
                    "X-Userinfo".to_string(),
                    base64::engine::general_purpose::STANDARD
                        .encode(r#"{"uin":"u-123","owner_uin":"ent-9"}"#),
                ),
                ("X-Enterprise-Id".to_string(), "ent-9".to_string()),
                ("X-Tenant-Id".to_string(), "ent-9".to_string()),
                ("X-Domain".to_string(), "copilot.tencent.com".to_string()),
            ]
            .into_iter()
            .collect(),
        );
        let headers = codebuddy_fingerprint_headers(&client, Some(&ctx_with_ids()));
        let get = |name: &str| {
            headers
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.as_str())
                .unwrap()
        };
        assert_eq!(get("X-User-Id"), "u-123");
        assert_eq!(
            get("X-Userinfo"),
            base64::engine::general_purpose::STANDARD
                .encode(r#"{"uin":"u-123","owner_uin":"ent-9"}"#)
        );
        assert_eq!(get("X-Enterprise-Id"), "ent-9");
        assert_eq!(get("X-Tenant-Id"), "ent-9");
        assert_eq!(get("X-Domain"), "copilot.tencent.com");
        // Department header absent when the auth layer did not produce one.
        assert!(headers.iter().all(|(n, _)| *n != "X-Department-Info"));
        // No duplicate header names once identity headers are appended.
        let mut sorted = headers.iter().map(|(n, _)| *n).collect::<Vec<_>>();
        sorted.sort_unstable();
        let mut deduped = sorted.clone();
        deduped.dedup();
        assert_eq!(sorted, deduped, "header names must not repeat");
    }

    #[test]
    fn codebuddy_fingerprint_headers_private_data_is_always_false() {
        // 主人定标: enableModelOptimization 必须关 — X-Private-Data is always
        // "false" and must ignore any custom_headers override.
        let mut client = test_client();
        client.config.custom_headers = Some(
            [("X-Private-Data".to_string(), "true".to_string())]
                .into_iter()
                .collect(),
        );
        let headers = codebuddy_fingerprint_headers(&client, Some(&ctx_with_ids()));
        let get = |name: &str| {
            headers
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.as_str())
                .unwrap()
        };
        assert_eq!(get("X-Private-Data"), "false");
    }

    #[test]
    fn codebuddy_fingerprint_headers_never_emit_product_headers() {
        // Even with an explicit custom_headers override, X-Product and
        // X-Product-Version must stay absent from /v2 inference requests:
        // the fingerprint layer owns the header set, and the shared reserved
        // list (providers/shared.rs) drops these keys before they could be
        // re-applied by the generic custom_headers channel.
        let mut client = test_client();
        client.config.custom_headers = Some(
            [
                ("X-Product".to_string(), "SaaS".to_string()),
                ("X-Product-Version".to_string(), "2.115.0".to_string()),
            ]
            .into_iter()
            .collect(),
        );
        let headers = codebuddy_fingerprint_headers(&client, Some(&ctx_with_ids()));
        assert!(headers.iter().all(|(n, _)| *n != "X-Product"));
        assert!(headers.iter().all(|(n, _)| *n != "X-Product-Version"));
    }

    #[test]
    fn codebuddy_fingerprint_headers_falls_back_when_context_missing() {
        let client = test_client();
        let headers = codebuddy_fingerprint_headers(&client, None);
        let get = |name: &str| {
            headers
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.as_str())
                .unwrap()
        };
        // Request-ID fallback still produced; fingerprint headers still present.
        assert_eq!(get("X-Conversation-Request-ID").len(), 32);
        assert!(headers.iter().all(|(n, _)| *n != "X-Conversation-ID"));
        assert_eq!(get("X-Agent-Intent"), "craft");
    }
}
