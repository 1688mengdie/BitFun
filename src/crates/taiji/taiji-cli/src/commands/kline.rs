//! `taiji kline <symbol>` — K 线数据

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(
    ctx: AppContext,
    symbol: String,
    period: String,
    from: Option<String>,
    to: Option<String>,
    limit: usize,
) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let data = super::rpc_call(
            &ctx,
            "market.kline",
            serde_json::json!({
                "symbol": &symbol,
                "period": &period,
                "from": from,
                "to": to,
                "limit": limit,
            }),
        )
        .await?;
        super::print_output(&data, ctx.output_format)
    })
}
