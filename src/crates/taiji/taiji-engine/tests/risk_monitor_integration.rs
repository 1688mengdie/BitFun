//! R-2-604 RiskMonitor ↔ Pipeline 集成测试。
//!
//! 验证完整的"注册 RiskMonitor → 风险订单被拒绝 → Pipeline 隔离"链路：
//!
//! 使用 pipeline::filter::filter_signals 作为风控过滤外部 API。
//!
//! 1. 创建 RiskMonitor 并注册
//! 2. 通过 filter_signals 过滤信号
//! 3. 确认高风险信号被拒绝，低风险信号通过
//! 4. 测试无 RiskMonitor 时信号全部通过（隔离验证）

use taiji_engine::error::Result;
use taiji_engine::pipeline::filter;
use taiji_engine::risk::{
    RiskAction, RiskAlert, RiskConfig, RiskFill, RiskMonitor, OrderDecision, RiskOrderRequest, RiskPosition,
};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::signal::{Signal, SignalAction};
use chrono::Utc;

// ============================================================================
// Mock RiskMonitors
// ============================================================================

/// Mock：拒绝所有开仓信号（仅允许平仓）。
struct RejectOpenMonitor;
impl RiskMonitor for RejectOpenMonitor {
    fn init(&mut self, _config: &RiskConfig) -> Result<()> { Ok(()) }
    fn check_order(&self, order: &RiskOrderRequest, _state: &StateStore) -> Result<OrderDecision> {
        match order.action.as_str() {
            "CloseLong" | "CloseShort" => Ok(OrderDecision::Allow),
            _ => Ok(OrderDecision::Reject(format!("禁止开仓: {}", order.action))),
        }
    }
    fn check_position(&self, _pos: &RiskPosition, _state: &StateStore) -> Result<RiskAction> { Ok(RiskAction::None) }
    fn on_fill(&mut self, _fill: &RiskFill, _state: &StateStore) {}
    fn on_calculate(&mut self, _state: &StateStore) -> Result<Vec<RiskAlert>> { Ok(vec![]) }
    fn enabled(&self) -> bool { true }
}

/// Mock：缩量监视器——单笔成交量超过 max_vol 时缩量。
struct VolumeCapMonitor { max_vol: f64 }
impl VolumeCapMonitor { fn new(max: f64) -> Self { Self { max_vol: max } } }
impl RiskMonitor for VolumeCapMonitor {
    fn init(&mut self, _config: &RiskConfig) -> Result<()> { Ok(()) }
    fn check_order(&self, order: &RiskOrderRequest, _state: &StateStore) -> Result<OrderDecision> {
        if order.volume > self.max_vol {
            Ok(OrderDecision::Reduce(self.max_vol))
        } else {
            Ok(OrderDecision::Allow)
        }
    }
    fn check_position(&self, _pos: &RiskPosition, _state: &StateStore) -> Result<RiskAction> { Ok(RiskAction::None) }
    fn on_fill(&mut self, _fill: &RiskFill, _state: &StateStore) {}
    fn on_calculate(&mut self, _state: &StateStore) -> Result<Vec<RiskAlert>> { Ok(vec![]) }
    fn enabled(&self) -> bool { true }
}

// ============================================================================
// 辅助函数
// ============================================================================

fn make_bar() -> RawBar {
    RawBar {
        symbol: "test".into(), dt: Utc::now(), freq: Freq::F1, id: 0,
        open: 100.0, high: 101.0, low: 99.0, close: 100.5,
        vol: 1000.0, amount: 100_000.0, open_interest: None, delta: None,
    }
}

fn make_signal(instrument: &str, action: SignalAction, size: f64) -> Signal {
    Signal {
        timestamp: Utc::now(),
        instrument: instrument.into(),
        freq: Freq::F1,
        action,
        entry: Some(100.0),
        stop_loss: None,
        take_profit: None,
        size: Some(size),
        source: "test_node".into(),
        confidence: 0.8,
        metadata: Default::default(),
        disclaimer: None,
    }
}

fn new_state() -> StateStore {
    StateStore::default()
}

// ============================================================================
// 集成测试
// ============================================================================

/// 测试 1：RiskMonitor 拒绝所有信号 → 输出为空。
#[test]
fn test_risk_monitor_rejects_all_signals() {
    let bar = make_bar();
    let state = new_state();
    let monitor = RejectOpenMonitor;
    let signals = vec![
        make_signal("rb2501", SignalAction::Long, 1.0),
        make_signal("rb2501", SignalAction::Short, 1.0),
    ];
    let filtered = filter::filter_signals(signals, Some(&monitor), &bar, &state);
    assert!(filtered.is_empty(), "应拒绝所有开仓信号");
}

/// 测试 2：开仓被拒，平仓通过。
#[test]
fn test_risk_monitor_allows_close_only() {
    let bar = make_bar();
    let state = new_state();
    let monitor = RejectOpenMonitor;
    let signals = vec![
        make_signal("rb2501", SignalAction::Long, 1.0),
        make_signal("rb2501", SignalAction::CloseLong, 1.0),
        make_signal("rb2502", SignalAction::Short, 2.0),
        make_signal("rb2502", SignalAction::CloseShort, 2.0),
    ];
    let filtered = filter::filter_signals(signals, Some(&monitor), &bar, &state);
    assert_eq!(filtered.len(), 2, "只有平仓应通过");
    for s in &filtered {
        match s.action {
            SignalAction::CloseLong | SignalAction::CloseShort => {},
            _ => panic!("不应有开仓信号通过: {:?}", s.action),
        }
    }
}

/// 测试 3：VolumeCapMonitor 缩量。
#[test]
fn test_risk_monitor_reduces_volume() {
    let bar = make_bar();
    let state = new_state();
    let monitor = VolumeCapMonitor::new(5.0);
    let signals = vec![
        make_signal("rb2501", SignalAction::Long, 10.0),  // >5 → 缩量
        make_signal("rb2502", SignalAction::Short, 3.0),   // ≤5 → 通过
    ];
    let filtered = filter::filter_signals(signals, Some(&monitor), &bar, &state);
    assert_eq!(filtered.len(), 2);
    assert!((filtered[0].size.unwrap() - 5.0).abs() < 1e-6, "大单应缩量至 5.0");
    assert!((filtered[1].size.unwrap() - 3.0).abs() < 1e-6, "小单保持 3.0");
}

/// 测试 4：无 RiskMonitor → 全量通过（Pipeline 隔离）。
#[test]
fn test_no_risk_monitor_passthrough() {
    let bar = make_bar();
    let state = new_state();
    let signals = vec![
        make_signal("rb2501", SignalAction::Long, 10.0),
        make_signal("rb2502", SignalAction::Short, 20.0),
    ];
    let n = signals.len();
    let filtered = filter::filter_signals(signals, None, &bar, &state);
    assert_eq!(filtered.len(), n, "无风控时应全量通过");
}

/// 测试 5：混合决策——Allow + Reduce + Allow。
#[test]
fn test_risk_monitor_mixed_decisions() {
    let bar = make_bar();
    let state = new_state();
    let monitor = VolumeCapMonitor::new(4.0);
    let signals = vec![
        make_signal("rb2501", SignalAction::Long, 2.0),   // Allow
        make_signal("rb2502", SignalAction::Short, 10.0),  // Reduce → 4.0
        make_signal("rb2503", SignalAction::Long, 1.0),    // Allow
    ];
    let filtered = filter::filter_signals(signals, Some(&monitor), &bar, &state);
    assert_eq!(filtered.len(), 3);
    assert!((filtered[0].size.unwrap() - 2.0).abs() < 1e-6);
    assert!((filtered[1].size.unwrap() - 4.0).abs() < 1e-6, "应缩量至 4.0");
    assert!((filtered[2].size.unwrap() - 1.0).abs() < 1e-6);
}

/// 测试 6：signal metadata 正确传递。
#[test]
fn test_signal_metadata_through_filter() {
    let bar = make_bar();
    let state = new_state();
    let monitor = VolumeCapMonitor::new(100.0);
    let signals = vec![make_signal("if2501", SignalAction::CloseLong, 2.0)];
    let filtered = filter::filter_signals(signals, Some(&monitor), &bar, &state);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].instrument, "if2501");
    assert!(matches!(filtered[0].action, SignalAction::CloseLong));
    assert!((filtered[0].size.unwrap() - 2.0).abs() < 1e-6);
}
