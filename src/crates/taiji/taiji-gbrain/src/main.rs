//! taiji-gbrain CLI binary entry point.
//!
//! Delegates to `taiji_gbrain::cli` for command parsing and execution.

use clap::Parser;
use taiji_gbrain::cli::Cli;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    taiji_gbrain::cli::execute(&cli)
}
