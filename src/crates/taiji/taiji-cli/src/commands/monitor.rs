//! `taiji monitor` — TUI 监控仪表盘（天机盘）

use crate::AppContext;
use anyhow::Result;

/// 启动 TUI 监控仪表盘
pub(crate) fn run(
    ctx: AppContext,
    layout: String,
    refresh_rate: f64,
    symbols: Vec<String>,
) -> Result<()> {
    // TUI 仪表盘需要终端支持
    crate::monitor::run_dashboard(ctx, layout, refresh_rate, symbols)
}
