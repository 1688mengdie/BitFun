//! `taiji account` — 账户/资金信息

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(ctx: AppContext) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let data = super::rpc_call(&ctx, "account.info", serde_json::json!({})).await?;
        super::print_output(&data, ctx.output_format)
    })
}
