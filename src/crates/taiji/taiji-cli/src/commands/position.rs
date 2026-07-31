//! `taiji position` — 持仓查询

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(ctx: AppContext, symbol: Option<String>) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let data = super::rpc_call(
            &ctx,
            "position.list",
            serde_json::json!({ "symbol": symbol }),
        )
        .await?;
        super::print_output(&data, ctx.output_format)
    })
}
