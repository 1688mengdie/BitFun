//! `taiji alert` — 告警管理

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(ctx: AppContext, action: super::super::AlertAction) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match action {
            super::super::AlertAction::List { status } => {
                let data = super::rpc_call(
                    &ctx,
                    "risk.alerts",
                    serde_json::json!({ "status": status }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            }
            super::super::AlertAction::Ack { alert_id } => {
                let data = super::rpc_call(
                    &ctx,
                    "risk.alert.ack",
                    serde_json::json!({ "alert_id": alert_id }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            }
        }
    })
}
