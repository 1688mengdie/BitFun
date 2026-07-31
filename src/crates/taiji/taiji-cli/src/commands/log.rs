//! `taiji log` — 日志查看

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(
    ctx: AppContext,
    level: Option<String>,
    tail: Option<usize>,
    follow: bool,
) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let data = super::rpc_call(
            &ctx,
            "log.tail",
            serde_json::json!({ "level": level, "tail": tail, "follow": follow }),
        )
        .await?;
        super::print_output(&data, ctx.output_format)
    })
}
