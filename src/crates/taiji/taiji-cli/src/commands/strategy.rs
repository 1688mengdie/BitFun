//! `taiji strategy` — 策略管理

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(ctx: AppContext, action: super::super::StrategyAction) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match action {
            super::super::StrategyAction::List => {
                let data = super::rpc_call(&ctx, "strategy.list", serde_json::json!({})).await?;
                super::print_output(&data, ctx.output_format)
            }
            super::super::StrategyAction::Deploy {
                strategy_id,
                param,
            } => {
                let params: serde_json::Value =
                    serde_json::to_value(param).unwrap_or(serde_json::json!({}));
                let data = super::rpc_call(
                    &ctx,
                    "strategy.deploy",
                    serde_json::json!({ "strategy_id": strategy_id, "params": params }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            }
            super::super::StrategyAction::Start { instance_id } => {
                let data = super::rpc_call(
                    &ctx,
                    "strategy.start",
                    serde_json::json!({ "instance_id": instance_id }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            }
            super::super::StrategyAction::Stop { instance_id } => {
                let data = super::rpc_call(
                    &ctx,
                    "strategy.stop",
                    serde_json::json!({ "instance_id": instance_id }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            }
            super::super::StrategyAction::Param {
                instance_id,
                get,
                set,
            } => {
                let data = super::rpc_call(
                    &ctx,
                    "strategy.param",
                    serde_json::json!({
                        "instance_id": instance_id,
                        "get": get,
                        "set": set,
                    }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            }
        }
    })
}
