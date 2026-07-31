//! Signal domain types for the execution layer.
//!
//! These are the executor's own signal representation, independent of
//! taiji-engine types, to avoid circular crate dependencies.
//!
//! Design reference: R-2-504 信号队列管理 — 优先级排序 + FIFO 顺序消费
//! 参考: 量价时空/Phase-2-派发提示词.md:843 — R-2-504 — taiji-executor 执行器

use serde::{Deserialize, Serialize};

/// Execution-level action derived from a strategy signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecAction {
    OpenLong,
    OpenShort,
    CloseLong,
    CloseShort,
}

/// Priority level for signal queue ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalPriority {
    /// Urgent signals (e.g., stop-loss alerts, forced liquidation).
    High = 0,
    /// Normal strategy signals.
    Normal = 1,
    /// Informational / low-confidence signals.
    Low = 2,
}

impl SignalPriority {
    /// Return the numeric rank (0 = highest).
    pub fn rank(self) -> u8 {
        self as u8
    }
}

/// Executor's own signal representation.
///
/// This is the input type for [`super::queue::SignalQueue`] and is converted
/// to an `OrderRequest` after passing the risk checker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecSignal {
    /// Unique signal ID (UUID v4).
    pub id: String,
    /// Instrument code (e.g. "ag2506").
    pub instrument: String,
    /// Trading action.
    pub action: ExecAction,
    /// Suggested entry price (None for market orders).
    pub price: Option<f64>,
    /// Order volume in lots.
    pub volume: f64,
    /// Source node identifier (e.g. "magnet_v1").
    pub source: String,
    /// Signal confidence [0.0, 1.0].
    pub confidence: f64,
    /// Timestamp when the signal was generated.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Queue priority.
    pub priority: SignalPriority,
}

impl ExecSignal {
    /// Create a new execution signal with default (Normal) priority.
    pub fn new(
        instrument: &str,
        action: ExecAction,
        price: Option<f64>,
        volume: f64,
        source: &str,
        confidence: f64,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            instrument: instrument.to_string(),
            action,
            price,
            volume,
            source: source.to_string(),
            confidence,
            timestamp: chrono::Utc::now(),
            priority: SignalPriority::Normal,
        }
    }

    /// Set priority and return self for builder-style usage.
    pub fn with_priority(mut self, priority: SignalPriority) -> Self {
        self.priority = priority;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_signal_creation() {
        let sig = ExecSignal::new("ag2506", ExecAction::OpenLong, Some(5625.0), 2.0, "test_node", 0.85);
        assert_eq!(sig.instrument, "ag2506");
        assert_eq!(sig.action, ExecAction::OpenLong);
        assert_eq!(sig.price, Some(5625.0));
        assert_eq!(sig.volume, 2.0);
        assert_eq!(sig.source, "test_node");
        assert_eq!(sig.confidence, 0.85);
        assert_eq!(sig.priority, SignalPriority::Normal);
        assert!(!sig.id.is_empty());
    }

    #[test]
    fn test_exec_signal_with_priority() {
        let sig = ExecSignal::new("ag2506", ExecAction::CloseLong, None, 1.0, "risk_mon", 1.0)
            .with_priority(SignalPriority::High);
        assert_eq!(sig.priority, SignalPriority::High);
        assert_eq!(sig.action, ExecAction::CloseLong);
        assert!(sig.price.is_none());
    }

    #[test]
    fn test_signal_priority_rank() {
        assert_eq!(SignalPriority::High.rank(), 0);
        assert_eq!(SignalPriority::Normal.rank(), 1);
        assert_eq!(SignalPriority::Low.rank(), 2);
    }

    #[test]
    fn test_serde_roundtrip_exec_signal() {
        let sig = ExecSignal::new("rb2501", ExecAction::OpenShort, Some(3500.0), 3.0, "test", 0.75);
        let json = serde_json::to_string(&sig).unwrap();
        let back: ExecSignal = serde_json::from_str(&json).unwrap();
        assert_eq!(sig.id, back.id);
        assert_eq!(sig.instrument, back.instrument);
        assert_eq!(sig.action, back.action);
        assert_eq!(sig.volume, back.volume);
    }

    #[test]
    fn test_serde_roundtrip_exec_action() {
        let actions = vec![ExecAction::OpenLong, ExecAction::OpenShort, ExecAction::CloseLong, ExecAction::CloseShort];
        for a in &actions {
            let json = serde_json::to_string(a).unwrap();
            let back: ExecAction = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{:?}", a), format!("{:?}", back));
        }
    }
}
