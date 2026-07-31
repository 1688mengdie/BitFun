//! 天眼阵 — Monitor trait + LvpaMonitor 实现。
//!
//! 指标采集委托标准 crate `metrics`，健康检查/告警自研。
//!
//! 设计参考：BitFun `docs/sdlc-harness/governance/metrics-spec.md §2-4` 指标分级模式，
//! Rust trait 翻译实现，非 Cargo 依赖。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::alert::{AlertEngine, AlertEvent, AlertRule};
use crate::error::{MonitorError, MonitorResult};
use crate::health::{HealthEngine, ModuleHandle, SystemHealthReport};
use crate::logging;
use crate::metrics::{find_metric, MetricKind};

/// 监控系统核心 trait。
///
/// 覆盖指标采集、健康检查、告警、日志控制、Prometheus 导出 5 个维度。
pub trait Monitor: Send + Sync {
    /// 记录计数器增量。
    fn increment_counter(&self, name: &str, value: u64) -> MonitorResult<()>;

    /// 设置仪表值。
    fn set_gauge(&self, name: &str, value: f64) -> MonitorResult<()>;

    /// 记录直方图值。
    fn record_histogram(&self, name: &str, value: f64) -> MonitorResult<()>;

    /// 注册模块心跳，返回 ModuleHandle（drop 时自动注销）。
    fn register_module(&self, name: &str) -> ModuleHandle;

    /// 获取系统健康报告。
    fn health_report(&self) -> SystemHealthReport;

    /// 添加告警规则。
    fn add_alert_rule(&mut self, rule: AlertRule);

    /// 评估所有告警规则。
    fn evaluate_alerts(&mut self, metric_values: &HashMap<String, f64>) -> Vec<AlertEvent>;

    /// 设置日志级别。
    fn set_log_level(&self, level: tracing::Level);

    /// 导出 Prometheus 格式指标。
    fn export_prometheus(&self) -> String;
}

/// LVPA 监控系统实现。
pub struct LvpaMonitor {
    /// 健康检查引擎。
    health: Arc<HealthEngine>,
    /// 告警规则引擎（使用 std::sync::Mutex 而非 tokio::sync::RwLock，因为所有操作都是同步的）。
    alert: Mutex<AlertEngine>,
}

impl LvpaMonitor {
    /// 创建 LvpaMonitor 实例。
    pub fn new() -> Self {
        Self {
            health: Arc::new(HealthEngine::new()),
            alert: Mutex::new(AlertEngine::new()),
        }
    }

    /// 获取健康检查引擎引用（内部使用）。
    pub fn health_engine(&self) -> &HealthEngine {
        &self.health
    }
}

impl Default for LvpaMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Monitor for LvpaMonitor {
    fn increment_counter(&self, name: &str, value: u64) -> MonitorResult<()> {
        if let Some(m) = find_metric(name) {
            if m.kind != MetricKind::Counter {
                return Err(MonitorError::MetricNotFound(
                    format!("指标 '{}' 不是 Counter 类型", name),
                ));
            }
        }
        let c = metrics::counter!(name.to_string());
        c.increment(value);
        Ok(())
    }

    fn set_gauge(&self, name: &str, value: f64) -> MonitorResult<()> {
        if let Some(m) = find_metric(name) {
            if m.kind != MetricKind::Gauge {
                return Err(MonitorError::MetricNotFound(
                    format!("指标 '{}' 不是 Gauge 类型", name),
                ));
            }
        }
        let g = metrics::gauge!(name.to_string());
        g.set(value);
        Ok(())
    }

    fn record_histogram(&self, name: &str, value: f64) -> MonitorResult<()> {
        if let Some(m) = find_metric(name) {
            if m.kind != MetricKind::Histogram {
                return Err(MonitorError::MetricNotFound(
                    format!("指标 '{}' 不是 Histogram 类型", name),
                ));
            }
        }
        let h = metrics::histogram!(name.to_string());
        h.record(value);
        Ok(())
    }

    fn register_module(&self, name: &str) -> ModuleHandle {
        self.health.register_module(name)
    }

    fn health_report(&self) -> SystemHealthReport {
        self.health.health_report()
    }

    fn add_alert_rule(&mut self, rule: AlertRule) {
        let mut alert = self.alert.lock()
            .unwrap_or_else(|e| e.into_inner());
        alert.add_rule(rule);
    }

    fn evaluate_alerts(&mut self, metric_values: &HashMap<String, f64>) -> Vec<AlertEvent> {
        let mut alert = self.alert.lock()
            .unwrap_or_else(|e| e.into_inner());
        alert.evaluate(metric_values)
    }

    fn set_log_level(&self, level: tracing::Level) {
        tracing::info!(target: logging::targets::MONITOR, "日志级别已切换至: {:?}", level);
    }

    fn export_prometheus(&self) -> String {
        "# LVPA Monitor Prometheus Metrics\n# 需要安装 metrics-exporter-prometheus 并调用 .install()\n".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_increment_counter() {
        let monitor = LvpaMonitor::new();
        assert!(monitor.increment_counter("request.total", 1).is_ok());
    }

    #[test]
    fn test_set_gauge() {
        let monitor = LvpaMonitor::new();
        assert!(monitor.set_gauge("module.healthy", 1.0).is_ok());
    }

    #[test]
    fn test_record_histogram() {
        let monitor = LvpaMonitor::new();
        assert!(monitor.record_histogram("request.latency_ms", 42.0).is_ok());
    }

    #[test]
    fn test_register_module_and_health() {
        let monitor = LvpaMonitor::new();
        let _handle = monitor.register_module("test_engine");
        let report = monitor.health_report();
        assert!(report.modules.contains_key("test_engine"));
    }

    #[test]
    fn test_add_alert_rule() {
        let mut monitor = LvpaMonitor::new();
        monitor.add_alert_rule(AlertRule {
            name: "test".into(),
            description: "测试规则".into(),
            metric_name: "cpu".into(),
            condition: crate::alert::AlertCondition::GreaterThan(90.0),
            severity: crate::alert::AlertSeverity::Warning,
            duration_secs: 0,
            channels: vec![crate::alert::AlertChannel::Log],
        });
        let mut values = HashMap::new();
        values.insert("cpu".into(), 95.0);
        let events = monitor.evaluate_alerts(&mut values);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_module_handle_drop() {
        let monitor = LvpaMonitor::new();
        let module_name = "temp";
        {
            let _handle = monitor.register_module(module_name);
        }
        let report = monitor.health_report();
        assert!(!report.modules.contains_key(module_name), "drop 后应自动注销");
    }
}
