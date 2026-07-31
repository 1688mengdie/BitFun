//! `taiji doctor` — 诊断检查

use crate::transport::TransportClient;
use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(ctx: AppContext) -> Result<()> {
    eprintln!("🔍 taiji 诊断检查");
    eprintln!("==================");

    // 1. 检查后端连接
    eprint!("[*] 检查后端连接 ({})... ", ctx.server_url);
    let rt = tokio::runtime::Runtime::new()?;
    let healthy = rt.block_on(async {
        let client = crate::transport::create_client(&ctx.server_url);
        match client.health().await {
            Ok(status) => {
                eprintln!("OK ({})", status);
                true
            }
            Err(e) => {
                eprintln!("失败: {}", e);
                false
            }
        }
    });

    // 2. 检查配置
    eprintln!("[*] 检查配置文件...");
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("taiji");
    let config_path = config_dir.join("config.toml");
    if config_path.exists() {
        eprintln!("    ✓ 配置文件: {}", config_path.display());
    } else {
        eprintln!("    - 无配置文件（使用默认值）");
    }

    // 3. 检查环境变量
    eprintln!("[*] 检查环境变量...");
    if let Ok(key) = std::env::var("TAIJI_API_KEY") {
        eprintln!("    ✓ TAIJI_API_KEY 已设置 (前缀: {}...)", &key[..7.min(key.len())]);
    } else {
        eprintln!("    - TAIJI_API_KEY 未设置（免费版）");
    }

    let output_env = std::env::var("TAIJI_OUTPUT").unwrap_or_else(|_| "text".to_string());
    eprintln!("    ✓ TAIJI_OUTPUT = {}", output_env);

    // 4. 总结
    eprintln!("==================");
    if healthy {
        eprintln!("✓ 系统状态: 正常");
        Ok(())
    } else {
        eprintln!("⚠ 系统状态: 后端不可用（CLI 可独立运行，但行情/交易命令不可用）");
        Ok(())
    }
}
