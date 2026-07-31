//! `taiji order` — 订单管理

use crate::AppContext;
use anyhow::Result;

/// 订单操作类型（从 main.rs 重导出）
#[allow(unused)]
pub(crate) use super::super::OrderAction;

pub(crate) fn run(ctx: AppContext, action: super::super::OrderAction) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match action {
            super::super::OrderAction::List { symbol, status } => {
                let data = super::rpc_call(
                    &ctx,
                    "order.list",
                    serde_json::json!({ "symbol": symbol, "status": status }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            }
            super::super::OrderAction::Create {
                symbol,
                side,
                qty,
                price,
                order_type,
            } => {
                let data = super::rpc_call(
                    &ctx,
                    "order.create",
                    serde_json::json!({
                        "symbol": symbol,
                        "side": side,
                        "qty": qty,
                        "price": price,
                        "order_type": order_type,
                    }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            }
            super::super::OrderAction::Cancel { order_id } => {
                let data = super::rpc_call(
                    &ctx,
                    "order.cancel",
                    serde_json::json!({ "order_id": order_id }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            }
            super::super::OrderAction::Modify {
                order_id,
                price,
                qty,
            } => {
                let data = super::rpc_call(
                    &ctx,
                    "order.modify",
                    serde_json::json!({
                        "order_id": order_id,
                        "price": price,
                        "qty": qty,
                    }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            }
        }
    })
}
