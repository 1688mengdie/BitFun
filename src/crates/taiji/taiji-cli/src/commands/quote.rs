//! `taiji quote <symbol>` — 实时行情快照

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(ctx: AppContext, symbol: String) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let data = super::rpc_call(&ctx, "market.quote", serde_json::json!({ "symbol": &symbol })).await?;
        super::print_output(&data, ctx.output_format)
    })
}
