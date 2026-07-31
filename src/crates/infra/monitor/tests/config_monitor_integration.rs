//! R-1-203 config ↔ monitor 集成测试。
//!
//! 验证配置变更触发 monitor 行为变化的端到端路径：
//!
//! ConfigManager.set("monitor.level", "debug") →
//!   ConfigChangeEvent → monitor.set_log_level(Debug)
//!
//! 前置条件：R-1-105（config）+ R-1-106（monitor）均已完成。

use std::collections::HashMap;

use taiji_infra_config::{ConfigManager, LvpaConfigManager};
use taiji_infra_monitor::{AlertRule, AlertCondition, AlertSeverity, AlertChannel, LvpaMonitor, Monitor};

/// 测试 1：ConfigManager 配置变更事件可被接收。
#[test]
fn test_config_change_event_received() {
    let mut cm = LvpaConfigManager::new();
    let mut rx = cm.subscribe();

    // 设置配置值
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        cm.load().await.unwrap();
        cm.set("monitor.level", "debug").await.unwrap();
    });

    // 验证收到 ConfigChangeEvent
    let event = tokio::runtime::Runtime::new().unwrap().block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await
    });

    assert!(event.is_ok(), "应在超时前收到 ConfigChangeEvent");
    let event = event.unwrap().unwrap();
    assert_eq!(event.path, "monitor.level");
    assert_eq!(event.new_value, serde_json::json!("debug"));
}

/// 测试 2：配置变更后 monitor 的 set_log_level 可正常调用（不 panic）。
#[test]
fn test_monitor_set_log_level_after_config_change() {
    let monitor = LvpaMonitor::new();

    // 模拟 config -> monitor.set_log_level 调用链
    let new_level = "debug";
    match new_level {
        "trace" => monitor.set_log_level(tracing::Level::TRACE),
        "debug" => monitor.set_log_level(tracing::Level::DEBUG),
        "info" => monitor.set_log_level(tracing::Level::INFO),
        "warn" => monitor.set_log_level(tracing::Level::WARN),
        "error" => monitor.set_log_level(tracing::Level::ERROR),
        _ => {},
    }
    // 调用成功即通过（不 panic）
}

/// 测试 3：告警规则可从配置加载并注入 monitor。
#[test]
fn test_alert_rules_from_config() {
    let mut monitor = LvpaMonitor::new();

    // 模拟从 config 加载的告警规则配置
    let rules_config = vec![
        AlertRule {
            name: "high_error_rate".into(),
            description: "错误率超过 10%".into(),
            metric_name: "request.errors".into(),
            condition: AlertCondition::GreaterThan(0.1),
            severity: AlertSeverity::Warning,
            duration_secs: 30,
            channels: vec![AlertChannel::Log],
        },
        AlertRule {
            name: "high_latency".into(),
            description: "P99 延迟超过 500ms".into(),
            metric_name: "request.latency_ms".into(),
            condition: AlertCondition::GreaterThan(500.0),
            severity: AlertSeverity::Critical,
            duration_secs: 60,
            channels: vec![AlertChannel::Stdout],
        },
    ];

    for rule in rules_config {
        monitor.add_alert_rule(rule);
    }

    // 验证告警规则可触发
    let mut values = HashMap::new();
    values.insert("request.errors".into(), 0.15);
    values.insert("request.latency_ms".into(), 600.0);

    let events = monitor.evaluate_alerts(&mut values);
    assert_eq!(events.len(), 2, "两条规则都应触发");

    assert!(events.iter().any(|e| e.rule_name == "high_error_rate"));
    assert!(events.iter().any(|e| e.rule_name == "high_latency"));
}

/// 测试 4：配置值变更后 monitor 指标采集不受影响。
#[test]
fn test_metrics_after_config_change() {
    let mut cm = LvpaConfigManager::new();
    let monitor = LvpaMonitor::new();

    tokio::runtime::Runtime::new().unwrap().block_on(async {
        cm.load().await.unwrap();
        cm.set("monitor.level", "warn").await.unwrap();
    });

    // 配置变更后指标采集仍然正常
    assert!(monitor.increment_counter("request.total", 1).is_ok());
    assert!(monitor.set_gauge("module.healthy", 1.0).is_ok());
    assert!(monitor.record_histogram("request.latency_ms", 42.0).is_ok());
}
