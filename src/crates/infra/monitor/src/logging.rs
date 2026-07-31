//! 天眼阵 — 日志分层定义。
//!
//! 基于 tracing crate 的 target 机制，按模块划分日志目标。

/// 日志目标（tracing target）定义。
///
/// 每个 LVPA 模块使用独立的 target，便于分级过滤。
pub mod targets {
    /// 天书阁（配置系统）。
    pub const CONFIG: &str = "lvpa::config";
    /// 天眼阵（自身）。
    pub const MONITOR: &str = "lvpa::monitor";
    /// 护山大阵（权限门控）。
    pub const HARNESS: &str = "lvpa::harness";
    /// 计算引擎。
    pub const ENGINE: &str = "lvpa::engine";
    /// 量化交易。
    pub const TRADING: &str = "lvpa::trading";
    /// 任务堂（事件总线）。
    pub const EVENT_BUS: &str = "lvpa::event_bus";
    /// 传音符（消息总线）。
    pub const MESSAGE_BUS: &str = "lvpa::msg_bus";
    /// 接引台（网关）。
    pub const GATEWAY: &str = "lvpa::gateway";
    /// 灵脉（数据库存储）。
    pub const DB_STORE: &str = "lvpa::db_store";
    /// 传音阵（传输层）。
    pub const TRANSPORT: &str = "lvpa::transport";
}

/// 初始化默认日志订阅者（开发环境用）。
///
/// 默认输出到 stderr，格式为 `[target] level message`。
/// 可通过环境变量 `LVPA_LOG_LEVEL` 控制日志级别（默认 info）。
pub fn init_default_subscriber() {
    let level = std::env::var("LVPA_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level))
        )
        .with_target(true)
        .with_thread_ids(false)
        .init();
}
