//! `taiji list-symbols` — 可交易品种列表

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(
    ctx: AppContext,
    _exchange: Option<String>,
    _asset_type: Option<String>,
) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let data = super::rpc_call(&ctx, "market.symbols", serde_json::json!({})).await?;
        super::print_output(&data, ctx.output_format)
    })
}
