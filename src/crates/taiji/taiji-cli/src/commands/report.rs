//! `taiji report <backtest-id>` — 回测报告查看

use crate::output::OutputFormat;
use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(
    ctx: AppContext,
    backtest_id: String,
    output_format: Option<OutputFormat>,
) -> Result<()> {
    let fmt = output_format.unwrap_or(ctx.output_format);
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let data = super::rpc_call(
            &ctx,
            "strategy.report",
            serde_json::json!({ "backtest_id": backtest_id }),
        )
        .await?;
        super::print_output(&data, fmt)
    })
}
