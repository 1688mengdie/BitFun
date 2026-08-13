//! taiji-quant tool family (RAD06 Phase 1).
//!
//! Bridges BitFun to the taiji-lvpa quant engine over the two channels the
//! RAD06 plan fixed as the primary path:
//!
//! - `quant_backtest` runs through the real ACP bridge (`AcpClientPort` →
//!   `AcpClientService` → spawn `taiji acp` → `run_backtest`), matching the
//!   "ACP 主通道" design and exercising the standard ACP handshake against the
//!   taiji-lvpa server (commit 5218859 adapter layer).
//! - `quote` / `strategy` / `order` call the taiji-server JSON-RPC endpoint
//!   (`http://127.0.0.1:9527/api/rpc`) directly over std TCP, which is the
//!   "MCP 互补" side of the plan: stateless one-shot quant queries do not need
//!   an ACP session. Zero new dependencies (std TCP + `tokio::task::spawn_blocking`,
//!   mirroring the taiji-lvpa `delegate_to_server_blocking` pattern).
//!
//! Every call is a true bridge to the taiji quant engine — never a local model
//! simulation. The default taiji server URL can be overridden with the
//! `TAIJI_QUANT_RPC_URL` environment variable (format `host:port`).

use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::coordination::get_global_coordinator;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_runtime_ports::{AcpClientPort, AcpClientCreateRequest, AcpClientMessageRequest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

/// Registered ACP client id for the taiji-lvpa quant engine (`taiji acp`).
pub(crate) const TAIJI_QUANT_CLIENT_ID: &str = "taiji-quant";

/// Default taiji-server JSON-RPC endpoint (`host:port` form; HTTP path is fixed
/// to `/api/rpc`, matching taiji-desktop's delegate transport).
const DEFAULT_TAIJI_RPC_ADDR: &str = "127.0.0.1:9527";
const TAIJI_RPC_PATH: &str = "/api/rpc";
const RPC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

// ── Inputs ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct QuantBacktestInput {
    /// BacktestConfig YAML (instruments/date_range/initial_capital/...).
    pub config: String,
    /// CSV data (OHLC rows) to backtest against.
    pub csv_data: String,
    /// Optional PipelineConfig YAML (name/version/bar_gen/data_source/nodes).
    /// Passed to the engine as a real `pipeline_template` file so the full
    /// double-layer contract (BacktestConfig → pipeline_template → PipelineConfig)
    /// is exercised. When omitted, `config` is used as the pipeline content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct QuantQuoteInput {
    /// Instrument symbol, e.g. "a2609".
    pub symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct QuantStrategyInput {
    /// One of "backtest" | "report".
    pub action: String,
    /// CSV path for backtest (strategy.backtest requires csv_path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csv_path: Option<String>,
    /// Backtest report id (strategy.report requires backtest_id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backtest_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct QuantOrderInput {
    /// One of "create" | "list" | "cancel" | "modify" | "trade_history".
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// "buy" | "sell" (create).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    /// Quantity in integer lots (create).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qty: Option<f64>,
    /// Limit price (create, limit orders).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<f64>,
    /// "market" | "limit" (create).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_type: Option<String>,
    /// Order id (cancel/modify).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
}

// ── Shared helpers ──────────────────────────────────────────────────────────

fn resolve_acp_client_port() -> BitFunResult<Arc<dyn AcpClientPort>> {
    let coordinator = get_global_coordinator()
        .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
    coordinator.acp_client_port().ok_or_else(|| {
        BitFunError::tool("ACP client port is not available; the desktop host did not inject it".to_string())
    })
}

fn workspace_or_context(
    workspace_param: Option<&str>,
    context: &ToolUseContext,
) -> BitFunResult<String> {
    if let Some(workspace) = workspace_param
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(workspace.to_string());
    }
    context
        .workspace_root()
        .map(|path| path.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            BitFunError::tool(
                "workspace_path is required when the current workspace is unavailable".to_string(),
            )
        })
}

fn rpc_addr() -> String {
    std::env::var("TAIJI_QUANT_RPC_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_TAIJI_RPC_ADDR.to_string())
}

/// One-shot JSON-RPC call to taiji-server over std TCP (`spawn_blocking` so the
/// async runtime is never blocked). Mirrors taiji-lvpa's delegate transport:
/// `POST /api/rpc` with `{"jsonrpc":"2.0","id":1,"method","params"}`.
async fn taiji_rpc_call(method: &str, params: Value) -> BitFunResult<Value> {
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
        use std::io::{Read, Write};
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

/// Start (or reuse) an ACP session for `taiji-quant` in `workspace` and forward
/// one prompt through the real ACP channel. Returns the external response text.
async fn acp_quant_prompt(
    message: &str,
    workspace_path: &str,
    timeout_seconds: Option<u64>,
    session_name: &str,
) -> BitFunResult<String> {
    let port = resolve_acp_client_port()?;

    // Create a fresh flow session per call so every tool invocation is a real
    // `taiji acp` spawn + standard ACP handshake (true bridge, non-mock).
    let created = port
        .create_session(AcpClientCreateRequest {
            client_id: TAIJI_QUANT_CLIENT_ID.to_string(),
            workspace_path: workspace_path.to_string(),
            session_name: Some(session_name.to_string()),
            remote_connection_id: None,
        })
        .await
        .map_err(|error| {
            BitFunError::tool(format!(
                "failed to start taiji-quant ACP session: {}",
                error.message
            ))
        })?;

    let sent = port
        .send_message(AcpClientMessageRequest {
            session_id: created.session_id.clone(),
            message: message.to_string(),
            workspace_path: Some(workspace_path.to_string()),
            timeout_seconds,
        })
        .await
        .map_err(|error| {
            BitFunError::tool(format!(
                "failed to run quant prompt through taiji acp: {}",
                error.message
            ))
        })?;

    Ok(sent.response)
}

fn quant_tool_result(data: Value) -> Vec<ToolResult> {
    vec![ToolResult::Result {
        data,
        result_for_assistant: None,
        image_attachments: None,
    }]
}

// ── quant_backtest (ACP channel: taiji acp run_backtest) ─────────────────────

/// `quant_backtest` — run a taiji backtest through the real ACP bridge.
pub struct QuantBacktestTool;

impl Default for QuantBacktestTool {
    fn default() -> Self {
        Self::new()
    }
}

impl QuantBacktestTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for QuantBacktestTool {
    fn name(&self) -> &str {
        "quant_backtest"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Run a taiji quantitative backtest through the real taiji-quant ACP bridge.

This tool spawns the external `taiji acp` process (standard Agent Client Protocol over stdio) and forwards a `run_backtest` method call to the taiji-lvpa quant engine. The result comes from the real engine, never a local simulation.

Arguments:
- "config": Required. BacktestConfig YAML (instruments / date_range / initial_capital / commission_per_lot / slippage_ticks / pipeline_template).
- "csv_data": Required. CSV data (OHLC rows) to backtest against.
- "pipeline": Optional. PipelineConfig YAML (name/version/bar_gen/data_source/nodes). Written to a temp file and referenced by pipeline_template, so the double-layer contract is exercised end to end.
- "workspace_path": Optional absolute workspace path; defaults to the current workspace.
- "timeout_seconds": Optional timeout for the external ACP turn."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Run a taiji quant backtest through the real taiji-quant ACP bridge.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "config": {
                    "type": "string",
                    "description": "BacktestConfig YAML (instruments/date_range/initial_capital/...)."
                },
                "csv_data": {
                    "type": "string",
                    "description": "CSV data (OHLC rows) to backtest against."
                },
                "pipeline": {
                    "type": "string",
                    "description": "Optional PipelineConfig YAML (name/version/bar_gen/data_source/nodes)."
                },
                "workspace_path": {
                    "type": "string",
                    "description": "Optional absolute workspace path; defaults to the current workspace."
                },
                "timeout_seconds": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Optional timeout for the external ACP turn."
                }
            },
            "required": ["config", "csv_data"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
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
        let parsed: QuantBacktestInput = match serde_json::from_value(input.clone()) {
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
        let result = !parsed.config.trim().is_empty() && !parsed.csv_data.trim().is_empty();
        ValidationResult {
            result,
            message: if result {
                None
            } else {
                Some("config and csv_data are required".to_string())
            },
            error_code: if result { None } else { Some(400) },
            meta: None,
        }
    }

    fn render_tool_use_message(&self, _input: &Value, _options: &ToolRenderOptions) -> String {
        "Run taiji quant backtest".to_string()
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: QuantBacktestInput = serde_json::from_value(input.clone())
            .map_err(|error| BitFunError::tool(format!("Invalid input: {}", error)))?;
        let workspace_path = workspace_or_context(params.workspace_path.as_deref(), context)?;

        // Double-layer contract (W1-P0-a, taiji-lvpa acp.rs handle_run_backtest):
        // - `config`   = BacktestConfig YAML (instruments/date_range/... +
        //   pipeline_template 指向 PipelineConfig 文件路径)
        // - `pipeline` = PipelineConfig YAML 内容字符串（可选）；lvpa 侧写临时
        //   文件并覆盖 config 内的 pipeline_template，runner.rs 再读文件穿透
        //   (read_to_string → PipelineConfig::from_yaml)。缺省时 lvpa 保持
        //   config 内 pipeline_template 路径（P0-c 回归兼容）。
        let mut message_body = json!({
            "config": params.config,
            "csv_data": params.csv_data,
        });
        if let Some(pipeline) = params.pipeline {
            message_body["pipeline"] = json!(pipeline);
        }

        // `run_backtest {"config": "<BacktestConfig YAML>", "csv_data": "...", "pipeline": "<PipelineConfig YAML>"}`
        // — the shape the taiji acp adapter layer parses (parse_custom_rpc).
        let message = format!("run_backtest {}", message_body);
        let response = acp_quant_prompt(
            &message,
            &workspace_path,
            params.timeout_seconds,
            "quant_backtest",
        )
        .await?;

        let parsed: Value = serde_json::from_str(&response)
            .unwrap_or_else(|_| json!({ "raw": response }));
        Ok(quant_tool_result(json!({
            "success": true,
            "method": "run_backtest",
            "channel": "acp",
            "response": parsed,
        })))
    }
}

// ── quote (9527 JSON-RPC: market.quote) ─────────────────────────────────────

/// `quote` — latest market snapshot from taiji-server JSON-RPC.
pub struct QuoteTool;

impl Default for QuoteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl QuoteTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for QuoteTool {
    fn name(&self) -> &str {
        "quote"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Fetch the latest market quote for one symbol from the taiji quant engine (taiji-server JSON-RPC `market.quote`).

Arguments:
- "symbol": Required. Instrument symbol, e.g. "a2609"."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Fetch the latest taiji market quote for one symbol.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Instrument symbol, e.g. a2609."
                }
            },
            "required": ["symbol"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: QuantQuoteInput = match serde_json::from_value(input.clone()) {
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
        let result = !parsed.symbol.trim().is_empty();
        ValidationResult {
            result,
            message: if result {
                None
            } else {
                Some("symbol is required".to_string())
            },
            error_code: if result { None } else { Some(400) },
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let symbol = input
            .get("symbol")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        format!("Fetch taiji market quote for '{}'", symbol)
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: QuantQuoteInput = serde_json::from_value(input.clone())
            .map_err(|error| BitFunError::tool(format!("Invalid input: {}", error)))?;
        let result = taiji_rpc_call("market.quote", json!({ "symbol": params.symbol })).await?;
        Ok(quant_tool_result(json!({
            "success": true,
            "method": "market.quote",
            "channel": "rpc",
            "response": result,
        })))
    }
}

// ── strategy (9527 JSON-RPC: strategy.backtest / strategy.report) ───────────

/// `strategy` — taiji strategy actions (backtest / report) via JSON-RPC.
pub struct StrategyTool;

impl Default for StrategyTool {
    fn default() -> Self {
        Self::new()
    }
}

impl StrategyTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for StrategyTool {
    fn name(&self) -> &str {
        "strategy"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Run taiji strategy actions through the taiji quant engine (taiji-server JSON-RPC).

Actions:
- "backtest": Run an offline backtest. Requires csv_path (absolute path to a CSV file). The engine builds an MA cross pipeline and returns a stored report id.
- "report": Fetch a stored backtest report. Requires backtest_id.

Arguments:
- "action": Required. One of "backtest", "report".
- "csv_path": Required for backtest.
- "backtest_id": Required for report."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Run taiji strategy backtest / report actions via JSON-RPC.".to_string()
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
                    "enum": ["backtest", "report"],
                    "description": "Strategy action to perform."
                },
                "csv_path": {
                    "type": "string",
                    "description": "Absolute CSV path; required for backtest."
                },
                "backtest_id": {
                    "type": "string",
                    "description": "Backtest report id; required for report."
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: QuantStrategyInput = match serde_json::from_value(input.clone()) {
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
        let result = match parsed.action.as_str() {
            "backtest" => parsed.csv_path.as_deref().map(str::trim).is_some_and(|v| !v.is_empty()),
            "report" => parsed.backtest_id.as_deref().map(str::trim).is_some_and(|v| !v.is_empty()),
            _ => false,
        };
        ValidationResult {
            result,
            message: if result {
                None
            } else {
                Some("action 'backtest' needs csv_path; action 'report' needs backtest_id".to_string())
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
        format!("Run taiji strategy '{}'", action)
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: QuantStrategyInput = serde_json::from_value(input.clone())
            .map_err(|error| BitFunError::tool(format!("Invalid input: {}", error)))?;
        let (method, call_params) = match params.action.as_str() {
            "backtest" => (
                "strategy.backtest",
                json!({ "csv_path": params.csv_path.unwrap_or_default() }),
            ),
            "report" => (
                "strategy.report",
                json!({ "backtest_id": params.backtest_id.unwrap_or_default() }),
            ),
            other => {
                return Err(BitFunError::tool(format!(
                    "unknown strategy action '{}'; expected backtest or report",
                    other
                )));
            }
        };
        let result = taiji_rpc_call(method, call_params).await?;
        Ok(quant_tool_result(json!({
            "success": true,
            "method": method,
            "channel": "rpc",
            "response": result,
        })))
    }
}

// ── order (9527 JSON-RPC: order.*) ──────────────────────────────────────────

/// `order` — taiji paper-trading order actions via JSON-RPC.
pub struct OrderTool;

impl Default for OrderTool {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for OrderTool {
    fn name(&self) -> &str {
        "order"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Manage taiji paper-trading orders through the taiji quant engine (taiji-server JSON-RPC).

Actions:
- "create": Place an order. Requires symbol, side ("buy"|"sell"), qty (integer lots). Optional: price (limit), order_type ("market"|"limit", default market), sl/tp.
- "list": List open orders (no extra arguments).
- "cancel": Cancel an order. Requires order_id.
- "modify": Modify an order. Requires order_id (plus the fields to change).
- "trade_history": Fetch executed trade history (no extra arguments).

Arguments:
- "action": Required.
- "symbol": Required for create.
- "side": Required for create.
- "qty": Required for create.
- "price": Optional for create (limit orders).
- "order_type": Optional for create.
- "order_id": Required for cancel/modify."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Manage taiji paper-trading orders via JSON-RPC.".to_string()
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
                    "enum": ["create", "list", "cancel", "modify", "trade_history"],
                    "description": "Order action to perform."
                },
                "symbol": {
                    "type": "string",
                    "description": "Instrument symbol; required for create."
                },
                "side": {
                    "type": "string",
                    "enum": ["buy", "sell"],
                    "description": "Order side; required for create."
                },
                "qty": {
                    "type": "number",
                    "description": "Quantity in integer lots; required for create."
                },
                "price": {
                    "type": "number",
                    "description": "Limit price; optional for create."
                },
                "order_type": {
                    "type": "string",
                    "enum": ["market", "limit"],
                    "description": "Order type; default market."
                },
                "order_id": {
                    "type": "string",
                    "description": "Order id; required for cancel/modify."
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
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
        let parsed: QuantOrderInput = match serde_json::from_value(input.clone()) {
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
        let result = match parsed.action.as_str() {
            "create" => {
                parsed.symbol.as_deref().map(str::trim).is_some_and(|v| !v.is_empty())
                    && matches!(parsed.side.as_deref(), Some("buy") | Some("sell"))
                    && parsed.qty.is_some_and(|qty| qty > 0.0)
            }
            "cancel" | "modify" => parsed
                .order_id
                .as_deref()
                .map(str::trim)
                .is_some_and(|v| !v.is_empty()),
            "list" | "trade_history" => true,
            _ => false,
        };
        ValidationResult {
            result,
            message: if result {
                None
            } else {
                Some("order action arguments are invalid".to_string())
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
        format!("Run taiji order '{}'", action)
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: QuantOrderInput = serde_json::from_value(input.clone())
            .map_err(|error| BitFunError::tool(format!("Invalid input: {}", error)))?;

        let (method, call_params) = match params.action.as_str() {
            "create" => {
                let mut map = serde_json::Map::new();
                map.insert("symbol".to_string(), json!(params.symbol.unwrap_or_default()));
                map.insert("side".to_string(), json!(params.side.unwrap_or_default()));
                map.insert("qty".to_string(), json!(params.qty.unwrap_or_default()));
                if let Some(price) = params.price {
                    map.insert("price".to_string(), json!(price));
                }
                if let Some(order_type) = params.order_type {
                    map.insert("order_type".to_string(), json!(order_type));
                }
                ("order.create", Value::Object(map))
            }
            "list" => ("order.list", json!({})),
            "cancel" => (
                "order.cancel",
                json!({ "order_id": params.order_id.unwrap_or_default() }),
            ),
            "modify" => {
                let mut map = serde_json::Map::new();
                map.insert(
                    "order_id".to_string(),
                    json!(params.order_id.unwrap_or_default()),
                );
                if let Some(price) = params.price {
                    map.insert("price".to_string(), json!(price));
                }
                if let Some(qty) = params.qty {
                    map.insert("qty".to_string(), json!(qty));
                }
                ("order.modify", Value::Object(map))
            }
            "trade_history" => ("order.trade_history", json!({})),
            other => {
                return Err(BitFunError::tool(format!(
                    "unknown order action '{}'",
                    other
                )));
            }
        };
        let result = taiji_rpc_call(method, call_params).await?;
        Ok(quant_tool_result(json!({
            "success": true,
            "method": method,
            "channel": "rpc",
            "response": result,
        })))
    }
}

// ── quant_pattern_chan / quant_pattern_dtw (9527 JSON-RPC: pattern.*) ────────
//
// W1-1（pattern 工具族）：复用 taiji_rpc_call（std TCP 直连 9527 /api/rpc），
// 零新依赖——与 quote/strategy/order 完全同通道、同超时、同错误映射。
// taiji-server 侧 pattern.chan（缠论 18 类买卖点）+ pattern.dtw（DTW 相似度）
// 由 taiji-pattern 能力 crate 提供（源区 taiji-lvpa 已注册）。

/// `quant_pattern_chan` — 缠论 18 类买卖点识别（taiji-server `pattern.chan`）。
pub struct QuantPatternChanTool;

impl Default for QuantPatternChanTool {
    fn default() -> Self {
        Self::new()
    }
}

impl QuantPatternChanTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for QuantPatternChanTool {
    fn name(&self) -> &str {
        "quant_pattern_chan"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Run Chan theory (缠论) buy/sell point recognition through the taiji quant engine (taiji-server JSON-RPC `pattern.chan`).

The engine detects fractals → bi strokes → hub (中枢) → segments → divergence → 18 buy/sell point types (一买/一卖/二买/二卖/三买/三卖 + T 系列).

Arguments:
- "bars": Optional. K-line array in ascending time order: [{"open":..,"high":..,"low":..,"close":..,"volume":..,"timestamp":..}, ...].
- "bis": Optional. Pre-computed bi strokes: [{"start_index":..,"end_index":..,"direction":"up"|"down","start_price":..,"end_price":..}, ...]. Provide either bars (full pipeline) or bis (hub→segment→divergence→BSP).
- "macd": Optional. MACD histogram values aligned with bars.
- "workspace_path": Optional absolute workspace path; defaults to the current workspace."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Run Chan theory buy/sell point recognition via taiji JSON-RPC.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bars": {
                    "type": "array",
                    "description": "K-line array: [{\"open\":..,\"high\":..,\"low\":..,\"close\":..,\"volume\":..,\"timestamp\":..}, ...]."
                },
                "bis": {
                    "type": "array",
                    "description": "Pre-computed bi strokes (skips the fractal/bi detection stage)."
                },
                "macd": {
                    "type": "array",
                    "description": "MACD histogram values aligned with bars (optional)."
                },
                "workspace_path": {
                    "type": "string",
                    "description": "Optional absolute workspace path."
                }
            },
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let has_bars = input
            .get("bars")
            .map(Value::is_array)
            .unwrap_or(false);
        let has_bis = input.get("bis").map(Value::is_array).unwrap_or(false);
        ValidationResult {
            result: has_bars || has_bis,
            message: if has_bars || has_bis {
                None
            } else {
                Some("bars or bis is required".to_string())
            },
            error_code: if has_bars || has_bis { None } else { Some(400) },
            meta: None,
        }
    }

    fn render_tool_use_message(&self, _input: &Value, _options: &ToolRenderOptions) -> String {
        "Run taiji Chan buy/sell point recognition".to_string()
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let mut params = match input.get("bars").map(Value::is_array).unwrap_or(false) {
            true => json!({ "bars": input.get("bars").unwrap() }),
            false => json!({ "bis": input.get("bis").unwrap_or(&Value::Null) }),
        };
        if let Some(macd) = input.get("macd").filter(|v| v.is_array()) {
            params["macd"] = macd.clone();
        }
        let result = taiji_rpc_call("pattern.chan", params).await?;
        Ok(quant_tool_result(json!({
            "success": true,
            "method": "pattern.chan",
            "channel": "rpc",
            "response": result,
        })))
    }
}

/// `quant_pattern_dtw` — 多维 DTW 相似度（taiji-server `pattern.dtw`）。
pub struct QuantPatternDtwTool;

impl Default for QuantPatternDtwTool {
    fn default() -> Self {
        Self::new()
    }
}

impl QuantPatternDtwTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for QuantPatternDtwTool {
    fn name(&self) -> &str {
        "quant_pattern_dtw"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Compute DTW (Dynamic Time Warping) similarity through the taiji quant engine (taiji-server JSON-RPC `pattern.dtw`).

Returns distance + LB_Keogh lower bound using the weighted-Euclidean DtwEngine with Sakoe-Chiba band.

Arguments:
- "query": Required. Query series as 2D array: [[f,..], ..] (each row = one time step, each column = one feature).
- "template": Required. Template series, same feature dimension as query.
- "window": Optional. Sakoe-Chiba window width in time steps (0/omitted = no band).
- "feature_weights": Optional. Per-feature weights (length must equal feature count; default all 1.0).
- "workspace_path": Optional absolute workspace path; defaults to the current workspace."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Compute DTW similarity via taiji JSON-RPC.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "array",
                    "description": "Query series 2D array: [[f,..], ..]."
                },
                "template": {
                    "type": "array",
                    "description": "Template series 2D array, same feature dimension."
                },
                "window": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Sakoe-Chiba window width (0 = no band)."
                },
                "feature_weights": {
                    "type": "array",
                    "description": "Per-feature weights (default all 1.0)."
                },
                "workspace_path": {
                    "type": "string",
                    "description": "Optional absolute workspace path."
                }
            },
            "required": ["query", "template"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let ok = input.get("query").map(Value::is_array).unwrap_or(false)
            && input.get("template").map(Value::is_array).unwrap_or(false);
        ValidationResult {
            result: ok,
            message: if ok {
                None
            } else {
                Some("query and template are required".to_string())
            },
            error_code: if ok { None } else { Some(400) },
            meta: None,
        }
    }

    fn render_tool_use_message(&self, _input: &Value, _options: &ToolRenderOptions) -> String {
        "Compute taiji DTW similarity".to_string()
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let query = input
            .get("query")
            .ok_or_else(|| BitFunError::tool("query is required".to_string()))?;
        let template = input
            .get("template")
            .ok_or_else(|| BitFunError::tool("template is required".to_string()))?;
        let mut params = json!({ "query": query, "template": template });
        if let Some(window) = input.get("window").and_then(Value::as_u64) {
            params["window"] = json!(window);
        }
        if let Some(weights) = input.get("feature_weights").filter(|v| v.is_array()) {
            params["feature_weights"] = weights.clone();
        }
        let result = taiji_rpc_call("pattern.dtw", params).await?;
        Ok(quant_tool_result(json!({
            "success": true,
            "method": "pattern.dtw",
            "channel": "rpc",
            "response": result,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_names_match_registered_contract() {
        assert_eq!(QuantBacktestTool::new().name(), "quant_backtest");
        assert_eq!(QuoteTool::new().name(), "quote");
        assert_eq!(StrategyTool::new().name(), "strategy");
        assert_eq!(OrderTool::new().name(), "order");
        assert_eq!(QuantPatternChanTool::new().name(), "quant_pattern_chan");
        assert_eq!(QuantPatternDtwTool::new().name(), "quant_pattern_dtw");
    }

    #[tokio::test]
    async fn quote_validation_requires_symbol() {
        let tool = QuoteTool::new();
        let bad = tool
            .validate_input(&json!({ "symbol": "" }), None)
            .await;
        assert!(!bad.result);

        let ok = tool
            .validate_input(&json!({ "symbol": "a2609" }), None)
            .await;
        assert!(ok.result);
    }

    #[tokio::test]
    async fn strategy_validation_matches_action_arguments() {
        let tool = StrategyTool::new();
        let backtest_ok = tool
            .validate_input(&json!({ "action": "backtest", "csv_path": "C:/data.csv" }), None)
            .await;
        assert!(backtest_ok.result);

        let report_ok = tool
            .validate_input(&json!({ "action": "report", "backtest_id": "1" }), None)
            .await;
        assert!(report_ok.result);

        let missing = tool
            .validate_input(&json!({ "action": "backtest" }), None)
            .await;
        assert!(!missing.result);
    }

    #[tokio::test]
    async fn order_validation_requires_create_fields() {
        let tool = OrderTool::new();
        let ok = tool
            .validate_input(
                &json!({ "action": "create", "symbol": "a2609", "side": "buy", "qty": 2 }),
                None,
            )
            .await;
        assert!(ok.result);

        let bad = tool
            .validate_input(&json!({ "action": "create", "symbol": "a2609" }), None)
            .await;
        assert!(!bad.result);

        let cancel_ok = tool
            .validate_input(&json!({ "action": "cancel", "order_id": "o1" }), None)
            .await;
        assert!(cancel_ok.result);
    }

    #[tokio::test]
    async fn quant_backtest_validation_requires_config_and_csv() {
        let tool = QuantBacktestTool::new();
        let bad = tool.validate_input(&json!({ "config": "" }), None).await;
        assert!(!bad.result);

        let ok = tool
            .validate_input(&json!({ "config": "yaml", "csv_data": "date,open\n" }), None)
            .await;
        assert!(ok.result);
    }

    #[tokio::test]
    async fn pattern_chan_validation_requires_bars_or_bis() {
        let tool = QuantPatternChanTool::new();
        let bad = tool.validate_input(&json!({}), None).await;
        assert!(!bad.result);

        let bars_ok = tool
            .validate_input(
                &json!({ "bars": [{"open":1.0,"high":2.0,"low":0.5,"close":1.5}] }),
                None,
            )
            .await;
        assert!(bars_ok.result);

        let bis_ok = tool
            .validate_input(
                &json!({ "bis": [{"start_index":0,"end_index":1,"direction":"up","start_price":1.0,"end_price":2.0}] }),
                None,
            )
            .await;
        assert!(bis_ok.result);
    }

    #[tokio::test]
    async fn pattern_dtw_validation_requires_query_and_template() {
        let tool = QuantPatternDtwTool::new();
        let bad = tool.validate_input(&json!({ "query": [[1.0]] }), None).await;
        assert!(!bad.result);

        let ok = tool
            .validate_input(&json!({ "query": [[1.0]], "template": [[1.0]] }), None)
            .await;
        assert!(ok.result);
    }

    #[test]
    fn quant_backtest_input_accepts_optional_pipeline() {
        let input: QuantBacktestInput = serde_json::from_value(json!({
            "config": "instruments:\n  - rb9999\n",
            "csv_data": "instrument,price\n",
            "pipeline": "name: x\nversion: \"1.0\"\n"
        }))
        .expect("pipeline field should deserialize");
        assert!(input.pipeline.is_some());
        assert_eq!(input.config.contains("rb9999"), true);

        let legacy: QuantBacktestInput = serde_json::from_value(json!({
            "config": "yaml",
            "csv_data": "csv"
        }))
        .expect("legacy shape without pipeline should deserialize");
        assert!(legacy.pipeline.is_none());
    }
}
