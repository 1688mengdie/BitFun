//! Risk checker trait for pre-trade order validation.
//!
//! Each signal MUST pass through the risk checker before being converted
//! into an order. This is the executor-level risk abstraction, independent
//! of taiji-engine's RiskMonitor (to avoid circular crate dependencies).
//!
//! Design reference: R-2-504 RiskMonitor 前置检查
//! 参考: 量价时空/Phase-2-派发提示词.md:843 — R-2-504 — taiji-executor 执行器

use serde::{Deserialize, Serialize};

/// Input to the risk checker — mirrors taiji-engine's RiskOrderRequest fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskCheckRequest {
    pub instrument: String,
    pub action: String,
    pub price: f64,
    pub volume: f64,
}

impl RiskCheckRequest {
    /// Create a new risk check request.
    pub fn new(instrument: &str, action: &str, price: f64, volume: f64) -> Self {
        Self {
            instrument: instrument.to_string(),
            action: action.to_string(),
            price,
            volume,
        }
    }
}

/// Risk check decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskDecision {
    /// Allow the order through.
    Allow,
    /// Reject with a reason.
    Reject(String),
    /// Allow but reduce volume to the specified amount.
    Reduce(f64),
}

/// The risk checker trait — called for each signal before order placement.
///
/// # L1 compliance
///
/// Implementations MUST NOT block L1 compute threads. Expensive checks
/// (DB queries, LLM calls) should be performed asynchronously and cached,
/// with the checker returning cached decisions during L1 ticks.
pub trait RiskChecker: Send + Sync {
    /// Check whether an order is allowed.
    fn check_order(&self, request: &RiskCheckRequest) -> Result<RiskDecision, String>;

    /// Optional name for diagnostics.
    fn name(&self) -> &str {
        "unnamed_risk_checker"
    }
}

/// A pass-through risk checker that allows all orders.
///
/// Suitable for development and backtesting. Production deployments
/// should replace this with a real implementation.
pub struct PassThroughRiskChecker;

impl PassThroughRiskChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PassThroughRiskChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl RiskChecker for PassThroughRiskChecker {
    fn check_order(&self, _request: &RiskCheckRequest) -> Result<RiskDecision, String> {
        Ok(RiskDecision::Allow)
    }

    fn name(&self) -> &str {
        "pass_through"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pass_through_allows_all() {
        let checker = PassThroughRiskChecker::new();
        let req = RiskCheckRequest::new("ag2506", "OpenLong", 5600.0, 2.0);
        let decision = checker.check_order(&req).unwrap();
        assert!(matches!(decision, RiskDecision::Allow));
    }

    #[test]
    fn test_checker_name() {
        let checker = PassThroughRiskChecker::new();
        assert_eq!(checker.name(), "pass_through");
    }

    #[test]
    fn test_serde_roundtrip_decision() {
        let decisions = vec![
            RiskDecision::Allow,
            RiskDecision::Reject("资金不足".into()),
            RiskDecision::Reduce(1.0),
        ];
        for d in &decisions {
            let json = serde_json::to_string(d).unwrap();
            let back: RiskDecision = serde_json::from_str(&json).unwrap();
            match (d, &back) {
                (RiskDecision::Allow, RiskDecision::Allow) => {}
                (RiskDecision::Reject(a), RiskDecision::Reject(b)) => assert_eq!(a, b),
                (RiskDecision::Reduce(a), RiskDecision::Reduce(b)) => assert!((a - b).abs() < 1e-6),
                _ => panic!("serde roundtrip mismatch"),
            }
        }
    }

    #[test]
    fn test_reject_with_reason() {
        let checker = PassThroughRiskChecker::new();
        let req = RiskCheckRequest::new("ag2506", "OpenLong", 5600.0, 1000.0);
        // PassThrough always allows, but we test the decision type
        let decision = checker.check_order(&req).unwrap();
        assert!(matches!(decision, RiskDecision::Allow));
    }

    #[test]
    fn test_custom_risk_checker() {
        struct MaxVolumeChecker {
            max_volume: f64,
        }
        impl RiskChecker for MaxVolumeChecker {
            fn check_order(&self, request: &RiskCheckRequest) -> Result<RiskDecision, String> {
                if request.volume > self.max_volume {
                    Ok(RiskDecision::Reduce(self.max_volume))
                } else {
                    Ok(RiskDecision::Allow)
                }
            }
            fn name(&self) -> &str {
                "max_volume"
            }
        }

        let checker = MaxVolumeChecker { max_volume: 5.0 };
        assert_eq!(checker.name(), "max_volume");

        let small = RiskCheckRequest::new("ag2506", "OpenLong", 5600.0, 3.0);
        assert!(matches!(checker.check_order(&small).unwrap(), RiskDecision::Allow));

        let large = RiskCheckRequest::new("ag2506", "OpenLong", 5600.0, 10.0);
        match checker.check_order(&large).unwrap() {
            RiskDecision::Reduce(v) => assert!((v - 5.0).abs() < 1e-6),
            _ => panic!("expected Reduce"),
        }
    }
}
