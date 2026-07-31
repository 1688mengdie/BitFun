//! RiskMonitor trait — 插件化风控接口。
//!
//! 每个监视器关注一个风控维度，Pipeline 在信号输出后依次调用所有已注册监视器。
//!
//! 设计参考：WonderTrader RiskMonDefs.h 插件化风控接口模式，Rust trait 翻译实现。
//! 参考已有 `taiji-engine/src/risk.rs:9-65` (Phase 1 骨架)。
//! 参考: 量价时空/Phase-2-派发提示词.md:620 — R-2-205 — RiskMonitor 插件化风控

use crate::error::Result;
use crate::store::StateStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// 风控支持类型
// ============================================================================

/// 风控配置——初始化参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    /// 模块参数 KV 表。
    pub params: HashMap<String, serde_json::Value>,
}

impl RiskConfig {
    /// 创建空配置。
    pub fn new() -> Self {
        Self { params: HashMap::new() }
    }
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// 订单请求——check_order 的输入参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskOrderRequest {
    /// 合约代码。
    pub instrument: String,
    /// 交易动作（如 "Open"、"Close"）。
    pub action: String,
    /// 申报价格。
    pub price: f64,
    /// 申报手数。
    pub volume: f64,
}

/// 风控决策——check_order 的返回值。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderDecision {
    /// 允许执行。
    Allow,
    /// 拒绝并给出原因。
    Reject(String),
    /// 允许但缩量到指定手数。
    Reduce(f64),
}

/// 持仓快照——check_position 的输入参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskPosition {
    /// 合约代码。
    pub instrument: String,
    /// 持仓手数（正值多头，负值空头）。
    pub volume: f64,
    /// 持仓均价。
    pub avg_price: f64,
}

/// 风控动作——check_position 的返回值。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskAction {
    /// 无操作。
    None,
    /// 警告。
    Warn(String),
    /// 强制平仓。
    ForceClose,
}

/// 成交回报——on_fill 的输入参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFill {
    /// 合约代码。
    pub instrument: String,
    /// 成交价格。
    pub price: f64,
    /// 成交量。
    pub volume: f64,
    /// 成交时间。
    pub time: chrono::DateTime<chrono::Utc>,
}

/// 风控告警级别。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskSeverity {
    /// 提示信息。
    Info,
    /// 警告。
    Warning,
    /// 严重（需立即关注）。
    Critical,
}

/// 风控告警——on_calculate 的返回值。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAlert {
    /// 发出告警的监控器名称。
    pub monitor: String,
    /// 告警级别。
    pub severity: RiskSeverity,
    /// 告警信息。
    pub message: String,
    /// 告警时间。
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

// ============================================================================
// RiskMonitor trait
// ============================================================================

/// 风险监控器 trait。
///
/// 每个实现关注一个风控维度（如资金限额、日内回转、最大持仓等），
/// Pipeline 在信号引擎输出后依次调用已注册的 RiskMonitor 进行过滤。
///
/// # 生命周期
///
/// 1. `init` — 创建或加载配置时调用一次
/// 2. `check_order` — 每次信号引擎输出后，对每个信号调用
/// 3. `check_position` — 定时检查持仓风险
/// 4. `on_fill` — 成交时回调，用于更新内部状态
/// 5. `on_calculate` — 定时计算，返回告警列表
/// 6. `enabled` — 是否启用
pub trait RiskMonitor: Send + Sync {
    /// 初始化监视器。
    fn init(&mut self, config: &RiskConfig) -> Result<()>;

    /// 订单检查（开仓/平仓前调用）。
    ///
    /// 返回 `Allow` 表示允许执行，`Reject(reason)` 拒绝，`Reduce(qty)` 缩量。
    fn check_order(&self, order: &RiskOrderRequest, state: &StateStore) -> Result<OrderDecision>;

    /// 持仓检查。
    fn check_position(&self, position: &RiskPosition, state: &StateStore) -> Result<RiskAction>;

    /// 成交回调。
    fn on_fill(&mut self, fill: &RiskFill, state: &StateStore);

    /// 定时计算，返回告警列表。
    fn on_calculate(&mut self, state: &StateStore) -> Result<Vec<RiskAlert>>;

    /// 是否启用。返回 `false` 时 Pipeline 跳过此监视器。
    fn enabled(&self) -> bool {
        true
    }
}

// ============================================================================
// 示例风控监视器
// ============================================================================

/// 通行风控监视器——允许所有订单，不施加任何限制。
///
/// 适用于开发/回测环境。生产环境应替换为实际风控实现。
pub struct PassThroughRiskMonitor;

impl PassThroughRiskMonitor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PassThroughRiskMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl RiskMonitor for PassThroughRiskMonitor {
    fn init(&mut self, _config: &RiskConfig) -> Result<()> {
        Ok(())
    }

    fn check_order(&self, _order: &RiskOrderRequest, _state: &StateStore) -> Result<OrderDecision> {
        Ok(OrderDecision::Allow)
    }

    fn check_position(&self, _position: &RiskPosition, _state: &StateStore) -> Result<RiskAction> {
        Ok(RiskAction::None)
    }

    fn on_fill(&mut self, _fill: &RiskFill, _state: &StateStore) {}

    fn on_calculate(&mut self, _state: &StateStore) -> Result<Vec<RiskAlert>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> StateStore {
        StateStore::default()
    }

    #[test]
    fn test_pass_through_risk_monitor() {
        let mut monitor = PassThroughRiskMonitor::new();
        assert!(monitor.init(&RiskConfig::new()).is_ok());
        assert!(monitor.enabled());
    }

    #[test]
    fn test_check_order_allow() {
        let monitor = PassThroughRiskMonitor::new();
        let state = make_state();
        let order = RiskOrderRequest {
            instrument: "rb2501".into(),
            action: "Open".into(),
            price: 3500.0,
            volume: 1.0,
        };
        let decision = monitor.check_order(&order, &state).unwrap();
        assert!(matches!(decision, OrderDecision::Allow));
    }

    #[test]
    fn test_check_position_none() {
        let monitor = PassThroughRiskMonitor::new();
        let state = make_state();
        let pos = RiskPosition {
            instrument: "rb2501".into(),
            volume: 10.0,
            avg_price: 3500.0,
        };
        let action = monitor.check_position(&pos, &state).unwrap();
        assert!(matches!(action, RiskAction::None));
    }

    #[test]
    fn test_serde_roundtrip_decision() {
        let decisions = vec![
            OrderDecision::Allow,
            OrderDecision::Reject("超过风险限额".into()),
            OrderDecision::Reduce(5.0),
        ];
        for d in &decisions {
            let json = serde_json::to_string(d).unwrap();
            let back: OrderDecision = serde_json::from_str(&json).unwrap();
            match (d, &back) {
                (OrderDecision::Allow, OrderDecision::Allow) => {}
                (OrderDecision::Reject(a), OrderDecision::Reject(b)) => assert_eq!(a, b),
                (OrderDecision::Reduce(a), OrderDecision::Reduce(b)) => assert!((a - b).abs() < 1e-6),
                _ => panic!("serde 往返不一致"),
            }
        }
    }

    #[test]
    fn test_serde_roundtrip_action() {
        let actions = vec![
            RiskAction::None,
            RiskAction::Warn("接近平仓线".into()),
            RiskAction::ForceClose,
        ];
        for a in &actions {
            let json = serde_json::to_string(a).unwrap();
            let back: RiskAction = serde_json::from_str(&json).unwrap();
            match (a, &back) {
                (RiskAction::None, RiskAction::None) => {}
                (RiskAction::Warn(x), RiskAction::Warn(y)) => assert_eq!(x, y),
                (RiskAction::ForceClose, RiskAction::ForceClose) => {}
                _ => panic!("serde 往返不一致"),
            }
        }
    }

    #[test]
    fn test_serde_roundtrip_severity() {
        let severities = vec![RiskSeverity::Info, RiskSeverity::Warning, RiskSeverity::Critical];
        for s in &severities {
            let json = serde_json::to_string(s).unwrap();
            let back: RiskSeverity = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{:?}", s), format!("{:?}", back));
        }
    }

    #[test]
    fn test_risk_alert_creation() {
        let alert = RiskAlert {
            monitor: "test_monitor".into(),
            severity: RiskSeverity::Warning,
            message: "测试告警".into(),
            timestamp: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&alert).unwrap();
        let back: RiskAlert = serde_json::from_str(&json).unwrap();
        assert_eq!(alert.monitor, back.monitor);
        assert_eq!(alert.message, back.message);
    }

    #[test]
    fn test_risk_config_default() {
        let cfg = RiskConfig::default();
        assert!(cfg.params.is_empty());
    }

    #[test]
    fn test_order_decision_reduce_value() {
        let d = OrderDecision::Reduce(3.5);
        let json = serde_json::to_string(&d).unwrap();
        let back: OrderDecision = serde_json::from_str(&json).unwrap();
        match back {
            OrderDecision::Reduce(v) => assert!((v - 3.5).abs() < 1e-6),
            _ => panic!("期望 Reduce"),
        }
    }
}
