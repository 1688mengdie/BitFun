//! gbrain tool — taiji structured RAG knowledge base (W2-3).
//!
//! Bridges BitFun to the taiji-server gbrain RPC domain over the 9527 JSON-RPC
//! channel (`taiji_rpc_call` in quant_tools.rs is crate-private, so this file
//! re-implements the same std-TCP one-shot transport — zero new dependencies,
//! identical semantics: same address, same timeout, same error mapping).
//!
//! gbrain = structured RAG knowledge base (pages / chunks / embeddings / hybrid
//! retrieval / per-agent namespace isolation / JSON persistence). This is
//! **complementary, not overlapping** with the existing `KnowledgeBaseSearch`
//! tool (full-text file search over the L0/L1/L3/L4 knowledge base directory):
//! the tool descriptions reference each other to avoid confusion.
//!
//! Write actions (put/delete/ingest) are non-readonly; read actions
//! (get/list/search) are read-only. Deferred exposure (multi-action).

use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{Read, Write};

/// Default taiji-server JSON-RPC endpoint (`host:port` form; HTTP path fixed).
const DEFAULT_TAIJI_RPC_ADDR: &str = "127.0.0.1:9527";
const TAIJI_RPC_PATH: &str = "/api/rpc";
const RPC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn rpc_addr() -> String {
    std::env::var("TAIJI_QUANT_RPC_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_TAIJI_RPC_ADDR.to_string())
}

/// One-shot JSON-RPC call to taiji-server over std TCP (spawn_blocking so the
/// async runtime is never blocked). Mirrors quant_tools::taiji_rpc_call.
async fn gbrain_rpc_call(method: &str, params: Value) -> BitFunResult<Value> {
    let addr = rpc_addr();
    let method = method.to_string();
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    })
    .to_string();

    let response = tokio::task::spawn_blocking(move || {
        let addr: std::net::SocketAddr = addr
            .parse()
            .map_err(|_| "invalid taiji RPC address".to_string())?;
        let mut stream = std::net::TcpStream::connect_timeout(&addr, RPC_TIMEOUT)
            .map_err(|_| format!("taiji-server is not reachable at {addr}"))?;
        stream
            .set_read_timeout(Some(RPC_TIMEOUT))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(RPC_TIMEOUT))
            .map_err(|e| e.to_string())?;
        let request = format!(
            "POST {} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            TAIJI_RPC_PATH,
            body.len(),
            body
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|e| e.to_string())?;
        let mut buffer = Vec::new();
        stream
            .read_to_end(&mut buffer)
            .map_err(|e| e.to_string())?;
        // Split headers from body on the first empty line.
        let text = String::from_utf8_lossy(&buffer);
        let body_start = text
            .find("\r\n\r\n")
            .map(|index| index + 4)
            .unwrap_or(0);
        let payload = &text[body_start.min(text.len())..];
        serde_json::from_str::<Value>(payload)
            .map_err(|e| format!("taiji-server returned malformed JSON: {e}"))
    })
    .await
    .map_err(|e| BitFunError::tool(format!("taiji RPC task failed: {e}")))??;

    if let Some(error) = response.get("error") {
        let code = error.get("code").and_then(Value::as_i64).unwrap_or(-32000);
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("taiji RPC error");
        return Err(BitFunError::tool(format!(
            "taiji RPC '{method}' failed [{code}]: {message}"
        )));
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| BitFunError::tool("taiji RPC returned no result".to_string()))
}

fn gbrain_tool_result(data: Value) -> Vec<ToolResult> {
    vec![ToolResult::Result {
        data,
        result_for_assistant: None,
        image_attachments: None,
    }]
}

// ── Inputs ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GbrainInput {
    /// One of "put" | "get" | "delete" | "list" | "search" | "ingest".
    pub action: String,
    /// Page slug (put/get/delete/ingest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// Page title (put/ingest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Page content (put/ingest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Source id override (put/ingest; default agent namespace).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Tags (put/ingest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Metadata object (put/ingest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    /// Search query (search).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Search top_k (search; default 10, capped 50).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u64>,
    /// Chunk strategy (ingest; paragraph | sentence | fixed:size:overlap).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_strategy: Option<String>,
    /// List limit (list; default 100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
}

/// `gbrain` — taiji structured RAG knowledge base actions via JSON-RPC.
pub struct GbrainTool;

impl Default for GbrainTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GbrainTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GbrainTool {
    fn name(&self) -> &str {
        "gbrain"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Manage the taiji structured RAG knowledge base (taiji-server JSON-RPC `gbrain.*`).

This is the structured knowledge base (pages / chunks / embeddings / hybrid retrieval / per-agent namespace isolation / JSON persistence), complementing the full-text file search tool "KnowledgeBaseSearch" (which scans the L0/L1/L3/L4 knowledge base directory by keyword). Use "gbrain" for structured document memory (e.g. per-agent notes, research findings, strategy memos) and "KnowledgeBaseSearch" for full-text grep over the knowledge base files.

Actions:
- "put": Create or update a page. Requires slug, title, content. Optional source_id (defaults to the calling agent namespace), tags, metadata.
- "get": Read a page by slug. Returns the page or null when missing.
- "delete": Delete a page (and its chunks) by slug. Returns deleted: true/false.
- "list": List all pages (newest first). Optional limit (default 100).
- "search": Hybrid keyword/vector retrieval. Requires query. Optional top_k (default 10, capped 50). Search is namespace-isolated: only the calling agent's pages are returned.
- "ingest": Write a page and auto chunk + embed its content. Requires slug, title, content. Optional chunk_strategy (paragraph | sentence | fixed:size:overlap; default paragraph).

Arguments:
- "action": Required. One of "put", "get", "delete", "list", "search", "ingest".
- "slug": Required for put/get/delete/ingest.
- "title" / "content": Required for put/ingest.
- "query": Required for search.
- "chunk_strategy": Optional for ingest.
- "top_k" / "limit": Optional caps.
- "source_id" / "tags" / "metadata": Optional metadata for put/ingest."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Manage the taiji structured RAG knowledge base (gbrain) via JSON-RPC.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["put", "get", "delete", "list", "search", "ingest"],
                    "description": "gbrain action to perform."
                },
                "slug": {
                    "type": "string",
                    "description": "Page slug; required for put/get/delete/ingest."
                },
                "title": {
                    "type": "string",
                    "description": "Page title; required for put/ingest."
                },
                "content": {
                    "type": "string",
                    "description": "Page content; required for put/ingest."
                },
                "source_id": {
                    "type": "string",
                    "description": "Source id override; defaults to the calling agent namespace."
                },
                "tags": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional page tags (put/ingest)."
                },
                "metadata": {
                    "type": "object",
                    "description": "Optional page metadata (put/ingest)."
                },
                "query": {
                    "type": "string",
                    "description": "Search query; required for search."
                },
                "top_k": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Search top_k (default 10, capped 50)."
                },
                "chunk_strategy": {
                    "type": "string",
                    "enum": ["paragraph", "sentence"],
                    "description": "Chunk strategy for ingest (default paragraph)."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "List limit (default 100)."
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        // 写操作（put/delete/ingest）→ 非只读；读操作（get/list/search）→ 只读
        // 由调用方 action 区分（工具级统一非只读，避免写操作被只读语义拦截）。
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: GbrainInput = match serde_json::from_value(input.clone()) {
            Ok(value) => value,
            Err(error) => {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid input: {}", error)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };
        let non_empty = |v: &Option<String>| {
            v.as_deref()
                .map(str::trim)
                .is_some_and(|s| !s.is_empty())
        };
        let result = match parsed.action.as_str() {
            "put" | "ingest" => {
                non_empty(&parsed.slug) && non_empty(&parsed.title) && non_empty(&parsed.content)
            }
            "get" | "delete" => non_empty(&parsed.slug),
            "search" => non_empty(&parsed.query),
            "list" => true,
            _ => false,
        };
        ValidationResult {
            result,
            message: if result {
                None
            } else {
                Some(
                    "action 'put'/'ingest' need slug+title+content; 'get'/'delete' need slug; 'search' needs query; 'list' takes no arguments"
                        .to_string(),
                )
            },
            error_code: if result { None } else { Some(400) },
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let action = input
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        format!("Run taiji gbrain '{}'", action)
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: GbrainInput = serde_json::from_value(input.clone())
            .map_err(|error| BitFunError::tool(format!("Invalid input: {}", error)))?;

        let mut map = serde_json::Map::new();
        if let Some(slug) = params.slug {
            map.insert("slug".to_string(), json!(slug));
        }
        if let Some(title) = params.title {
            map.insert("title".to_string(), json!(title));
        }
        if let Some(content) = params.content {
            map.insert("content".to_string(), json!(content));
        }
        if let Some(source_id) = params.source_id {
            map.insert("source_id".to_string(), json!(source_id));
        }
        if let Some(tags) = params.tags {
            map.insert(
                "tags".to_string(),
                json!(tags.iter().filter(|s| !s.trim().is_empty()).collect::<Vec<_>>()),
            );
        }
        if let Some(metadata) = params.metadata {
            map.insert("metadata".to_string(), metadata);
        }
        if let Some(query) = params.query {
            map.insert("query".to_string(), json!(query));
        }
        if let Some(top_k) = params.top_k {
            map.insert("top_k".to_string(), json!(top_k));
        }
        if let Some(chunk_strategy) = params.chunk_strategy {
            map.insert("chunk_strategy".to_string(), json!(chunk_strategy));
        }
        if let Some(limit) = params.limit {
            map.insert("limit".to_string(), json!(limit));
        }

        let method = match params.action.as_str() {
            "put" => "gbrain.put",
            "get" => "gbrain.get",
            "delete" => "gbrain.delete",
            "list" => "gbrain.list",
            "search" => "gbrain.search",
            "ingest" => "gbrain.ingest",
            other => {
                return Err(BitFunError::tool(format!(
                    "unknown gbrain action '{}'; expected put/get/delete/list/search/ingest",
                    other
                )));
            }
        };
        let result = gbrain_rpc_call(method, Value::Object(map)).await?;
        Ok(gbrain_tool_result(json!({
            "success": true,
            "method": method,
            "channel": "rpc",
            "response": result,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name_matches_registered_contract() {
        assert_eq!(GbrainTool::new().name(), "gbrain");
    }

    #[tokio::test]
    async fn gbrain_validation_requires_action_arguments() {
        let tool = GbrainTool::new();

        let put_ok = tool
            .validate_input(
                &json!({ "action": "put", "slug": "s", "title": "t", "content": "c" }),
                None,
            )
            .await;
        assert!(put_ok.result);

        let put_missing_content = tool
            .validate_input(&json!({ "action": "put", "slug": "s", "title": "t" }), None)
            .await;
        assert!(!put_missing_content.result);

        let get_ok = tool
            .validate_input(&json!({ "action": "get", "slug": "s" }), None)
            .await;
        assert!(get_ok.result);

        let get_missing_slug = tool
            .validate_input(&json!({ "action": "get" }), None)
            .await;
        assert!(!get_missing_slug.result);

        let search_ok = tool
            .validate_input(&json!({ "action": "search", "query": "量化" }), None)
            .await;
        assert!(search_ok.result);

        let search_missing_query = tool
            .validate_input(&json!({ "action": "search" }), None)
            .await;
        assert!(!search_missing_query.result);

        let list_ok = tool.validate_input(&json!({ "action": "list" }), None).await;
        assert!(list_ok.result);

        let ingest_ok = tool
            .validate_input(
                &json!({ "action": "ingest", "slug": "s", "title": "t", "content": "c" }),
                None,
            )
            .await;
        assert!(ingest_ok.result);

        let unknown = tool
            .validate_input(&json!({ "action": "bogus" }), None)
            .await;
        assert!(!unknown.result);
    }
}
