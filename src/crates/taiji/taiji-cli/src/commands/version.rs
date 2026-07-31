//! `taiji version` — 版本信息

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(_ctx: AppContext, verbose: bool) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let name = env!("CARGO_PKG_NAME");

    if verbose {
        println!("{} v{}", name, version);
        println!("  Layer: 3b (CLI/TUI)");
        println!("  Framework: clap + ratatui + crossterm");
        println!("  Transport: JSON-RPC + WebSocket");
        #[cfg(debug_assertions)]
        println!("  Build: debug");
        #[cfg(not(debug_assertions))]
        println!("  Build: release");
        println!("  Target: {}", std::env::consts::ARCH);
        println!("  OS: {}", std::env::consts::OS);
    } else {
        println!("{} v{}", name, version);
    }
    Ok(())
}
