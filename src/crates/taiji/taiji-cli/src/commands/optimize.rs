//! `taiji optimize <strategy-id>` — 参数优化

use std::path::PathBuf;

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(
    ctx: AppContext,
    strategy_id: String,
    param: Vec<super::super::OptimizeParamSpec>,
    objective: String,
    _csv: Option<PathBuf>,
) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let params: Vec<serde_json::Value> = param
            .into_iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "min": p.min,
                    "max": p.max,
                    "step": p.step,
                })
            })
            .collect();

        let data = super::rpc_call(
            &ctx,
            "strategy.optimize",
            serde_json::json!({
                "strategy_id": strategy_id,
                "params": params,
                "objective": objective,
            }),
        )
        .await?;
        super::print_output(&data, ctx.output_format)
    })
}
