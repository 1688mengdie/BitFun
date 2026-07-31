//! Executor — orchestrates signal queue → risk check → batch → order lifecycle.
//!
//! The executor is the top-level coordinator for the execution layer:
//!
//! 1. Signals arrive via `push_signal()` and enter the priority queue.
//! 2. On each processing cycle, signals are dequeued in priority+FIFO order.
//! 3. Each signal passes through the optional RiskChecker.
//! 4. Allowed signals are assembled into batch orders (same-instrument merge).
//! 5. Batch orders are submitted to the OrderManager.
//!
//! Design reference: R-2-504 执行器 — 信号→订单转换骨架
//! 参考: 量价时空/Phase-2-派发提示词.md:843 — R-2-504 — taiji-executor 执行器

use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use crate::batch::{BatchBuilder, BatchOrder};
use crate::order_mgr::OrderManager;
use crate::queue::SignalQueue;
use crate::risk::{RiskCheckRequest, RiskChecker, RiskDecision};
use crate::signal::{ExecAction, ExecSignal};
use crate::types::{Direction, Offset, OrderAck, OrderRequest, OrderType};

/// Convert an ExecAction to Direction + Offset.
fn action_to_direction_offset(action: ExecAction) -> (Direction, Offset) {
    match action {
        ExecAction::OpenLong => (Direction::Buy, Offset::Open),
        ExecAction::OpenShort => (Direction::Sell, Offset::Open),
        ExecAction::CloseLong => (Direction::Sell, Offset::Close),
        ExecAction::CloseShort => (Direction::Buy, Offset::Close),
    }
}

/// Convert an ExecAction to a human-readable string.
fn action_to_string(action: ExecAction) -> &'static str {
    match action {
        ExecAction::OpenLong => "OpenLong",
        ExecAction::OpenShort => "OpenShort",
        ExecAction::CloseLong => "CloseLong",
        ExecAction::CloseShort => "CloseShort",
    }
}

/// The execution orchestrator.
///
/// Owns the signal queue, optional risk checker, batch builder, and order
/// manager. Callers push signals and then drive processing.
pub struct Executor {
    signal_queue: SignalQueue,
    risk_checker: Option<Box<dyn RiskChecker>>,
    order_manager: OrderManager,
    next_order_id: AtomicU64,
}

impl Executor {
    /// Create a new executor with the given signal queue capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            signal_queue: SignalQueue::new(capacity),
            risk_checker: None,
            order_manager: OrderManager::new(),
            next_order_id: AtomicU64::new(1),
        }
    }

    /// Set the risk checker. None = no risk checks (development mode).
    pub fn set_risk_checker(&mut self, checker: Box<dyn RiskChecker>) {
        self.risk_checker = Some(checker);
    }

    /// Remove the risk checker (disables pre-trade checks).
    pub fn clear_risk_checker(&mut self) {
        self.risk_checker = None;
    }

    /// Push a signal into the execution queue.
    pub fn push_signal(&mut self, signal: ExecSignal) -> Result<(), ExecSignal> {
        self.signal_queue.enqueue(signal)
    }

    /// Return a reference to the order manager.
    pub fn order_manager(&self) -> &OrderManager {
        &self.order_manager
    }

    /// Return the number of signals waiting in the queue.
    pub fn queue_len(&self) -> usize {
        self.signal_queue.len()
    }

    /// Return true if the signal queue is empty.
    pub fn is_queue_empty(&self) -> bool {
        self.signal_queue.is_empty()
    }

    /// Process a single signal: dequeue → risk check → submit to OrderManager.
    ///
    /// Returns the OrderAck if a signal was processed, or None if the queue
    /// is empty or the signal was rejected by risk check.
    pub fn process_one(&mut self) -> Option<OrderAck> {
        let signal = self.signal_queue.dequeue()?;
        self.process_signal(signal)
    }

    /// Process all queued signals and return batch orders and acknowledgements.
    ///
    /// Returns a tuple of (batch_orders, order_acks).
    pub fn process_all(&mut self) -> (Vec<BatchOrder>, Vec<OrderAck>) {
        let signals = self.signal_queue.drain();
        if signals.is_empty() {
            return (vec![], vec![]);
        }

        // Phase 1: risk check each signal
        let allowed = self.filter_by_risk(signals);

        // Phase 2: batch assembly
        let batches = BatchBuilder::build(allowed);

        // Phase 3: submit batch orders to OrderManager
        let acks = self.submit_batches(&batches);

        (batches, acks)
    }

    // ── Internal helpers ──

    /// Process a single signal through risk check → OrderManager.
    fn process_signal(&mut self, signal: ExecSignal) -> Option<OrderAck> {
        // Risk check (if configured)
        if let Some(ref checker) = self.risk_checker {
            let request = RiskCheckRequest::new(
                &signal.instrument,
                action_to_string(signal.action),
                signal.price.unwrap_or(0.0),
                signal.volume,
            );
            match checker.check_order(&request) {
                Ok(RiskDecision::Allow) => {}
                Ok(RiskDecision::Reduce(max_vol)) => {
                    let adjusted = ExecSignal {
                        volume: max_vol.min(signal.volume),
                        ..signal
                    };
                    return Some(self.submit_single(adjusted));
                }
                Ok(RiskDecision::Reject(reason)) => {
                    tracing::warn!("Risk rejected signal {}: {}", signal.id, reason);
                    return None;
                }
                Err(e) => {
                    tracing::error!("Risk check error for {}: {}", signal.id, e);
                    return None;
                }
            }
        }

        Some(self.submit_single(signal))
    }

    /// Submit a single signal as an order (used by process_one).
    fn submit_single(&mut self, signal: ExecSignal) -> OrderAck {
        let (direction, offset) = action_to_direction_offset(signal.action);
        let order_id = self.next_order_id();
        let order = OrderRequest {
            order_id: order_id.clone(),
            instrument: signal.instrument,
            direction,
            offset,
            price: signal.price.unwrap_or(0.0),
            volume: signal.volume as u32,
            order_type: OrderType::Market,
        };
        self.order_manager.submit(order)
    }

    /// Filter signals through risk checker.
    fn filter_by_risk(&self, signals: Vec<ExecSignal>) -> Vec<ExecSignal> {
        let checker = match self.risk_checker {
            Some(ref c) => c,
            None => return signals, // no risk check = pass all
        };

        signals
            .into_iter()
            .filter_map(|signal| {
                let request = RiskCheckRequest::new(
                    &signal.instrument,
                    action_to_string(signal.action),
                    signal.price.unwrap_or(0.0),
                    signal.volume,
                );
                match checker.check_order(&request) {
                    Ok(RiskDecision::Allow) => Some(signal),
                    Ok(RiskDecision::Reduce(max_vol)) => {
                        let mut s = signal;
                        s.volume = max_vol.min(s.volume);
                        Some(s)
                    }
                    Ok(RiskDecision::Reject(reason)) => {
                        tracing::warn!("Risk rejected {}: {}", signal.id, reason);
                        None
                    }
                    Err(e) => {
                        tracing::error!("Risk error on {}: {}", signal.id, e);
                        None
                    }
                }
            })
            .collect()
    }

    /// Submit batch orders to the OrderManager.
    fn submit_batches(&self, batches: &[BatchOrder]) -> Vec<OrderAck> {
        batches
            .iter()
            .map(|batch| {
                let (direction, offset) = action_to_direction_offset(batch.action);
                let order_id = self.next_order_id();
                let order = OrderRequest {
                    order_id,
                    instrument: batch.instrument.clone(),
                    direction,
                    offset,
                    price: batch.avg_price.unwrap_or(0.0),
                    volume: batch.total_volume as u32,
                    order_type: OrderType::Market,
                };
                self.order_manager.submit(order)
            })
            .collect()
    }

    /// Generate a monotonic order ID.
    fn next_order_id(&self) -> String {
        let seq = self.next_order_id.fetch_add(1, AtomicOrdering::Relaxed);
        format!("exec-{:06}", seq)
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk::PassThroughRiskChecker;
    use crate::signal::SignalPriority;
    use crate::OrderStatus;

    fn make_signal(instrument: &str, action: ExecAction, volume: f64) -> ExecSignal {
        ExecSignal::new(instrument, action, Some(100.0), volume, "test", 0.8)
    }

    #[test]
    fn test_push_and_process_one() {
        let mut exec = Executor::new(128);
        exec.set_risk_checker(Box::new(PassThroughRiskChecker::new()));

        exec.push_signal(make_signal("ag2506", ExecAction::OpenLong, 2.0))
            .unwrap();
        assert_eq!(exec.queue_len(), 1);

        let ack = exec.process_one().unwrap();
        assert_eq!(ack.status, OrderStatus::Sent);
        assert_eq!(ack.filled_volume, 0);
        assert!(exec.is_queue_empty());
    }

    #[test]
    fn test_process_all_with_batching() {
        let mut exec = Executor::new(128);
        exec.set_risk_checker(Box::new(PassThroughRiskChecker::new()));

        exec.push_signal(make_signal("ag2506", ExecAction::OpenLong, 2.0))
            .unwrap();
        exec.push_signal(make_signal("ag2506", ExecAction::OpenLong, 3.0))
            .unwrap();
        exec.push_signal(make_signal("rb2501", ExecAction::OpenShort, 5.0))
            .unwrap();

        let (batches, acks) = exec.process_all();
        assert_eq!(batches.len(), 2); // 2 instruments
        assert_eq!(acks.len(), 2);

        // ag2506 batch merged
        let ag_batch = batches.iter().find(|b| b.instrument == "ag2506").unwrap();
        assert_eq!(ag_batch.total_volume, 5.0);
        assert_eq!(ag_batch.signal_count, 2);
    }

    #[test]
    fn test_risk_reject_blocks_signal() {
        // Use a custom checker that rejects all OpenLong on ag2506
        struct RejectAgChecker;
        impl RiskChecker for RejectAgChecker {
            fn check_order(&self, request: &RiskCheckRequest) -> Result<RiskDecision, String> {
                if request.instrument == "ag2506" {
                    Ok(RiskDecision::Reject("黑名单品种".into()))
                } else {
                    Ok(RiskDecision::Allow)
                }
            }
            fn name(&self) -> &str {
                "reject_ag"
            }
        }

        let mut exec = Executor::new(128);
        exec.set_risk_checker(Box::new(RejectAgChecker));

        exec.push_signal(make_signal("ag2506", ExecAction::OpenLong, 2.0))
            .unwrap();
        exec.push_signal(make_signal("rb2501", ExecAction::OpenLong, 3.0))
            .unwrap();

        let (batches, acks) = exec.process_all();
        // Only rb2501 should pass
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].instrument, "rb2501");
        assert_eq!(acks.len(), 1);
    }

    #[test]
    fn test_risk_reduce_adjusts_volume() {
        struct ReduceChecker;
        impl RiskChecker for ReduceChecker {
            fn check_order(&self, _request: &RiskCheckRequest) -> Result<RiskDecision, String> {
                Ok(RiskDecision::Reduce(1.0))
            }
            fn name(&self) -> &str {
                "reduce"
            }
        }

        let mut exec = Executor::new(128);
        exec.set_risk_checker(Box::new(ReduceChecker));

        exec.push_signal(make_signal("ag2506", ExecAction::OpenLong, 5.0))
            .unwrap();
        let (batches, acks) = exec.process_all();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].total_volume, 1.0); // reduced from 5 to 1
        assert_eq!(acks.len(), 1);
    }

    #[test]
    fn test_no_risk_checker_passes_all() {
        let mut exec = Executor::new(128);
        // No risk checker set
        exec.push_signal(make_signal("ag2506", ExecAction::OpenLong, 2.0))
            .unwrap();
        let ack = exec.process_one().unwrap();
        assert_eq!(ack.status, OrderStatus::Sent);
    }

    #[test]
    fn test_priority_processing() {
        let mut exec = Executor::new(128);
        exec.set_risk_checker(Box::new(PassThroughRiskChecker::new()));

        let normal = make_signal("a", ExecAction::OpenLong, 1.0);
        let high = make_signal("b", ExecAction::CloseLong, 1.0).with_priority(SignalPriority::High);

        exec.push_signal(normal).unwrap();
        exec.push_signal(high).unwrap();

        // High priority processed first
        let ack1 = exec.process_one().unwrap();
        // We can't easily check which order_id corresponds to which signal,
        // but we verify both are processed
        let ack2 = exec.process_one().unwrap();
        assert!(exec.is_queue_empty());
        assert_eq!(ack1.status, OrderStatus::Sent);
        assert_eq!(ack2.status, OrderStatus::Sent);
    }

    #[test]
    fn test_queue_overflow() {
        let mut exec = Executor::new(2);
        assert!(exec.push_signal(make_signal("a", ExecAction::OpenLong, 1.0)).is_ok());
        assert!(exec.push_signal(make_signal("b", ExecAction::OpenLong, 1.0)).is_ok());
        assert!(exec.push_signal(make_signal("c", ExecAction::OpenLong, 1.0)).is_err());
    }

    #[test]
    fn test_empty_queue_returns_none() {
        let mut exec = Executor::new(128);
        assert!(exec.process_one().is_none());
        let (batches, acks) = exec.process_all();
        assert!(batches.is_empty());
        assert!(acks.is_empty());
    }

    #[test]
    fn test_clear_risk_checker() {
        struct RejectAll;
        impl RiskChecker for RejectAll {
            fn check_order(&self, _: &RiskCheckRequest) -> Result<RiskDecision, String> {
                Ok(RiskDecision::Reject("拒绝所有".into()))
            }
            fn name(&self) -> &str {
                "reject_all"
            }
        }

        let mut exec = Executor::new(128);
        exec.set_risk_checker(Box::new(RejectAll));
        exec.push_signal(make_signal("ag2506", ExecAction::OpenLong, 1.0))
            .unwrap();
        assert!(exec.process_one().is_none()); // rejected

        exec.clear_risk_checker();
        exec.push_signal(make_signal("ag2506", ExecAction::OpenLong, 1.0))
            .unwrap();
        assert!(exec.process_one().is_some()); // allowed
    }
}
