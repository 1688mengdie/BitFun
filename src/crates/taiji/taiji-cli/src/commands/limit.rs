//! `taiji limit` — 限额管理

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(ctx: AppContext, action: super::super::LimitAction) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match action {
            super::super::LimitAction::List => {
                let data = super::rpc_call(&ctx, "risk.limits", serde_json::json!({})).await?;
                super::print_output(&data, ctx.output_format)
            }
            super::super::LimitAction::Set { limit_type, value } => {
                let data = super::rpc_call(
                    &ctx,
                    "risk.limit.set",
                    serde_json::json!({ "limit_type": limit_type, "value": value }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            }
            super::super::LimitAction::Remove { limit_type } => {
                let data = super::rpc_call(
                    &ctx,
                    "risk.limit.remove",
                    serde_json::json!({ "limit_type": limit_type }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            }
        }
    })
}
