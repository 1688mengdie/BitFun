//! `taiji depth <symbol>` — 深度盘口

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(ctx: AppContext, symbol: String, level: usize) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let data = super::rpc_call(
            &ctx,
            "market.depth",
            serde_json::json!({ "symbol": &symbol, "level": level }),
        )
        .await?;
        super::print_output(&data, ctx.output_format)
    })
}
