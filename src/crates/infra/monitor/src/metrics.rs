//! 天眼阵 — 指标注册表。
//!
//! 定义指标类型（Counter/Gauge/Histogram）、指标元数据、预定义 P0/P1 指标集。
//!
//! 设计参考：BitFun `docs/sdlc-harness/governance/metrics-spec.md §2-4` P0-P4 指标分级模式，
//! LVPA 自定 P0/P1 指标集，Rust 翻译实现，非 Cargo 依赖。

use serde::{Deserialize, Serialize};

/// 指标类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricKind {
    /// 计数器（只增不减，如请求总数）。
    Counter,
    /// 仪表（可增可减，如当前连接数）。
    Gauge,
    /// 直方图（延迟分布，如请求延迟）。
    Histogram,
}

/// 指标元数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDef {
    /// 指标名称（如 `"request.total"`）。
    pub name: &'static str,
    /// 指标类型。
    pub kind: MetricKind,
    /// 指标描述。
    pub description: &'static str,
    /// 计量单位（如 "count", "ms", "percent"）。
    pub unit: &'static str,
    /// P0 核心指标标记。
    pub p0: bool,
}

/// 预定义 P0/P1 指标清单。
///
/// P0 指标（必须采集）：
/// - system.uptime_secs — 系统运行时间
/// - module.healthy — 模块健康状态（1=healthy, 0=down）
/// - request.total — 总请求数
/// - request.errors — 请求错误数
/// - request.latency_ms — 请求延迟
/// - token.consumption — Token 消耗
///
/// P1 指标（按需采集）：
/// - harness.deny_count — 护山大阵拦截次数
/// - alert.fired — 告警触发次数
pub const BUILTIN_METRICS: &[MetricDef] = &[
    // === P0 核心指标 ===
    MetricDef { name: "system.uptime_secs", kind: MetricKind::Gauge, description: "系统运行时间", unit: "secs", p0: true },
    MetricDef { name: "module.healthy", kind: MetricKind::Gauge, description: "模块健康状态 (1=healthy, 0=down)", unit: "bool", p0: true },
    MetricDef { name: "request.total", kind: MetricKind::Counter, description: "总请求数", unit: "count", p0: true },
    MetricDef { name: "request.errors", kind: MetricKind::Counter, description: "请求错误数", unit: "count", p0: true },
    MetricDef { name: "request.latency_ms", kind: MetricKind::Histogram, description: "请求延迟", unit: "ms", p0: true },
    MetricDef { name: "token.consumption", kind: MetricKind::Counter, description: "Token 消耗数", unit: "count", p0: true },
    // === P1 业务指标 ===
    MetricDef { name: "harness.deny_count", kind: MetricKind::Counter, description: "护山大阵拦截次数", unit: "count", p0: false },
    MetricDef { name: "alert.fired", kind: MetricKind::Counter, description: "告警触发次数", unit: "count", p0: false },
];

/// 查找预定义指标定义。
pub fn find_metric(name: &str) -> Option<&'static MetricDef> {
    BUILTIN_METRICS.iter().find(|m| m.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_metrics_count() {
        assert_eq!(BUILTIN_METRICS.len(), 8, "应有 8 个预定义指标");
    }

    #[test]
    fn test_p0_metrics_count() {
        let p0_count = BUILTIN_METRICS.iter().filter(|m| m.p0).count();
        assert_eq!(p0_count, 6, "应有 6 个 P0 核心指标");
    }

    #[test]
    fn test_find_metric() {
        assert!(find_metric("request.total").is_some());
        assert!(find_metric("nonexistent").is_none());
    }
}
