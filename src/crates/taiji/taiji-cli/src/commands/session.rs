//! `taiji session` — 会话管理

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(ctx: AppContext, action: super::super::SessionAction) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match action {
            super::super::SessionAction::List => {
                let data = super::rpc_call(&ctx, "session.list", serde_json::json!({})).await?;
                super::print_output(&data, ctx.output_format)
            }
            super::super::SessionAction::Get { session_id } => {
                let data = super::rpc_call(
                    &ctx,
                    "session.get",
                    serde_json::json!({ "session_id": session_id }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            }
            super::super::SessionAction::Close { session_id } => {
                let data = super::rpc_call(
                    &ctx,
                    "session.close",
                    serde_json::json!({ "session_id": session_id }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            }
        }
    })
}
