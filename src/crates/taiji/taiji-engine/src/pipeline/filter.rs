//! Pipeline 风控过滤模块。
//!
//! 在信号引擎 DAG 执行完毕后，调用已注册的 RiskMonitor 逐一过滤信号，
//! 拒绝高风险信号或缩量执行。
//!
//! 设计参考：已有 `pipeline/mod.rs:411-457` filter_signals() 实现，
//! 提取为独立模块。
//! 参考: 量价时空/Phase-2-派发提示词.md:188 — R-2-201 — Pipeline 执行引擎

use crate::risk::{OrderDecision, RiskMonitor, RiskOrderRequest};
use crate::store::StateStore;
use crate::types::bar::RawBar;
use crate::types::signal::Signal;
use tracing::{error, warn};

/// 对信号列表执行风控过滤。
///
/// 遍历 signals 并对每个信号调用 `monitor.check_order()`：
/// - `Allow` → 保留
/// - `Reject(reason)` → 移除并 warn
/// - `Reduce(qty)` → 调整 volume 后保留
/// - `Err(e)` → 移除并 error
///
/// 如果 `monitor` 为 `None`（未设置风控），直接返回原信号列表。
pub fn filter_signals(
    signals: Vec<Signal>,
    monitor: Option<&dyn RiskMonitor>,
    bar: &RawBar,
    state: &StateStore,
) -> Vec<Signal> {
    let monitor = match monitor {
        Some(m) => m,
        None => return signals,
    };

    signals
        .into_iter()
        .filter_map(|signal| {
            let order = RiskOrderRequest {
                instrument: signal.instrument.clone(),
                action: format!("{:?}", signal.action),
                price: signal.entry.unwrap_or(bar.close),
                volume: signal.size.unwrap_or(0.0),
            };
            match monitor.check_order(&order, state) {
                Ok(OrderDecision::Allow) => Some(signal),
                Ok(OrderDecision::Reject(reason)) => {
                    warn!("RiskMonitor rejected signal from {}: {}", signal.source, reason);
                    None
                }
                Ok(OrderDecision::Reduce(max_qty)) => {
                    let mut adjusted = signal.clone();
                    adjusted.size = Some(max_qty);
                    warn!("RiskMonitor reduced signal from {} to volume {}", adjusted.source, max_qty);
                    Some(adjusted)
                }
                Err(e) => {
                    error!("RiskMonitor error on signal from {}: {}", signal.source, e);
                    None
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk::PassThroughRiskMonitor;
    use crate::store::StateStore;
    use crate::types::bar::{Freq, RawBar};
    use crate::types::signal::{Signal, SignalAction};
    use chrono::Utc;

    fn make_signal(instrument: &str, size: Option<f64>) -> Signal {
        Signal {
            timestamp: Utc::now(),
            instrument: instrument.to_string(),
            freq: Freq::F1,
            action: SignalAction::Long,
            entry: Some(100.0),
            stop_loss: None,
            take_profit: None,
            size,
            source: "test".to_string(),
            confidence: 0.8,
            metadata: Default::default(),
            disclaimer: None,
        }
    }

    fn make_bar() -> RawBar {
        RawBar {
            symbol: "test".into(),
            dt: Utc::now(),
            freq: Freq::F1,
            id: 1,
            open: 100.0,
            close: 101.0,
            high: 102.0,
            low: 99.0,
            vol: 1000.0,
            amount: 100_000.0,
            open_interest: None,
            delta: None,
        }
    }

    #[test]
    fn test_no_monitor_passes_all() {
        let bar = make_bar();
        let state = StateStore::default();
        let signals = vec![make_signal("rb2501", Some(1.0)), make_signal("rb2502", Some(2.0))];
        let result = filter_signals(signals.clone(), None, &bar, &state);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_pass_through_monitor_allows_all() {
        let monitor = PassThroughRiskMonitor::new();
        let bar = make_bar();
        let state = StateStore::default();
        let signals = vec![make_signal("rb2501", Some(1.0))];
        let result = filter_signals(signals, Some(&monitor), &bar, &state);
        assert_eq!(result.len(), 1);
    }
}
