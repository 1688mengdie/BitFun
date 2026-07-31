//! `taiji watch` — 持续输出模式

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(
    ctx: AppContext,
    symbol: String,
    interval: f64,
    _fields: Vec<String>,
) -> Result<()> {
    eprintln!("持续监控 {} (刷新间隔: {}s)...", symbol, interval);
    eprintln!("按 Ctrl+C 退出");

    // 简单轮询模式
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        loop {
            match super::rpc_call(
                &ctx,
                "market.quote",
                serde_json::json!({ "symbol": &symbol }),
            )
            .await
            {
                Ok(data) => {
                    super::print_output(&data, ctx.output_format)?;
                }
                Err(e) => {
                    if !ctx.quiet {
                        eprintln!("获取数据失败: {}", e);
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs_f64(interval)).await;
        }
    })
}
