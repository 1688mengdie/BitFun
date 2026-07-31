//! Batch order assembly — merge N signals into a single batch order per instrument.
//!
//! Design reference: R-2-504 批量订单组装
//! 参考: 量价时空/Phase-2-派发提示词.md:843 — R-2-504 — taiji-executor 执行器

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::signal::{ExecAction, ExecSignal};

/// A batch order aggregating multiple signals for the same instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOrder {
    /// Unique batch ID.
    pub batch_id: String,
    /// Target instrument.
    pub instrument: String,
    /// Unified action after merging.
    pub action: ExecAction,
    /// Total volume across all merged signals.
    pub total_volume: f64,
    /// Volume-weighted average price.
    pub avg_price: Option<f64>,
    /// Source signals that were merged into this batch.
    pub signal_count: usize,
}

/// Builds batch orders by grouping signals by (instrument, action).
///
/// # Merging rules
///
/// - Signals are grouped by (instrument, action) pair.
/// - Total volume is the sum of individual volumes.
/// - Average price is volume-weighted across all merged signals.
/// - Each group produces one `BatchOrder`.
pub struct BatchBuilder;

impl BatchBuilder {
    /// Build batch orders from a list of signals.
    ///
    /// Signals with `Hold`-equivalent actions (no open/close) are skipped.
    pub fn build(signals: Vec<ExecSignal>) -> Vec<BatchOrder> {
        // Group by (instrument, action)
        let mut groups: HashMap<(String, ExecAction), Vec<ExecSignal>> = HashMap::new();

        for signal in signals {
            groups
                .entry((signal.instrument.clone(), signal.action))
                .or_default()
                .push(signal);
        }

        groups
            .into_iter()
            .map(|((instrument, action), sigs)| {
                let total_volume: f64 = sigs.iter().map(|s| s.volume).sum();
                let total_weighted_price: f64 = sigs
                    .iter()
                    .filter_map(|s| s.price.map(|p| p * s.volume))
                    .sum();
                let total_price_volume: f64 = sigs.iter().filter(|s| s.price.is_some()).map(|s| s.volume).sum();
                let avg_price = if total_price_volume > 0.0 {
                    Some(total_weighted_price / total_price_volume)
                } else {
                    None
                };

                BatchOrder {
                    batch_id: uuid::Uuid::new_v4().to_string(),
                    instrument,
                    action,
                    total_volume,
                    avg_price,
                    signal_count: sigs.len(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::ExecSignal;

    fn sig(instrument: &str, action: ExecAction, volume: f64, price: Option<f64>) -> ExecSignal {
        ExecSignal::new(instrument, action, price, volume, "test", 0.8)
    }

    #[test]
    fn test_same_instrument_merge() {
        let signals = vec![
            sig("ag2506", ExecAction::OpenLong, 2.0, Some(5600.0)),
            sig("ag2506", ExecAction::OpenLong, 3.0, Some(5625.0)),
        ];
        let batches = BatchBuilder::build(signals);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].instrument, "ag2506");
        assert_eq!(batches[0].total_volume, 5.0);
        assert_eq!(batches[0].signal_count, 2);
        // Weighted avg: (2*5600 + 3*5625) / 5 = (11200 + 16875) / 5 = 28075/5 = 5615
        assert!((batches[0].avg_price.unwrap() - 5615.0).abs() < 0.01);
    }

    #[test]
    fn test_cross_instrument_separation() {
        let signals = vec![
            sig("ag2506", ExecAction::OpenLong, 2.0, Some(5600.0)),
            sig("rb2501", ExecAction::OpenLong, 5.0, Some(3500.0)),
        ];
        let batches = BatchBuilder::build(signals);
        assert_eq!(batches.len(), 2);
    }

    #[test]
    fn test_different_actions_no_merge() {
        let signals = vec![
            sig("ag2506", ExecAction::OpenLong, 2.0, Some(5600.0)),
            sig("ag2506", ExecAction::CloseLong, 2.0, Some(5650.0)),
        ];
        let batches = BatchBuilder::build(signals);
        assert_eq!(batches.len(), 2);
    }

    #[test]
    fn test_single_signal() {
        let signals = vec![sig("ag2506", ExecAction::OpenLong, 1.0, Some(5600.0))];
        let batches = BatchBuilder::build(signals);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].total_volume, 1.0);
        assert_eq!(batches[0].signal_count, 1);
    }

    #[test]
    fn test_empty_signals() {
        let batches = BatchBuilder::build(vec![]);
        assert!(batches.is_empty());
    }

    #[test]
    fn test_price_no_price_mixed() {
        let signals = vec![
            sig("ag2506", ExecAction::OpenLong, 2.0, Some(5600.0)),
            sig("ag2506", ExecAction::OpenLong, 3.0, None), // market order
        ];
        let batches = BatchBuilder::build(signals);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].total_volume, 5.0);
        // Only the signal with a price contributes to avg_price
        assert!((batches[0].avg_price.unwrap() - 5600.0).abs() < 0.01);
    }

    #[test]
    fn test_serde_roundtrip_batch_order() {
        let batch = BatchOrder {
            batch_id: "b-001".into(),
            instrument: "ag2506".into(),
            action: ExecAction::OpenLong,
            total_volume: 5.0,
            avg_price: Some(5615.0),
            signal_count: 2,
        };
        let json = serde_json::to_string(&batch).unwrap();
        let back: BatchOrder = serde_json::from_str(&json).unwrap();
        assert_eq!(back.batch_id, "b-001");
        assert_eq!(back.total_volume, 5.0);
    }
}
