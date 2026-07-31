//! # taiji — LVPA 量化交易终端（天机令/天机盘）
//!
//! Layer 3b CLI/TUI 终端。功能优先，信息密度高，不承载修仙视觉元素。
//! 交易员通过终端查看实时行情、运行回测、管理策略。
//!
//! ## 架构
//!
//! - `cli` crate = 命令树 + 用户交互（天机令）
//! - `commands/` = 各子命令实现（天机令正文）
//! - `output/` = 输出格式化（传音符/上界文书）
//! - `monitor/` = TUI 仪表盘（天机盘）
//! - `transport/` = 后端通信（千里传音/剑气传书）
//! - `auth/` = 认证（洞府令牌）
//! - `config/` = 配置（天书阁·凡卷）
//! - `mcp/` + `acp/` = 协议服务

mod acp;
mod auth;
mod commands;
mod config;
mod mcp;
mod monitor;
mod output;
mod transport;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use taiji_engine::node::ComputeNode;
use tracing::{info, warn};

// ── 退出码 ────────────────────────────────────────────────────────────────

/// 退出码语义（接口设计 §5.3）
///
/// | 退出码 | 含义 | 场景 |
/// |:-------|:-----|:-----|
/// | 0 | 成功 | 正常完成 |
/// | 1 | 运行时错误 | 后端不可用、数据错误、网络超时 |
/// | 2 | 输入/配置错误 | 参数格式错误、配置文件缺失 |
/// | 3 | 认证错误 | 登录失效、权限不足、API Key 无效 |
/// | 4 | 取消/超时 | Ctrl+C、操作超时（30s 默认） |
/// | 5 | 策略/风控拒绝 | 风控规则拦截、策略校验失败 |
/// | 10+ | 保留扩展 | 后续按类别细分 |
mod exit_code {
    pub(crate) const SUCCESS: i32 = 0;
    pub(crate) const RUNTIME_ERROR: i32 = 1;
    pub(crate) const INPUT_ERROR: i32 = 2;
    pub(crate) const AUTH_ERROR: i32 = 3;
    pub(crate) const TIMEOUT: i32 = 4;
    pub(crate) const RISK_REJECTED: i32 = 5;

    /// 根据错误类型映射退出码
    pub(crate) fn from_error(err: &anyhow::Error) -> i32 {
        let msg = err.to_string();

        // 优先尝试 downcast 到 TransportError
        if let Some(te) = err.downcast_ref::<crate::transport::error::TransportError>() {
            return match te {
                crate::transport::error::TransportError::Connection(_) => RUNTIME_ERROR,
                crate::transport::error::TransportError::Protocol(_) => RUNTIME_ERROR,
                crate::transport::error::TransportError::Auth(_) => AUTH_ERROR,
                crate::transport::error::TransportError::Timeout(_) => TIMEOUT,
            };
        }

        // 关键词匹配（兜底）
        if msg.contains("Auth") || msg.contains("认证") || msg.contains("TAIJI_API_KEY") || msg.contains("token") {
            AUTH_ERROR
        } else if msg.contains("超时") || msg.contains("Timeout") || msg.contains("timeout") {
            TIMEOUT
        } else if msg.contains("风控") || msg.contains("风控拒绝") || msg.contains("risk") || msg.contains("限额") {
            RISK_REJECTED
        } else if msg.contains("参数") || msg.contains("解析失败") || msg.contains("not found") || msg.contains("无效") {
            INPUT_ERROR
        } else {
            RUNTIME_ERROR
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_success_code() {
            assert_eq!(SUCCESS, 0);
        }

        #[test]
        fn test_runtime_error_code() {
            assert_eq!(RUNTIME_ERROR, 1);
        }

        #[test]
        fn test_input_error_code() {
            assert_eq!(INPUT_ERROR, 2);
        }

        #[test]
        fn test_auth_error_code() {
            assert_eq!(AUTH_ERROR, 3);
        }

        #[test]
        fn test_timeout_code() {
            assert_eq!(TIMEOUT, 4);
        }

        #[test]
        fn test_risk_rejected_code() {
            assert_eq!(RISK_REJECTED, 5);
        }

        #[test]
        fn test_from_error_runtime_fallback() {
            let err = anyhow::anyhow!("未知错误: something broke");
            assert_eq!(from_error(&err), RUNTIME_ERROR);
        }

        #[test]
        fn test_from_error_auth_keyword() {
            let err = anyhow::anyhow!("Auth failed: invalid token");
            assert_eq!(from_error(&err), AUTH_ERROR);
        }

        #[test]
        fn test_from_error_auth_chinese() {
            let err = anyhow::anyhow!("认证失败: TAIJI_API_KEY 未设置");
            assert_eq!(from_error(&err), AUTH_ERROR);
        }

        #[test]
        fn test_from_error_timeout() {
            let err = anyhow::anyhow!("请求超时: 30s");
            assert_eq!(from_error(&err), TIMEOUT);
        }

        #[test]
        fn test_from_error_risk() {
            let err = anyhow::anyhow!("风控拒绝: 超出日内亏损限额");
            assert_eq!(from_error(&err), RISK_REJECTED);
        }

        #[test]
        fn test_from_error_input() {
            let err = anyhow::anyhow!("参数错误: --symbol 是必填参数");
            assert_eq!(from_error(&err), INPUT_ERROR);
        }

        #[test]
        fn test_from_error_transport_connection() {
            let te = crate::transport::error::TransportError::Connection("refused".into());
            let err = anyhow::Error::from(te);
            assert_eq!(from_error(&err), RUNTIME_ERROR);
        }

        #[test]
        fn test_from_error_transport_auth() {
            let te = crate::transport::error::TransportError::Auth("expired".into());
            let err = anyhow::Error::from(te);
            assert_eq!(from_error(&err), AUTH_ERROR);
        }

        #[test]
        fn test_from_error_transport_timeout() {
            let te = crate::transport::error::TransportError::Timeout("5s".into());
            let err = anyhow::Error::from(te);
            assert_eq!(from_error(&err), TIMEOUT);
        }
    }
}

// ── CLI 入口 ─────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "taiji",
    version,
    about = "LVPA 量化交易终端（天机令/天机盘）",
    long_about = "\
Layer 3b CLI/TUI 终端 — 功能优先，信息密度高。
交易员通过终端查看实时行情、运行回测、管理策略。

输出格式：--output-format text|json|csv|table（默认 text）
管道友好：所有结构化输出可 pipe 给 jq/csvtool 等下游工具

示例：
  taiji quote BTC-USDT
  taiji kline BTC-USDT --period 1h --from 2026-01-01 --to 2026-06-30
  taiji backtest ma-cross --from 2026-01-01 --to 2026-06-30
  taiji monitor --symbols BTC-USDT,ETH-USDT
  taiji doctor
"
)]
struct Cli {
    /// 输出格式
    #[arg(
        long,
        value_name = "FORMAT",
        default_value = "text",
        env = "TAIJI_OUTPUT"
    )]
    output_format: output::OutputFormat,

    /// 禁用颜色输出
    #[arg(long, env = "NO_COLOR")]
    no_color: bool,

    /// 安静模式：仅输出关键结果
    #[arg(long, short = 'q')]
    quiet: bool,

    /// 详细输出（debug 级别）
    #[arg(long, short = 'v')]
    verbose: bool,

    /// 后端服务地址
    #[arg(long, value_name = "URL", env = "TAIJI_SERVER")]
    server: Option<String>,

    /// 命令
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    // ── 监控 (Monitor) ──────────────────────────────────────────────
    /// 启动 TUI 监控仪表盘（天机盘）
    #[command(name = "monitor")]
    Monitor {
        /// 布局模式
        #[arg(long, default_value = "default")]
        layout: String,

        /// 刷新率（Hz）
        #[arg(long, default_value_t = 2.0)]
        refresh_rate: f64,

        /// 监控品种列表（逗号分隔）
        #[arg(long, value_delimiter = ',')]
        symbols: Vec<String>,
    },

    /// 持续输出模式（跟踪指定数据流）
    #[command(name = "watch")]
    Watch {
        /// 监控品种
        #[arg(required = true)]
        symbol: String,

        /// 刷新间隔（秒）
        #[arg(long, default_value_t = 1.0)]
        interval: f64,

        /// 输出字段
        #[arg(long, value_delimiter = ',')]
        fields: Vec<String>,
    },

    // ── 行情 (Market) ───────────────────────────────────────────────
    /// 实时行情快照
    #[command(name = "quote")]
    Quote {
        /// 合约代码
        #[arg(required = true)]
        symbol: String,
    },

    /// 获取 K 线数据
    #[command(name = "kline")]
    Kline {
        /// 合约代码
        #[arg(required = true)]
        symbol: String,

        /// K 线周期（1m/5m/15m/1h/4h/1d/1w）
        #[arg(long, default_value = "1h")]
        period: String,

        /// 开始时间（YYYY-MM-DD）
        #[arg(long)]
        from: Option<String>,

        /// 结束时间（YYYY-MM-DD）
        #[arg(long)]
        to: Option<String>,

        /// 返回条数上限
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },

    /// 获取深度盘口
    #[command(name = "depth")]
    Depth {
        /// 合约代码
        #[arg(required = true)]
        symbol: String,

        /// 深度档位数
        #[arg(long, default_value_t = 5)]
        level: usize,
    },

    /// 可交易品种列表
    #[command(name = "list-symbols")]
    ListSymbols {
        /// 交易所过滤
        #[arg(long)]
        exchange: Option<String>,

        /// 品种类型过滤
        #[arg(long)]
        asset_type: Option<String>,
    },

    // ── 交易 (Trading) ──────────────────────────────────────────────
    /// 订单管理
    #[command(name = "order")]
    Order {
        #[command(subcommand)]
        action: OrderAction,
    },

    /// 持仓查询
    #[command(name = "position")]
    Position {
        /// 合约代码（不传则查全部）
        #[arg(long)]
        symbol: Option<String>,
    },

    /// 账户/资金信息
    #[command(name = "account")]
    Account,

    /// 成交历史
    #[command(name = "trade-history")]
    TradeHistory {
        /// 开始时间
        #[arg(long)]
        from: Option<String>,

        /// 结束时间
        #[arg(long)]
        to: Option<String>,

        /// 合约代码过滤
        #[arg(long)]
        symbol: Option<String>,
    },

    // ── 策略 (Strategy) ─────────────────────────────────────────────
    /// 策略管理
    #[command(name = "strategy")]
    Strategy {
        #[command(subcommand)]
        action: StrategyAction,
    },

    /// 回测执行
    #[command(name = "backtest")]
    Backtest {
        /// 策略 ID
        #[arg(required = true)]
        strategy_id: String,

        /// 参数（key=value 格式，可重复）
        #[arg(long, value_parser = parse_key_val)]
        param: Vec<(String, String)>,

        /// 开始时间
        #[arg(long)]
        from: Option<String>,

        /// 结束时间
        #[arg(long)]
        to: Option<String>,

        /// CSV 数据文件路径
        #[arg(long)]
        csv: Option<PathBuf>,

        /// 并行运行所有品种
        #[arg(long)]
        parallel: bool,
    },

    /// 参数优化
    #[command(name = "optimize")]
    Optimize {
        /// 策略 ID
        #[arg(required = true)]
        strategy_id: String,

        /// 优化参数范围（格式: param=min:max:step）
        #[arg(long, value_parser = parse_optimize_param)]
        param: Vec<OptimizeParamSpec>,

        /// 优化目标（sharpe/return/drawdown）
        #[arg(long, default_value = "sharpe")]
        objective: String,

        /// CSV 数据文件路径
        #[arg(long)]
        csv: Option<PathBuf>,
    },

    /// 回测报告查看
    #[command(name = "report")]
    Report {
        /// 回测 ID
        #[arg(required = true)]
        backtest_id: String,

        /// 输出格式覆盖
        #[arg(long)]
        output_format: Option<output::OutputFormat>,
    },

    // ── 风控 (Risk) ────────────────────────────────────────────────
    /// 风控状态总览
    #[command(name = "risk")]
    Risk,

    /// 限额管理
    #[command(name = "limit")]
    Limit {
        #[command(subcommand)]
        action: LimitAction,
    },

    /// 告警管理
    #[command(name = "alert")]
    Alert {
        #[command(subcommand)]
        action: AlertAction,
    },

    // ── 管理 (Admin) ────────────────────────────────────────────────
    /// 配置管理
    #[command(name = "config")]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// 日志查看
    #[command(name = "log")]
    Log {
        /// 日志级别过滤
        #[arg(long)]
        level: Option<String>,

        /// 尾部行数
        #[arg(long)]
        tail: Option<usize>,

        /// 实时跟踪
        #[arg(long)]
        follow: bool,
    },

    /// 诊断检查
    #[command(name = "doctor")]
    Doctor,

    /// 会话管理
    #[command(name = "session")]
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },

    /// 版本信息
    #[command(name = "version")]
    VersionInfo {
        /// 详细版本信息
        #[arg(long)]
        verbose: bool,
    },

    /// Shell 补全脚本生成
    #[command(name = "completion")]
    Completion {
        /// Shell 类型（bash/zsh/fish/powershell）
        #[arg(required = true)]
        shell: String,
    },

    // ── 遗留命令（原有 pipeline CLI 功能）─────────────────────────
    /// 运行管道处理（原有 pipeline 模式）
    #[command(name = "pipeline", hide = true)]
    Pipeline {
        /// 管道配置文件路径
        #[arg(long)]
        config: PathBuf,

        /// CSV 数据文件路径
        #[arg(long)]
        csv: PathBuf,

        /// 输出文件路径
        #[arg(long)]
        output: Option<PathBuf>,

        /// 跳过前 N 行
        #[arg(long, default_value_t = 0)]
        resume: usize,
    },

    /// 生成交易信号
    #[command(name = "signal", hide = true)]
    Signal {
        /// 管道配置文件路径
        #[arg(long)]
        config: PathBuf,

        /// CSV 数据文件路径
        #[arg(long)]
        csv: PathBuf,

        /// 输出文件路径
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// 分析 CSV 数据
    #[command(name = "analyze", hide = true)]
    Analyze {
        /// CSV 数据文件路径
        #[arg(long)]
        csv: PathBuf,

        /// 品种过滤
        #[arg(long)]
        symbol: Option<String>,
    },

    /// 启动 MCP 服务器
    #[command(name = "mcp", hide = true)]
    Mcp,

    /// 启动 ACP 服务器
    #[command(name = "acp", hide = true)]
    Acp,

    /// 认证管理
    #[command(name = "auth", hide = true)]
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
}

// ── 子命令枚举 ───────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
enum OrderAction {
    /// 列出订单
    List {
        /// 合约代码过滤
        #[arg(long)]
        symbol: Option<String>,
        /// 状态过滤（pending/filled/canceled/all）
        #[arg(long, default_value = "all")]
        status: String,
    },
    /// 创建订单
    Create {
        /// 合约代码
        #[arg(long)]
        symbol: String,
        /// 买卖方向
        #[arg(long)]
        side: String,
        /// 数量
        #[arg(long)]
        qty: f64,
        /// 价格（市价单不传）
        #[arg(long)]
        price: Option<f64>,
        /// 订单类型
        #[arg(long, default_value = "limit")]
        order_type: String,
    },
    /// 取消订单
    Cancel {
        /// 订单 ID
        #[arg(required = true)]
        order_id: String,
    },
    /// 修改订单
    Modify {
        /// 订单 ID
        #[arg(required = true)]
        order_id: String,
        /// 新价格
        #[arg(long)]
        price: Option<f64>,
        /// 新数量
        #[arg(long)]
        qty: Option<f64>,
    },
}

#[derive(Subcommand, Debug)]
enum StrategyAction {
    /// 列出策略
    List,
    /// 部署策略
    Deploy {
        /// 策略 ID
        #[arg(required = true)]
        strategy_id: String,
        /// 参数（key=value）
        #[arg(long, value_parser = parse_key_val)]
        param: Vec<(String, String)>,
    },
    /// 启动策略
    Start {
        /// 策略实例 ID
        #[arg(required = true)]
        instance_id: String,
    },
    /// 停止策略
    Stop {
        /// 策略实例 ID
        #[arg(required = true)]
        instance_id: String,
    },
    /// 查看/修改策略参数
    Param {
        /// 策略实例 ID
        #[arg(required = true)]
        instance_id: String,
        /// 参数名
        #[arg(long)]
        get: Option<String>,
        /// 设置参数（key=value）
        #[arg(long, value_parser = parse_key_val)]
        set: Vec<(String, String)>,
    },
}

#[derive(Subcommand, Debug)]
enum LimitAction {
    /// 列出限额
    List,
    /// 设置限额
    Set {
        /// 限额类型（exposure/margin/loss/position）
        #[arg(required = true)]
        limit_type: String,
        /// 限额值
        #[arg(required = true)]
        value: f64,
    },
    /// 移除限额
    Remove {
        /// 限额类型
        #[arg(required = true)]
        limit_type: String,
    },
}

#[derive(Subcommand, Debug)]
enum AlertAction {
    /// 列出告警
    List {
        /// 状态过滤
        #[arg(long, default_value = "all")]
        status: String,
    },
    /// 确认告警
    Ack {
        /// 告警 ID
        #[arg(required = true)]
        alert_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// 查看当前配置（合并后）
    List,
    /// 查看单个配置项
    Get {
        /// 配置键名
        #[arg(required = true)]
        key: String,
    },
    /// 设置配置项
    Set {
        /// 配置键名
        #[arg(required = true)]
        key: String,
        /// 配置值
        #[arg(required = true)]
        value: String,
    },
    /// 打开编辑器编辑配置文件
    Edit,
}

#[derive(Subcommand, Debug)]
enum SessionAction {
    /// 列出会话
    List,
    /// 查看会话详情
    Get {
        /// 会话 ID
        #[arg(required = true)]
        session_id: String,
    },
    /// 关闭会话
    Close {
        /// 会话 ID
        #[arg(required = true)]
        session_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum AuthAction {
    /// 设备码登录
    Login,
    /// 清除本地 token
    Logout,
    /// 查看当前认证状态
    Status,
}

// ── 参数解析辅助 ─────────────────────────────────────────────────────────

/// 解析 key=value 格式参数
fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("无效的 key=value 格式: `{s}`"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

/// 参数优化规格（param=min:max:step）
#[derive(Debug, Clone)]
struct OptimizeParamSpec {
    name: String,
    min: f64,
    max: f64,
    step: f64,
}

fn parse_optimize_param(s: &str) -> Result<OptimizeParamSpec, String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(format!("无效的优化参数格式 (需要 param=min:max:step): `{s}`"));
    }
    let name = parts[0].to_string();
    let range_parts: Vec<&str> = parts[1].splitn(3, ':').collect();
    if range_parts.len() != 3 {
        return Err(format!("无效的范围格式 (需要 min:max:step): `{}`", parts[1]));
    }
    let min = range_parts[0]
        .parse::<f64>()
        .map_err(|e| format!("无效的 min 值: {}", e))?;
    let max = range_parts[1]
        .parse::<f64>()
        .map_err(|e| format!("无效的 max 值: {}", e))?;
    let step = range_parts[2]
        .parse::<f64>()
        .map_err(|e| format!("无效的 step 值: {}", e))?;
    Ok(OptimizeParamSpec { name, min, max, step })
}

// ── 主入口（带退出码）────────────────────────────────────────────────────

fn main() {
    let code = match run_main() {
        Ok(_) => exit_code::SUCCESS,
        Err(e) => {
            // 错误信息已由各命令输出到 stderr，这里只映射退出码
            exit_code::from_error(&e)
        }
    };
    std::process::exit(code);
}

/// 实际主逻辑，返回 Result（退出码在 main() 中映射）
fn run_main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    // 构建应用上下文
    let output_format = cli.output_format;
    let server_url = cli.server.unwrap_or_else(|| "http://127.0.0.1:9527".to_string());
    let ctx = AppContext {
        output_format,
        no_color: cli.no_color,
        quiet: cli.quiet,
        verbose: cli.verbose,
        server_url,
    };

    match cli.command {
        // ── 监控 ──
        Some(Command::Monitor {
            layout,
            refresh_rate,
            symbols,
        }) => commands::monitor::run(ctx, layout, refresh_rate, symbols),

        Some(Command::Watch {
            symbol,
            interval,
            fields,
        }) => commands::watch::run(ctx, symbol, interval, fields),

        // ── 行情 ──
        Some(Command::Quote { symbol }) => commands::quote::run(ctx, symbol),
        Some(Command::Kline {
            symbol,
            period,
            from,
            to,
            limit,
        }) => commands::kline::run(ctx, symbol, period, from, to, limit),
        Some(Command::Depth { symbol, level }) => commands::depth::run(ctx, symbol, level),
        Some(Command::ListSymbols {
            exchange,
            asset_type,
        }) => commands::list_symbols::run(ctx, exchange, asset_type),

        // ── 交易 ──
        Some(Command::Order { action }) => commands::order::run(ctx, action),
        Some(Command::Position { symbol }) => commands::position::run(ctx, symbol),
        Some(Command::Account) => commands::account::run(ctx),
        Some(Command::TradeHistory { from, to, symbol }) => {
            commands::trade_history::run(ctx, from, to, symbol)
        }

        // ── 策略 ──
        Some(Command::Strategy { action }) => commands::strategy::run(ctx, action),
        Some(Command::Backtest {
            strategy_id,
            param,
            from,
            to,
            csv,
            parallel,
        }) => commands::backtest::run(ctx, strategy_id, param, from, to, csv, parallel),
        Some(Command::Optimize {
            strategy_id,
            param,
            objective,
            csv,
        }) => commands::optimize::run(ctx, strategy_id, param, objective, csv),
        Some(Command::Report {
            backtest_id,
            output_format,
        }) => commands::report::run(ctx, backtest_id, output_format),

        // ── 风控 ──
        Some(Command::Risk) => commands::risk::run(ctx),
        Some(Command::Limit { action }) => commands::limit::run(ctx, action),
        Some(Command::Alert { action }) => commands::alert::run(ctx, action),

        // ── 管理 ──
        Some(Command::Config { action }) => commands::config_cmd::run(ctx, action),
        Some(Command::Log {
            level,
            tail,
            follow,
        }) => commands::log::run(ctx, level, tail, follow),
        Some(Command::Doctor) => commands::doctor::run(ctx),
        Some(Command::Session { action }) => commands::session::run(ctx, action),
        Some(Command::VersionInfo { verbose }) => commands::version::run(ctx, verbose),
        Some(Command::Completion { shell }) => commands::completion::run(ctx, shell),

        // ── 遗留命令 ──
        Some(Command::Pipeline {
            config,
            csv,
            output,
            resume,
        }) => run_pipeline(&config, &csv, &output, resume),
        Some(Command::Signal { config, csv, output }) => run_legacy_signal(&config, &csv, &output),
        Some(Command::Analyze { csv, symbol }) => run_legacy_analyze(&csv, symbol.as_deref()),
        Some(Command::Mcp) => mcp::run_mcp_server_sync(load_legacy_config()?),
        Some(Command::Acp) => acp::run_acp_server(load_legacy_config()?),
        Some(Command::Auth { action }) => run_legacy_auth(action),

        None => {
            // 无命令时打印帮助
            use clap::CommandFactory;
            Cli::command().print_help()?;
            println!();
            Ok(())
        }
    }
}

// ── 应用上下文 ────────────────────────────────────────────────────────────

/// CLI 应用上下文，传递给各子命令
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct AppContext {
    pub(crate) output_format: output::OutputFormat,
    pub(crate) no_color: bool,
    pub(crate) quiet: bool,
    pub(crate) verbose: bool,
    pub(crate) server_url: String,
}

// ── 遗留函数（保持原有 pipeline CLI 兼容）──────────────────────────────

fn load_legacy_config() -> Result<config::ResolvedConfig> {
    let (_tier, cfg) = auth::resolve_tier()
        .map_err(|e| anyhow::anyhow!("Auth error: {}", e))?;
    Ok(cfg)
}

fn run_legacy_signal(config_path: &Path, csv_path: &Path, output_path: &Option<PathBuf>) -> Result<()> {
    info!(
        "taiji signal --config {} --csv {}",
        config_path.display(),
        csv_path.display()
    );
    if let Some(out) = output_path {
        info!("  output: {}", out.display());
    }
    // TODO(R1.2): full signal generation implementation
    Ok(())
}

fn run_legacy_analyze(csv_path: &Path, _symbol: Option<&str>) -> Result<()> {
    info!("taiji analyze --csv {}", csv_path.display());
    // TODO(R1.3): full analysis implementation
    Ok(())
}

fn run_legacy_auth(action: AuthAction) -> Result<()> {
    let cmd = match action {
        AuthAction::Login => AuthCommand::Login,
        AuthAction::Logout => AuthCommand::Logout,
        AuthAction::Status => AuthCommand::Status,
    };
    auth::handle_auth_command(cmd).map_err(|e| anyhow::anyhow!("Auth error: {}", e))
}

/// 认证子命令（用于 auth.rs 兼容）
#[derive(Debug, Clone)]
pub(crate) enum AuthCommand {
    Login,
    Logout,
    Status,
}

/// 运行管道处理（从原有 pipeline CLI 保留）
fn run_pipeline(
    config_path: &PathBuf,
    csv_path: &PathBuf,
    output_path: &Option<PathBuf>,
    resume: usize,
) -> Result<()> {
    use std::collections::HashMap;

    let yaml_str = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;
    let pipeline_config = taiji_engine::config::PipelineConfig::from_yaml(&yaml_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))?;

    // Depth-limited YAML check
    if let Err(e) = taiji_engine::safe_json::from_yaml_str_limited::<serde_json::Value>(&yaml_str) {
        anyhow::bail!("Config YAML exceeds depth limit: {}", e);
    }

    info!("Pipeline: {} v{}", pipeline_config.name, pipeline_config.version);

    let mut pipeline = taiji_engine::pipeline::Pipeline::from_config(pipeline_config.clone())
        .map_err(|e| anyhow::anyhow!("Failed to create pipeline: {}", e))?;

    let mut factory = taiji_engine::factory::NodeFactory::new();
    factory.register(
        "ma_cross",
        Box::new(
            |_config: &taiji_engine::node::NodeConfig| -> taiji_engine::error::Result<Box<dyn taiji_engine::node::ComputeNode>> {
                let mut node = taiji_example::MaCross::new("ma_cross");
                let store = taiji_engine::store::StateStore::new();
                node.on_init(_config, &store)?;
                Ok(Box::new(node))
            },
        ),
    );
    factory.register(
        "BarNode",
        Box::new(
            |_config: &taiji_engine::node::NodeConfig| -> taiji_engine::error::Result<Box<dyn taiji_engine::node::ComputeNode>> {
                let id = _config.get_str("id").unwrap_or("bar_node");
                let mut node = taiji_bar::BarNode::new(id.to_string());
                let store = taiji_engine::store::StateStore::new();
                node.on_init(_config, &store)?;
                Ok(Box::new(node))
            },
        ),
    );

    for spec in &pipeline_config.nodes {
        let params: HashMap<String, serde_json::Value> =
            if let serde_json::Value::Object(map) = &spec.config {
                map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
            } else {
                HashMap::new()
            };
        let node_config = taiji_engine::node::NodeConfig {
            type_name: spec.type_name.clone(),
            params,
        };
        let mut node = factory.create(&spec.type_name, &node_config)
            .map_err(|e| anyhow::anyhow!("Failed to create node '{}': {}", spec.id, e))?;
        let store = taiji_engine::store::StateStore::new();
        node.on_init(&node_config, &store)
            .map_err(|e| anyhow::anyhow!("Failed to init node '{}': {}", spec.id, e))?;
        info!("  + node: id={}, type={}", spec.id, spec.type_name);
        pipeline.add_node(node);
    }

    pipeline.derive_edges()
        .map_err(|e| anyhow::anyhow!("Failed to derive DAG edges: {}", e))?;

    let csv_content = std::fs::read_to_string(csv_path)
        .with_context(|| format!("Failed to read CSV: {}", csv_path.display()))?;

    let lines: Vec<&str> = csv_content.lines().collect();
    if lines.is_empty() {
        anyhow::bail!("CSV file is empty");
    }

    let header_fields: Vec<String> = lines[0].split(',').map(|s| s.trim().to_string()).collect();
    let mut column_map: HashMap<String, usize> = HashMap::new();
    for (i, col) in header_fields.iter().enumerate() {
        column_map.insert(col.clone(), i);
    }

    let mut all_signals: Vec<taiji_engine::types::signal::Signal> = Vec::new();
    let mut ticks_processed: u64 = 0;

    for line in lines.iter().skip(1 + resume) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        let instrument = column_map.get("instrument")
            .or_else(|| column_map.get("symbol"))
            .and_then(|&idx| fields.get(idx))
            .unwrap_or(&"")
            .to_string();
        let price = column_map.get("price")
            .and_then(|&idx| fields.get(idx))
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        let tick = taiji_engine::types::tick::TickData {
            instrument,
            last_price: price.max(0.0),
            ..Default::default()
        };

        match pipeline.feed_tick_direct(&tick) {
            Ok(result) => {
                ticks_processed += 1;
                all_signals.extend(result.signals);
            }
            Err(e) => {
                warn!("pipeline error: {}", e);
            }
        }
    }

    info!("Done: {} ticks, {} signals", ticks_processed, all_signals.len());

    let signals_json = serde_json::to_string_pretty(&all_signals)?;
    if let Some(output_path) = output_path {
        std::fs::write(output_path, &signals_json)?;
    } else {
        println!("{}", signals_json);
    }

    Ok(())
}
