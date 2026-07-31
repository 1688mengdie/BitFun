//! 订单流分析 ComputeNode — Delta / CVD / 大单检测。
//!
//! Delta（逐笔成交方向统计）：tick 级买卖压力。
//! CVD（Cumulative Volume Delta）：Delta 的累积值，衡量长期买卖压力。
//! 大单检测：单笔成交量超过阈值时标记。
//!
//! 输入：tick（实时行情）
//! 输出：delta / cvd / large_trade
//! 参考: 量价时空/Phase-2-派发提示词.md:795 — R-2-502 — taiji-orderflow ComputeNode

use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::state::StateValue;
use taiji_engine::types::tick::TickData;

/// 订单流分析节点配置参数。
pub struct OrderFlowConfig {
    /// 大单判定阈值（单笔成交量超过此值标记为大单）。
    pub large_trade_volume: f64,
}

impl Default for OrderFlowConfig {
    fn default() -> Self {
        Self { large_trade_volume: 500.0 }
    }
}

/// 订单流分析 ComputeNode。
///
/// 在 on_tick 中完成三项计算并写入 StateStore：
/// - `"delta"`：当前 tick 的 Delta（f64，正值=买方主导，负值=卖方主导）
/// - `"cvd"`：累计 Delta（f64，session 内持续累积）
/// - `"large_trade"`：大单标记（JSON：{volume, price, direction, timestamp}）
pub struct OrderFlowNode {
    id: NodeId,
    /// 累计 Delta（CVD）。
    cumulative_delta: f64,
    /// 上一 tick 的 volume（用于推算 Delta）。
    prev_volume: f64,
    /// 大单成交量阈值。
    large_trade_threshold: f64,
}

impl OrderFlowNode {
    pub fn new(node_id: &str) -> Self {
        Self {
            id: node_id.to_string(),
            cumulative_delta: 0.0,
            prev_volume: 0.0,
            large_trade_threshold: OrderFlowConfig::default().large_trade_volume,
        }
    }
}

impl ComputeNode for OrderFlowNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "orderflow"
    }

    fn input_keys(&self) -> Vec<String> {
        vec!["tick".into()]
    }

    fn output_keys(&self) -> Vec<String> {
        vec!["delta".into(), "cvd".into(), "large_trade".into()]
    }

    fn on_init(&mut self, config: &NodeConfig, _state: &StateStore) -> Result<()> {
        if let Some(v) = config.get_f64("large_trade_volume") {
            self.large_trade_threshold = v;
        }
        Ok(())
    }

    fn on_tick(&mut self, tick: &TickData, state: &StateStore) -> Result<()> {
        // ===== Delta 计算 =====
        // 优先使用 CTP 提供的 curr_delta（买卖单方向统计），
        // 若无 delta 数据则通过 volume 变化推算。
        let delta = if tick.curr_delta != 0.0 {
            tick.curr_delta
        } else {
            // 用 volume 增量估计 Delta
            tick.volume - self.prev_volume
        };

        // ===== 大单检测 =====
        // 使用本 tick 的实际成交量（与 delta 计算独立）
        let tick_volume = (tick.volume - self.prev_volume).abs();
        let is_large_trade = tick_volume >= self.large_trade_threshold;

        // 更新上一 tick volume（必须在 tick_volume 计算之后）
        self.prev_volume = tick.volume;

        // ===== CVD（累计成交量差）=====
        self.cumulative_delta += delta;

        // ===== 写入 StateStore =====
        state.set("delta".into(), StateValue::F64(delta), self.id());
        state.set("cvd".into(), StateValue::F64(self.cumulative_delta), self.id());

        if is_large_trade {
            let direction = if delta > 0.0 { "buy" } else { "sell" };
            let ts = format!("{}.{}", tick.update_time, tick.update_millisec);
            state.set(
                "large_trade".into(),
                StateValue::Json(serde_json::json!({
                    "volume": tick_volume,
                    "price": tick.last_price,
                    "direction": direction,
                    "timestamp": ts,
                })),
                self.id(),
            );
        }

        Ok(())
    }

    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
        // Bar 闭合时重置 CVD（按交易节/日重置由 Pipeline 或上级调度决定）
        // 此处不清零，保持累积状态
        Ok(())
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::Tick]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taiji_engine::store::StateStore;
    use taiji_engine::types::tick::TickData;

    fn make_tick(volume: f64, delta: f64, price: f64, time: &str) -> TickData {
        TickData {
            instrument: "test".into(),
            trading_day: "20260730".into(),
            exchange_id: "DCE".into(),
            exchange_inst_id: "test".into(),
            last_price: price,
            pre_settlement_price: 0.0,
            pre_close_price: 0.0,
            pre_open_interest: 0.0,
            open_price: 0.0,
            highest_price: 0.0,
            lowest_price: 0.0,
            volume,
            turnover: 0.0,
            open_interest: 0.0,
            close_price: 0.0,
            settlement_price: 0.0,
            upper_limit_price: 0.0,
            lower_limit_price: 0.0,
            pre_delta: 0.0,
            curr_delta: delta,
            update_time: time.into(),
            update_millisec: 0,
            bid_price1: 0.0,
            bid_volume1: 0,
            ask_price1: 0.0,
            ask_volume1: 0,
            bid_price2: 0.0,
            bid_volume2: 0,
            ask_price2: 0.0,
            ask_volume2: 0,
            bid_price3: 0.0,
            bid_volume3: 0,
            ask_price3: 0.0,
            ask_volume3: 0,
            bid_price4: 0.0,
            bid_volume4: 0,
            ask_price4: 0.0,
            ask_volume4: 0,
            bid_price5: 0.0,
            bid_volume5: 0,
            ask_price5: 0.0,
            ask_volume5: 0,
            average_price: 0.0,
            action_day: String::new(),
            trade_type: None,
            cum_volume: None,
            cum_position: None,
            timestamp_ms: 0,
        }
    }

    #[test]
    fn test_delta_from_curr_delta() {
        let mut node = OrderFlowNode::new("delta_test");
        let store = StateStore::new();
        let tick = make_tick(1000.0, 50.0, 3500.0, "10:00:00");
        node.on_tick(&tick, &store).unwrap();

        let delta: Option<f64> = store.get(&"delta".into());
        assert!(delta.is_some());
        assert!((delta.unwrap() - 50.0).abs() < 1e-6, "Delta 应等于 curr_delta");
    }

    #[test]
    fn test_cvd_accumulates() {
        let mut node = OrderFlowNode::new("cvd_test");
        let store = StateStore::new();

        // Tick 1: delta=30
        node.on_tick(&make_tick(500.0, 30.0, 3500.0, "10:00:00"), &store).unwrap();
        let cvd: Option<f64> = store.get(&"cvd".into());
        assert!((cvd.unwrap() - 30.0).abs() < 1e-6, "CVD = 30");

        // Tick 2: delta=-10
        node.on_tick(&make_tick(600.0, -10.0, 3501.0, "10:00:01"), &store).unwrap();
        let cvd: Option<f64> = store.get(&"cvd".into());
        assert!((cvd.unwrap() - 20.0).abs() < 1e-6, "CVD = 20 (30-10)");

        // Tick 3: delta=5
        node.on_tick(&make_tick(700.0, 5.0, 3502.0, "10:00:02"), &store).unwrap();
        let cvd: Option<f64> = store.get(&"cvd".into());
        assert!((cvd.unwrap() - 25.0).abs() < 1e-6, "CVD = 25 (30-10+5)");
    }

    #[test]
    fn test_large_trade_detected() {
        let mut node = OrderFlowNode::new("large_trade_test");
        let store = StateStore::new();

        // 设置大单阈值为 400，tick volume=1000 > 400
        let config = taiji_engine::node::NodeConfig {
            type_name: "orderflow".into(),
            params: [("large_trade_volume".into(), serde_json::json!(400.0))]
                .iter()
                .cloned()
                .collect(),
        };
        node.on_init(&config, &store).unwrap();

        // 模拟一个 delta=200 的大 tick（volume=1000, curr_delta=200）
        node.on_tick(&make_tick(1000.0, 200.0, 3500.0, "10:00:00"), &store).unwrap();

        let lt = store.get_json(&"large_trade".into());
        assert!(lt.is_some(), "应检测到大单");
        let lt = lt.unwrap();
        assert_eq!(lt["direction"].as_str(), Some("buy"));
        assert!((lt["volume"].as_f64().unwrap() - 1000.0).abs() < 1e-6, "large_trade.volume 应为 tick volume=1000");
    }

    #[test]
    fn test_no_large_trade_for_small_tick() {
        let mut node = OrderFlowNode::new("small_tick");
        let store = StateStore::new();

        // 小 tick：volume=50，远小于阈值
        node.on_tick(&make_tick(50.0, 10.0, 3500.0, "10:00:00"), &store).unwrap();

        let lt = store.get_json(&"large_trade".into());
        assert!(lt.is_none(), "小 tick 不应标记为大单");
    }

    #[test]
    fn test_delta_negative_for_sell_pressure() {
        let mut node = OrderFlowNode::new("sell_test");
        let store = StateStore::new();

        node.on_tick(&make_tick(800.0, -80.0, 3490.0, "10:00:00"), &store).unwrap();

        let delta: Option<f64> = store.get(&"delta".into());
        assert!(delta.is_some());
        assert!((delta.unwrap() + 80.0).abs() < 1e-6, "卖出方向 Delta 应为负");

        let lt = store.get_json(&"large_trade".into());
        assert!(lt.is_some(), "大 delta 应触发大单检测");
        let lt_json = lt.unwrap();
        assert_eq!(lt_json["direction"].as_str(), Some("sell"));
        assert!((lt_json["volume"].as_f64().unwrap() - 800.0).abs() < 1e-6, "large_trade.volume 应为 tick volume=800");
    }
}
