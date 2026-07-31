//! 天眼阵 — 告警规则引擎。
//!
//! 定义告警规则、触发条件、通知渠道、告警事件。
//! LVPA 自研告警规则引擎，参考通用监控告警模式。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// 告警严重级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// 信息。
    Info,
    /// 警告。
    Warning,
    /// 严重。
    Critical,
}

impl AlertSeverity {
    /// 返回优先级数值（0=最高）。
    pub fn priority(&self) -> u8 {
        match self {
            AlertSeverity::Critical => 0,
            AlertSeverity::Warning => 1,
            AlertSeverity::Info => 2,
        }
    }
}

/// 告警触发条件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertCondition {
    /// 大于阈值。
    GreaterThan(f64),
    /// 小于阈值。
    LessThan(f64),
    /// 等于阈值。
    Equals(f64),
    /// 变化率（窗口秒数, 阈值百分比）。
    RateOfChange { window_secs: u64, threshold: f64 },
    /// 心跳超时（超时秒数）。
    Absence { timeout_secs: u64 },
}

/// 通知渠道。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertChannel {
    /// 仅记录日志。
    Log,
    /// 标准输出。
    Stdout,
    /// 通过 event-bus 发布配置（仅配置，不直接调用 event-bus）。
    EventBus(String),
}

/// 告警规则。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    /// 规则名称。
    pub name: String,
    /// 规则描述。
    pub description: String,
    /// 监控指标名称。
    pub metric_name: String,
    /// 触发条件。
    pub condition: AlertCondition,
    /// 严重级别。
    pub severity: AlertSeverity,
    /// 持续时间（秒，防抖动）。
    pub duration_secs: u64,
    /// 通知渠道。
    pub channels: Vec<AlertChannel>,
}

/// 告警事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    /// 触发规则名称。
    pub rule_name: String,
    /// 严重级别。
    pub severity: AlertSeverity,
    /// 指标名称。
    pub metric_name: String,
    /// 当前值。
    pub current_value: f64,
    /// 阈值。
    pub threshold: f64,
    /// 告警消息。
    pub message: String,
    /// 触发时间。
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 告警规则引擎。
#[derive(Debug)]
pub struct AlertEngine {
    /// 告警规则列表。
    rules: Vec<AlertRule>,
    /// 规则上次触发时间（用于防抖动）。
    last_fired: HashMap<String, Instant>,
}

impl AlertEngine {
    /// 创建告警引擎。
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            last_fired: HashMap::new(),
        }
    }

    /// 添加告警规则。
    pub fn add_rule(&mut self, rule: AlertRule) {
        self.rules.push(rule);
    }

    /// 获取告警规则列表。
    pub fn rules(&self) -> &[AlertRule] {
        &self.rules
    }

    /// 评估所有告警规则，返回触发的告警事件列表。
    pub fn evaluate(&mut self, metric_values: &HashMap<String, f64>) -> Vec<AlertEvent> {
        let now = Instant::now();
        let mut events = Vec::new();

        for rule in &self.rules {
            let current = match metric_values.get(&rule.metric_name) {
                Some(v) => *v,
                None => continue,
            };

            let threshold = match rule.condition {
                AlertCondition::GreaterThan(t) => t,
                AlertCondition::LessThan(t) => t,
                AlertCondition::Equals(t) => t,
                AlertCondition::RateOfChange { threshold, .. } => threshold,
                AlertCondition::Absence { .. } => continue, // 超时检测由健康检查引擎处理
            };

            let triggered = match rule.condition {
                AlertCondition::GreaterThan(t) => current > t,
                AlertCondition::LessThan(t) => current < t,
                AlertCondition::Equals(t) => (current - t).abs() < f64::EPSILON,
                AlertCondition::RateOfChange { window_secs: _, threshold: t } => current > t,
                AlertCondition::Absence { .. } => false,
            };

            if !triggered {
                continue;
            }

            // 防抖动：检查是否在冷却期内
            let cooldown = Duration::from_secs(rule.duration_secs);
            if let Some(last) = self.last_fired.get(&rule.name) {
                if now.duration_since(*last) < cooldown {
                    continue;
                }
            }

            self.last_fired.insert(rule.name.clone(), now);

            events.push(AlertEvent {
                rule_name: rule.name.clone(),
                severity: rule.severity,
                metric_name: rule.metric_name.clone(),
                current_value: current,
                threshold,
                message: format!(
                    "[{}] {}: {} = {}, 阈值 = {}",
                    match rule.severity {
                        AlertSeverity::Critical => "CRITICAL",
                        AlertSeverity::Warning => "WARNING",
                        AlertSeverity::Info => "INFO",
                    },
                    rule.description,
                    rule.metric_name,
                    current,
                    threshold,
                ),
                timestamp: chrono::Utc::now(),
            });
        }

        events
    }
}

impl Default for AlertEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_alert_greater_than() {
        let mut engine = AlertEngine::new();
        engine.add_rule(AlertRule {
            name: "high_error_rate".into(),
            description: "错误率过高".into(),
            metric_name: "error_rate".into(),
            condition: AlertCondition::GreaterThan(0.1),
            severity: AlertSeverity::Warning,
            duration_secs: 0,
            channels: vec![AlertChannel::Log],
        });

        let mut values = HashMap::new();
        values.insert("error_rate".into(), 0.15);
        let events = engine.evaluate(&values);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].rule_name, "high_error_rate");
    }

    #[test]
    fn test_alert_no_trigger_below_threshold() {
        let mut engine = AlertEngine::new();
        engine.add_rule(AlertRule {
            name: "low_value".into(),
            description: "值过低".into(),
            metric_name: "value".into(),
            condition: AlertCondition::GreaterThan(100.0),
            severity: AlertSeverity::Info,
            duration_secs: 0,
            channels: vec![AlertChannel::Log],
        });

        let mut values = HashMap::new();
        values.insert("value".into(), 50.0);
        let events = engine.evaluate(&values);
        assert!(events.is_empty());
    }

    #[test]
    fn test_alert_cooldown() {
        let mut engine = AlertEngine::new();
        engine.add_rule(AlertRule {
            name: "cooldown_test".into(),
            description: "防抖动测试".into(),
            metric_name: "cpu".into(),
            condition: AlertCondition::GreaterThan(90.0),
            severity: AlertSeverity::Critical,
            duration_secs: 60, // 60 秒冷却
            channels: vec![AlertChannel::Log],
        });

        let mut values = HashMap::new();
        values.insert("cpu".into(), 95.0);
        let events = engine.evaluate(&values);
        assert_eq!(events.len(), 1, "首次应触发");

        // 再次评估（冷却期内）
        let events = engine.evaluate(&values);
        assert_eq!(events.len(), 0, "冷却期内不应触发");
    }
}
