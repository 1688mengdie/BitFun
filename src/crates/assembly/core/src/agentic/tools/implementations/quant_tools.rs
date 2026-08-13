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

// ── quant_sentiment (9527 JSON-RPC: sentiment.*) ───────────────────────────
//
// W1-3（sentiment 工具族）：复用 taiji_rpc_call（std TCP 直连 9527 /api/rpc），
// 零新依赖——与 quote/strategy/order 完全同通道、同超时、同错误映射。
// taiji-server 侧 sentiment.analyze（jieba + 内置词典文本情绪）+ sentiment.fgi
// （FearGreedIndex 五因子 0-100 温度计）由 taiji-sentiment 能力 crate 提供
// （源区 taiji-lvpa 已注册）。

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct QuantSentimentInput {
    /// One of "analyze" | "fgi".
    pub action: String,
    /// Text to analyze (analyze).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    // ── fgi 五因子（可选，缺省 50.0 中性）──
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hv20: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commodity_momentum: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oi_change_rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basis_slope: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nlp_sentiment: Option<f64>,
}

/// `quant_sentiment` — 市场情绪分析（taiji-server `sentiment.analyze` /
/// `sentiment.fgi`）。
pub struct QuantSentimentTool;

impl Default for QuantSentimentTool {
    fn default() -> Self {
        Self::new()
    }
}

impl QuantSentimentTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for QuantSentimentTool {
    fn name(&self) -> &str {
        "quant_sentiment"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Run market sentiment analysis through the taiji quant engine (taiji-server JSON-RPC `sentiment.analyze` / `sentiment.fgi`).

Actions:
- "analyze": Analyze the sentiment of a Chinese text (news/research/commentary). jieba tokenization + built-in financial sentiment dictionary + degree modifiers + negation flip. Returns score in [-1,1] (>0 bullish, <0 bearish), confidence, positive/negative words, and policy keywords.
- "fgi": Compute the Fear & Greed Index (0-100) from five factors: hv20 (25%), commodity_momentum (25%), oi_change_rate (20%), basis_slope (15%), nlp_sentiment (15%). Each factor is a pre-normalized 0-100 contribution; missing factors default to neutral 50.0.

Arguments:
- "action": Required. One of "analyze", "fgi".
- "text": Required for analyze.
- "hv20" / "commodity_momentum" / "oi_change_rate" / "basis_slope" / "nlp_sentiment": Optional 0-100 factor contributions for fgi."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Run taiji market sentiment analysis (analyze / fgi) via JSON-RPC.".to_string()
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
                    "enum": ["analyze", "fgi"],
                    "description": "Sentiment action to perform."
                },
                "text": {
                    "type": "string",
                    "description": "Text to analyze; required for analyze."
                },
                "hv20": {
                    "type": "number",
                    "description": "HV20 factor contribution (0-100); optional for fgi."
                },
                "commodity_momentum": {
                    "type": "number",
                    "description": "Commodity momentum factor contribution (0-100); optional for fgi."
                },
                "oi_change_rate": {
                    "type": "number",
                    "description": "OI change rate factor contribution (0-100); optional for fgi."
                },
                "basis_slope": {
                    "type": "number",
                    "description": "Basis slope factor contribution (0-100); optional for fgi."
                },
                "nlp_sentiment": {
                    "type": "number",
                    "description": "NLP sentiment factor contribution (0-100); optional for fgi."
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
        let parsed: QuantSentimentInput = match serde_json::from_value(input.clone()) {
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
            "analyze" => parsed.text.as_deref().map(str::trim).is_some_and(|v| !v.is_empty()),
            "fgi" => true,
            _ => false,
        };
        ValidationResult {
            result,
            message: if result {
                None
            } else {
                Some("action 'analyze' needs text; action 'fgi' takes optional factors".to_string())
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
        format!("Run taiji sentiment '{}'", action)
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: QuantSentimentInput = serde_json::from_value(input.clone())
            .map_err(|error| BitFunError::tool(format!("Invalid input: {}", error)))?;
        let (method, call_params) = match params.action.as_str() {
            "analyze" => (
                "sentiment.analyze",
                json!({ "text": params.text.unwrap_or_default() }),
            ),
            "fgi" => {
                let mut map = serde_json::Map::new();
                if let Some(v) = params.hv20 {
                    map.insert("hv20".to_string(), json!(v));
                }
                if let Some(v) = params.commodity_momentum {
                    map.insert("commodity_momentum".to_string(), json!(v));
                }
                if let Some(v) = params.oi_change_rate {
                    map.insert("oi_change_rate".to_string(), json!(v));
                }
                if let Some(v) = params.basis_slope {
                    map.insert("basis_slope".to_string(), json!(v));
                }
                if let Some(v) = params.nlp_sentiment {
                    map.insert("nlp_sentiment".to_string(), json!(v));
                }
                ("sentiment.fgi", Value::Object(map))
            }
            other => {
                return Err(BitFunError::tool(format!(
                    "unknown sentiment action '{}'; expected analyze or fgi",
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

// ── quant_alert (9527 JSON-RPC: alert.*) ────────────────────────────────────
//
// W1-5（alert 工具族）：复用 taiji_rpc_call（std TCP 直连 9527 /api/rpc），
// 零新依赖——与 quote/strategy/order 完全同通道、同超时、同错误映射。
// taiji-server 侧 alert.send（taiji-alert AlertManager 三通道路由 +
// AlertStore 登记衔接 risk.alerts/risk.alert.ack）+ alert.heartbeat.record
// （HeartbeatMonitor 活动登记）由 taiji-alert 能力 crate 提供（源区
// taiji-lvpa 已注册）。
//
// 副作用红线（W1-5）：alert.send 涉及真实发送通道（Feishu webhook / SMTP /
// 桌面通知）。本工具默认 dry_run=true（仅登记 AlertStore 模拟，零真实投递）；
// 显式 dry_run=false 才放行真实投递——真实发送需用户确认（干跑优先）。

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct QuantAlertInput {
    /// One of "send" | "heartbeat".
    pub action: String,
    /// Alert level (send): heartbeat | warn | error | critical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// Short title (send).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Detailed body (send).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// Source component (send; optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// dry-run (send; default true): true = only register to AlertStore, zero
    /// real delivery. Real delivery requires explicit dry_run=false.
    #[serde(default = "default_dry_run_true")]
    pub dry_run: bool,
}

/// serde 缺省：dry_run 默认 true（干跑优先，副作用红线——真实发送通道需
/// 显式 dry_run=false + 用户确认）。
fn default_dry_run_true() -> bool {
    true
}

/// `quant_alert` — taiji alert actions (send / heartbeat) via JSON-RPC.
pub struct QuantAlertTool;

impl Default for QuantAlertTool {
    fn default() -> Self {
        Self::new()
    }
}

impl QuantAlertTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for QuantAlertTool {
    fn name(&self) -> &str {
        "quant_alert"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Run taiji alert actions through the taiji quant engine (taiji-server JSON-RPC).

Actions:
- "send": Submit an alert. Requires level (heartbeat|warn|error|critical), title, body. Optional source, dry_run (default true). dry_run=true only registers the alert to the server AlertStore (visible via risk.alerts / risk.alert.ack) with zero real delivery; real delivery (desktop/Feishu/email) requires explicit dry_run=false and user confirmation.
- "heartbeat": Record system activity, resetting the heartbeat timeout timer (no arguments).

Arguments:
- "action": Required. One of "send", "heartbeat".
- "level": Required for send.
- "title": Required for send.
- "body": Required for send.
- "source": Optional for send.
- "dry_run": Optional for send (default true)."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Run taiji alert send / heartbeat actions via JSON-RPC.".to_string()
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
                    "enum": ["send", "heartbeat"],
                    "description": "Alert action to perform."
                },
                "level": {
                    "type": "string",
                    "enum": ["heartbeat", "warn", "error", "critical"],
                    "description": "Alert level; required for send."
                },
                "title": {
                    "type": "string",
                    "description": "Short alert title; required for send."
                },
                "body": {
                    "type": "string",
                    "description": "Detailed alert body (Markdown); required for send."
                },
                "source": {
                    "type": "string",
                    "description": "Source component (e.g. cron:job_id); optional for send."
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "Default true: only register to AlertStore, zero real delivery."
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
        let parsed: QuantAlertInput = match serde_json::from_value(input.clone()) {
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
            "send" => {
                parsed.level.as_deref().map(str::trim).is_some_and(|v| !v.is_empty())
                    && parsed.title.as_deref().map(str::trim).is_some_and(|v| !v.is_empty())
                    && parsed.body.as_deref().map(str::trim).is_some_and(|v| !v.is_empty())
            }
            "heartbeat" => true,
            _ => false,
        };
        ValidationResult {
            result,
            message: if result {
                None
            } else {
                Some("action 'send' needs level/title/body; action 'heartbeat' takes no arguments".to_string())
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
        format!("Run taiji alert '{}'", action)
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: QuantAlertInput = serde_json::from_value(input.clone())
            .map_err(|error| BitFunError::tool(format!("Invalid input: {}", error)))?;

        let (method, call_params) = match params.action.as_str() {
            "send" => {
                let mut map = serde_json::Map::new();
                map.insert("level".to_string(), json!(params.level.unwrap_or_default()));
                map.insert("title".to_string(), json!(params.title.unwrap_or_default()));
                map.insert("body".to_string(), json!(params.body.unwrap_or_default()));
                if let Some(source) = params.source {
                    map.insert("source".to_string(), json!(source));
                }
                // 副作用红线：默认 dry_run=true（干跑优先，零真实投递）；
                // 显式 dry_run=false 才放行真实投递（需用户确认）。
                map.insert("dry_run".to_string(), json!(params.dry_run));
                ("alert.send", Value::Object(map))
            }
            "heartbeat" => ("alert.heartbeat.record", json!({})),
            other => {
                return Err(BitFunError::tool(format!(
                    "unknown alert action '{}'; expected send or heartbeat",
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

// ── quant_strategen (9527 JSON-RPC: strategen.generate / strategen.validate) ─
//
// W1-4（strategen 工具族）：复用 taiji_rpc_call（std TCP 直连 9527 /api/rpc），
// 零新依赖——与 quote/strategy/order/quant_pattern_*/quant_sentiment 完全同通道、
// 同超时、同错误映射。
// taiji-server 侧 strategen.generate（StrategyGenPipeline 五阶段策略生成，默认
// mock refiner 无 LLM key 可跑，产出 yaml 含 version:"1.0" 与 P0 呼应）+
// strategen.validate（假设校验）由 taiji-strategen 能力 crate 提供（源区
// taiji-lvpa 已注册）。

/// `quant_strategen` — taiji 策略生成/校验（taiji-server `strategen.*`）。
pub struct QuantStrategenTool;

impl Default for QuantStrategenTool {
    fn default() -> Self {
        Self::new()
    }
}

impl QuantStrategenTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for QuantStrategenTool {
    fn name(&self) -> &str {
        "quant_strategen"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Run taiji strategy generation / validation through the taiji quant engine (taiji-server JSON-RPC `strategen.generate` / `strategen.validate`).

Actions:
- "generate": Run the 5-stage StrategyGenPipeline (hypothesis → validate → compile → backtest → analyze/refine). Defaults to the mock refiner so no LLM API key is required. Produces PipelineConfig YAML containing version "1.0" (P0 contract). Requires "hypothesis".
- "validate": Validate a strategy hypothesis (type safety, reasonability, look-ahead bias). Returns is_valid / lookahead_free / errors / warnings. Requires "hypothesis".

Hypothesis shape:
{"name":.., "description":.., "entry_conditions":[{"indicator":"MA","params":{"period":5},"operator":"cross_above","value":0.0}], "exit_conditions":[...], "position_sizing":{"method":"fixed","value":1.0}, "risk_params":{"stop_loss":50.0,"take_profit":100.0,"max_holding_bars":20}, "instruments":["rb9999"]}

Arguments:
- "action": Required. One of "generate", "validate".
- "hypothesis": Required. Hypothesis object (see shape above).
- "csv_path": Optional for generate. Absolute CSV path to backtest against (omitted → pipeline runs without backtest).
- "use_mock_refiner": Optional for generate, default true.
- "registered_indicators": Optional. Indicator name list for the validator.
- "workspace_path": Optional absolute workspace path."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Run taiji strategy generation / validation via JSON-RPC.".to_string()
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
                    "enum": ["generate", "validate"],
                    "description": "Strategen action to perform."
                },
                "hypothesis": {
                    "type": "object",
                    "description": "Strategy hypothesis (name/description/entry_conditions/exit_conditions/position_sizing/risk_params/instruments)."
                },
                "csv_path": {
                    "type": "string",
                    "description": "Absolute CSV path; optional for generate."
                },
                "use_mock_refiner": {
                    "type": "boolean",
                    "description": "Use heuristic refinement without LLM key (default true)."
                },
                "registered_indicators": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Registered indicator names for validation warnings."
                },
                "workspace_path": {
                    "type": "string",
                    "description": "Optional absolute workspace path."
                }
            },
            "required": ["action", "hypothesis"],
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
        let action_ok = match input.get("action").and_then(Value::as_str) {
            Some("generate") | Some("validate") => true,
            _ => false,
        };
        let hypothesis_ok = input.get("hypothesis").map(Value::is_object).unwrap_or(false);
        let result = action_ok && hypothesis_ok;
        ValidationResult {
            result,
            message: if result {
                None
            } else {
                Some("action ('generate'|'validate') and hypothesis object are required".to_string())
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
        format!("Run taiji strategen '{}'", action)
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let action = input
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| BitFunError::tool("action is required".to_string()))?;
        let hypothesis = input
            .get("hypothesis")
            .cloned()
            .ok_or_else(|| BitFunError::tool("hypothesis is required".to_string()))?;

        let (method, mut params) = match action {
            "generate" => ("strategen.generate", json!({ "hypothesis": hypothesis })),
            "validate" => ("strategen.validate", json!({ "hypothesis": hypothesis })),
            other => {
                return Err(BitFunError::tool(format!(
                    "unknown strategen action '{}'; expected generate or validate",
                    other
                )));
            }
        };
        if let Some(csv_path) = input.get("csv_path").and_then(Value::as_str) {
            params["csv_path"] = json!(csv_path);
        }
        if let Some(use_mock) = input.get("use_mock_refiner").and_then(Value::as_bool) {
            params["use_mock_refiner"] = json!(use_mock);
        }
        if let Some(indicators) = input
            .get("registered_indicators")
            .filter(|v| v.is_array())
        {
            params["registered_indicators"] = indicators.clone();
        }
        let result = taiji_rpc_call(method, params).await?;
        Ok(quant_tool_result(json!({
            "success": true,
            "method": method,
            "channel": "rpc",
            "response": result,
        })))
    }
}

// ── quant_anomaly (9527 JSON-RPC: abnormal.score) ──────────────────────────
//
// W1-2（anomaly 工具族）：复用 taiji_rpc_call（std TCP 直连 9527 /api/rpc），
// 零新依赖——与 quote/strategy/order 完全同通道、同超时、同错误映射。
// taiji-server 侧 abnormal.score（5 节点 compute_score 纯函数 +
// ScorecardFusionNode 加权融合，阈值 warn70/reduce85/emergency95）
// 由 taiji-abnormal 能力 crate 提供（源区 taiji-lvpa 已注册）。

/// `quant_anomaly` — taiji abnormal scoring card via JSON-RPC
///（abnormal.score：5 指标加权融合异常评分）。
pub struct QuantAnomalyTool;

impl Default for QuantAnomalyTool {
    fn default() -> Self {
        Self::new()
    }
}

impl QuantAnomalyTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for QuantAnomalyTool {
    fn name(&self) -> &str {
        "quant_anomaly"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Compute the taiji abnormal scoring card through the taiji quant engine (taiji-server JSON-RPC `abnormal.score`).

The engine fuses 5 anomaly indicators (vol_regime / vol_anomaly / corr_fracture / gap_alert / trend_accel) with default weights 0.25/0.20/0.15/0.25/0.15 and returns a 0-100 score plus alert level (normal/warn/reduce/emergency) at thresholds warn=70 / reduce=85 / emergency=95.

Arguments:
- "bars": Required. K-line array in ascending time order: [{"open":..,"high":..,"low":..,"close":..,"volume":..,"timestamp":..}, ...].
- "lookback": Optional. Lookback window in bars (default 30; each indicator clamps to its own domain window).
- "weights": Optional. Per-indicator weight overrides ({vol_regime,vol_anomaly,corr_fracture,gap_alert,trend_accel}; must sum to 1).
- "thresholds": Optional. Threshold overrides ({warn,reduce,emergency}; must satisfy warn < reduce < emergency).
- "workspace_path": Optional absolute workspace path; defaults to the current workspace."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Compute taiji abnormal scoring card via JSON-RPC.".to_string()
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
                "lookback": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Lookback window in bars (default 30)."
                },
                "weights": {
                    "type": "object",
                    "description": "Per-indicator weights: {vol_regime,vol_anomaly,corr_fracture,gap_alert,trend_accel} (sum to 1)."
                },
                "thresholds": {
                    "type": "object",
                    "description": "Threshold overrides: {warn,reduce,emergency} (warn < reduce < emergency)."
                },
                "workspace_path": {
                    "type": "string",
                    "description": "Optional absolute workspace path."
                }
            },
            "required": ["bars"],
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
        ValidationResult {
            result: has_bars,
            message: if has_bars {
                None
            } else {
                Some("bars is required".to_string())
            },
            error_code: if has_bars { None } else { Some(400) },
            meta: None,
        }
    }

    fn render_tool_use_message(&self, _input: &Value, _options: &ToolRenderOptions) -> String {
        "Compute taiji abnormal scoring card".to_string()
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let bars = input
            .get("bars")
            .ok_or_else(|| BitFunError::tool("bars is required".to_string()))?;
        let mut params = json!({ "bars": bars });
        if let Some(lookback) = input.get("lookback").and_then(Value::as_u64) {
            params["lookback"] = json!(lookback);
        }
        if let Some(weights) = input.get("weights").filter(|v| v.is_object()) {
            params["weights"] = weights.clone();
        }
        if let Some(thresholds) = input.get("thresholds").filter(|v| v.is_object()) {
            params["thresholds"] = thresholds.clone();
        }
        let result = taiji_rpc_call("abnormal.score", params).await?;
        Ok(quant_tool_result(json!({
            "success": true,
            "method": "abnormal.score",
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
        assert_eq!(QuantSentimentTool::new().name(), "quant_sentiment");
        assert_eq!(QuantAlertTool::new().name(), "quant_alert");
        assert_eq!(QuantStrategenTool::new().name(), "quant_strategen");
        assert_eq!(QuantAnomalyTool::new().name(), "quant_anomaly");
    }

    #[tokio::test]
    async fn anomaly_validation_requires_bars() {
        let tool = QuantAnomalyTool::new();
        let bad = tool.validate_input(&json!({}), None).await;
        assert!(!bad.result);

        let bars_ok = tool
            .validate_input(
                &json!({ "bars": [{"open":1.0,"high":2.0,"low":0.5,"close":1.5}] }),
                None,
            )
            .await;
        assert!(bars_ok.result);
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

    #[tokio::test]
    async fn sentiment_validation_matches_action_arguments() {
        let tool = QuantSentimentTool::new();
        let analyze_ok = tool
            .validate_input(&json!({ "action": "analyze", "text": "央行降息" }), None)
            .await;
        assert!(analyze_ok.result);

        let fgi_ok = tool
            .validate_input(
                &json!({ "action": "fgi", "hv20": 40.0, "commodity_momentum": 70.0 }),
                None,
            )
            .await;
        assert!(fgi_ok.result);

        let fgi_default = tool.validate_input(&json!({ "action": "fgi" }), None).await;
        assert!(fgi_default.result, "fgi 全部因子可选，缺省 50 中性");

        let missing_text = tool
            .validate_input(&json!({ "action": "analyze" }), None)
            .await;
        assert!(!missing_text.result);

        let unknown = tool
            .validate_input(&json!({ "action": "bogus" }), None)
            .await;
        assert!(!unknown.result);
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

    #[tokio::test]
    async fn alert_validation_matches_action_arguments() {
        let tool = QuantAlertTool::new();
        let send_ok = tool
            .validate_input(
                &json!({ "action": "send", "level": "warn", "title": "限额告警", "body": "80%" }),
                None,
            )
            .await;
        assert!(send_ok.result);

        let send_dry_run_default = tool
            .validate_input(
                &json!({ "action": "send", "level": "error", "title": "t", "body": "b" }),
                None,
            )
            .await;
        assert!(send_dry_run_default.result);

        let heartbeat_ok = tool
            .validate_input(&json!({ "action": "heartbeat" }), None)
            .await;
        assert!(heartbeat_ok.result);

        let missing_body = tool
            .validate_input(&json!({ "action": "send", "level": "warn", "title": "t" }), None)
            .await;
        assert!(!missing_body.result);

        let missing_level = tool
            .validate_input(&json!({ "action": "send", "title": "t", "body": "b" }), None)
            .await;
        assert!(!missing_level.result);

        let unknown = tool
            .validate_input(&json!({ "action": "bogus" }), None)
            .await;
        assert!(!unknown.result);
    }

    #[test]
    fn alert_input_defaults_dry_run_true() {
        let input: QuantAlertInput = serde_json::from_value(json!({
            "action": "send",
            "level": "critical",
            "title": "t",
            "body": "b"
        }))
        .expect("send shape should deserialize");
        assert!(input.dry_run, "dry_run 缺省应为 true（干跑优先，副作用红线）");
    }

    #[tokio::test]
    async fn strategen_validation_requires_action_and_hypothesis() {
        let tool = QuantStrategenTool::new();
        let hypothesis = json!({
            "name": "t",
            "entry_conditions": [{"indicator": "MA", "params": {"period": 5}, "operator": "cross_above", "value": 0.0}],
            "exit_conditions": [],
            "position_sizing": {"method": "fixed", "value": 1.0},
            "risk_params": {"max_holding_bars": 20},
            "instruments": ["rb9999"]
        });

        let ok = tool
            .validate_input(&json!({ "action": "generate", "hypothesis": hypothesis }), None)
            .await;
        assert!(ok.result, "generate + hypothesis 应通过");

        let ok_validate = tool
            .validate_input(&json!({ "action": "validate", "hypothesis": hypothesis }), None)
            .await;
        assert!(ok_validate.result, "validate + hypothesis 应通过");

        let bad = tool
            .validate_input(&json!({ "action": "generate" }), None)
            .await;
        assert!(!bad.result, "缺 hypothesis 应拒绝");

        let bad_action = tool
            .validate_input(&json!({ "action": "bogus", "hypothesis": hypothesis }), None)
            .await;
        assert!(!bad_action.result, "未知 action 应拒绝");
    }
}
