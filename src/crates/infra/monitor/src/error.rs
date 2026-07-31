//! 天眼阵 — 监控系统错误类型。
//!
//! MonitorError 覆盖指标采集、健康检查、告警三类操作错误。

use thiserror::Error;

/// 监控系统错误。
#[derive(Debug, Error)]
pub enum MonitorError {
    /// 指标采集错误（counter/gauge/histogram 操作失败）。
    #[error("指标采集错误: {0}")]
    MetricError(String),

    /// 健康检查错误。
    #[error("健康检查错误: {0}")]
    HealthError(String),

    /// 告警引擎错误。
    #[error("告警引擎错误: {0}")]
    AlertError(String),

    /// 指标未注册。
    #[error("指标未注册: {0}")]
    MetricNotFound(String),

    /// 内部错误。
    #[error("内部错误: {0}")]
    Internal(String),
}

/// 监控系统 Result 别名。
pub type MonitorResult<T> = Result<T, MonitorError>;
