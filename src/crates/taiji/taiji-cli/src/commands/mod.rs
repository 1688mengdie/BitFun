//! # 命令模块 — 天机令正文
//!
//! 所有子命令的实现都在此模块中。
//! 每个文件对应一个子命令，统一通过 `run(ctx, ...)` 入口调用。

pub(crate) mod account;
pub(crate) mod alert;
pub(crate) mod backtest;
pub(crate) mod completion;
pub(crate) mod config_cmd;
pub(crate) mod depth;
pub(crate) mod doctor;
pub(crate) mod kline;
pub(crate) mod limit;
pub(crate) mod list_symbols;
pub(crate) mod log;
pub(crate) mod monitor;
pub(crate) mod optimize;
pub(crate) mod order;
pub(crate) mod position;
pub(crate) mod quote;
pub(crate) mod report;
pub(crate) mod risk;
pub(crate) mod session;
pub(crate) mod strategy;
pub(crate) mod trade_history;
pub(crate) mod version;
pub(crate) mod watch;

use crate::output::{self, OutputFormat};
use crate::transport;
use crate::transport::TransportClient;
use crate::AppContext;
use anyhow::Result;

/// 连接到后端并执行 JSON-RPC 请求的辅助函数
pub(crate) async fn rpc_call(
    ctx: &AppContext,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let client = transport::create_client(&ctx.server_url);
    client
        .request(method, params)
        .await
        .map_err(|e| anyhow::anyhow!("RPC 调用失败 [{}]: {}", method, e))
}

/// 输出数据的统一入口
pub(crate) fn print_output(
    data: &serde_json::Value,
    format: OutputFormat,
) -> Result<()> {
    output::write_output(data, format, &mut std::io::stdout())?;
    Ok(())
}


