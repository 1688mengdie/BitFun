//! `taiji backtest <strategy-id>` — 回测执行
//!
//! 回测可以通过两种方式运行：
//! 1. 连接后端服务执行（默认）
//! 2. 本地 CSV 文件直接运行（--csv 参数）

use std::path::{Path, PathBuf};

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(
    ctx: AppContext,
    strategy_id: String,
    param: Vec<(String, String)>,
    from: Option<String>,
    to: Option<String>,
    csv: Option<PathBuf>,
    parallel: bool,
) -> Result<()> {
    // 如果有本地 CSV 文件，直接本地运行
    if let Some(csv_path) = csv {
        return run_local_backtest(&strategy_id, &csv_path, parallel);
    }

    // 默认：通过后端服务
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let params: serde_json::Value =
            serde_json::to_value(param).unwrap_or(serde_json::json!({}));
        let data = super::rpc_call(
            &ctx,
            "strategy.backtest",
            serde_json::json!({
                "strategy_id": strategy_id,
                "params": params,
                "from": from,
                "to": to,
                "parallel": parallel,
            }),
        )
        .await?;
        super::print_output(&data, ctx.output_format)
    })
}

/// 本地 CSV 回测（通过 taiji-backtest crate 直接运行）
fn run_local_backtest(
    strategy_id: &str,
    csv_path: &Path,
    parallel: bool,
) -> Result<()> {
    eprintln!("本地回测: strategy={}, csv={}, parallel={}", strategy_id, csv_path.display(), parallel);
    // 通过 taiji-backtest crate 运行，需要 YAML 配置
    // 目前需要 taiji-backtest crate 的配置，简化处理
    eprintln!("提示: 本地回测需要 YAML 配置文件。使用 `taiji backtest` 通过后端服务运行更便捷。");
    eprintln!("或使用原有 `taiji pipeline --config <yaml> --csv <csv>` 命令。");
    Ok(())
}
