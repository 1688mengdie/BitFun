//! `taiji risk` — 风控状态总览

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(ctx: AppContext) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let data = super::rpc_call(&ctx, "risk.summary", serde_json::json!({})).await?;
        super::print_output(&data, ctx.output_format)
    })
}
