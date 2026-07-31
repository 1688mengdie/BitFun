//! `taiji trade-history` — 成交历史

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(
    ctx: AppContext,
    from: Option<String>,
    to: Option<String>,
    symbol: Option<String>,
) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let data = super::rpc_call(
            &ctx,
            "order.trade_history",
            serde_json::json!({ "from": from, "to": to, "symbol": symbol }),
        )
        .await?;
        super::print_output(&data, ctx.output_format)
    })
}
