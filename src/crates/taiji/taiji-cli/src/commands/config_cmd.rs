//! `taiji config` — 配置管理

use std::process::Command;

use crate::AppContext;
use anyhow::{Context, Result};

pub(crate) fn run(ctx: AppContext, action: super::super::ConfigAction) -> Result<()> {
    match action {
        super::super::ConfigAction::List => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let data = super::rpc_call(&ctx, "system.config", serde_json::json!({})).await?;
                super::print_output(&data, ctx.output_format)
            })
        }
        super::super::ConfigAction::Get { key } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let data = super::rpc_call(
                    &ctx,
                    "system.config.get",
                    serde_json::json!({ "key": key }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            })
        }
        super::super::ConfigAction::Set { key, value } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let data = super::rpc_call(
                    &ctx,
                    "system.config.set",
                    serde_json::json!({ "key": key, "value": value }),
                )
                .await?;
                super::print_output(&data, ctx.output_format)
            })
        }
        super::super::ConfigAction::Edit => {
            let config_path = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("taiji")
                .join("config.toml");
            if !config_path.exists() {
                std::fs::create_dir_all(config_path.parent().unwrap())?;
                std::fs::write(
                    &config_path,
                    "# taiji 配置文件\n[server]\nurl = \"http://127.0.0.1:9527\"\ntimeout = 30\n\n[output]\nformat = \"text\"\ncolor = \"auto\"\n",
                )?;
            }
            let editor = std::env::var("EDITOR")
                .or_else(|_| std::env::var("VISUAL"))
                .unwrap_or_else(|_| {
                    if cfg!(windows) { "notepad".to_string() } else { "vim".to_string() }
                });
            let status = Command::new(&editor)
                .arg(&config_path)
                .status()
                .with_context(|| format!("Failed to launch editor: {}", editor))?;
            if !status.success() {
                anyhow::bail!("编辑器退出码: {:?}", status.code());
            }
            eprintln!("配置已保存: {}", config_path.display());
            Ok(())
        }
    }
}
