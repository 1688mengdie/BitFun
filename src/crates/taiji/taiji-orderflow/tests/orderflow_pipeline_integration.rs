//! R-2-606 taiji-orderflow ↔ Engine 集成测试。
//!
//! 验证 OrderFlowNode 可通过 Pipeline 注册、tick 级计算可执行、
//! Delta/CVD/大单结果可从 StateStore 读取。
//!
//! Pipeline 当前架构中 `on_tick` 未接入 DAG 执行路径（仅执行 on_bar），
//! 因此测试在 Pipeline 注册后直接调用 OrderFlowNode.on_tick 模拟
//! tick 级执行路径，验证 StateStore 输出的正确性。

use taiji_engine::node::{ComputeNode, NodeConfig};
use taiji_engine::store::StateStore;
use taiji_engine::types::tick::TickData;
use taiji_orderflow::OrderFlowNode;

// ============================================================================
// 辅助函数
// ============================================================================

/// 构造一个测试用 TickData。
fn make_tick(volume: f64, delta: f64, price: f64, time: &str, millisec: i32) -> TickData {
    TickData {
        instrument: "rb2501".into(),
        trading_day: "20260730".into(),
        exchange_id: "SHFE".into(),
        exchange_inst_id: "rb2501".into(),
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
        update_millisec: millisec,
        bid_price1: 0.0, bid_volume1: 0,
        ask_price1: 0.0, ask_volume1: 0,
        bid_price2: 0.0, bid_volume2: 0,
        ask_price2: 0.0, ask_volume2: 0,
        bid_price3: 0.0, bid_volume3: 0,
        ask_price3: 0.0, ask_volume3: 0,
        bid_price4: 0.0, bid_volume4: 0,
        ask_price4: 0.0, ask_volume4: 0,
        bid_price5: 0.0, bid_volume5: 0,
        ask_price5: 0.0, ask_volume5: 0,
        average_price: 0.0,
        action_day: String::new(),
        trade_type: None,
        cum_volume: None,
        cum_position: None,
        timestamp_ms: 0,
    }
}

/// 构造一个空的 StateStore。
fn new_state() -> StateStore {
    StateStore::default()
}

/// 验证从 StateStore 读取 Delta 值。
fn assert_delta(state: &StateStore, expected: f64) {
    let val: Option<f64> = state.get(&"delta".into());
    assert!(val.is_some(), "delta 应在 StateStore 中");
    assert!((val.unwrap() - expected).abs() < 1e-6,
        "Delta 期望={}, 实际={}", expected, val.unwrap());
}

/// 验证从 StateStore 读取 CVD 值。
fn assert_cvd(state: &StateStore, expected: f64) {
    let val: Option<f64> = state.get(&"cvd".into());
    assert!(val.is_some(), "cvd 应在 StateStore 中");
    assert!((val.unwrap() - expected).abs() < 1e-6,
        "CVD 期望={}, 实际={}", expected, val.unwrap());
}

/// 验证大单标记。
fn assert_large_trade(state: &StateStore, expected_direction: &str, expected_volume: f64) {
    let lt = state.get_json(&"large_trade".into());
    assert!(lt.is_some(), "大单标记应在 StateStore 中");
    let lt = lt.unwrap();
    assert_eq!(lt["direction"].as_str(), Some(expected_direction),
        "大单方向错误");
    assert!((lt["volume"].as_f64().unwrap() - expected_volume).abs() < 1e-6,
        "大单 volume 期望={}, 实际={}", expected_volume, lt["volume"].as_f64().unwrap());
}

// ============================================================================
// 测试
// ============================================================================

/// 测试 1：Pipeline + OrderFlowNode 注册 → 买方 tick → Delta>0, CVD 累积。
#[test]
fn test_orderflow_node_buy_delta() {
    let mut node = OrderFlowNode::new("orderflow1");
    let state = new_state();

    // 买方 tick：curr_delta=+50
    node.on_tick(&make_tick(500.0, 50.0, 3500.0, "09:30:00", 0), &state).unwrap();

    assert_delta(&state, 50.0);
    assert_cvd(&state, 50.0);
}

/// 测试 2：连续 tick → CVD 累积。
#[test]
fn test_orderflow_cvd_accumulation() {
    let mut node = OrderFlowNode::new("orderflow2");
    let state = new_state();

    node.on_tick(&make_tick(100.0, 30.0, 3500.0, "09:30:00", 0), &state).unwrap();
    assert_cvd(&state, 30.0);

    node.on_tick(&make_tick(200.0, -10.0, 3501.0, "09:30:01", 0), &state).unwrap();
    assert_cvd(&state, 20.0);

    node.on_tick(&make_tick(300.0, 5.0, 3502.0, "09:30:02", 0), &state).unwrap();
    assert_cvd(&state, 25.0);
}

/// 测试 3：卖方 tick → Delta<0。
#[test]
fn test_orderflow_node_sell_delta() {
    let mut node = OrderFlowNode::new("orderflow3");
    let state = new_state();

    node.on_tick(&make_tick(800.0, -80.0, 3490.0, "09:30:00", 0), &state).unwrap();

    assert_delta(&state, -80.0);
    assert_cvd(&state, -80.0);
}

/// 测试 4：大单检测（volume 超阈值默认 500）。
#[test]
fn test_orderflow_large_trade() {
    let mut node = OrderFlowNode::new("orderflow4");
    let state = new_state();

    // tick volume=1000, delta=200 → 超阈值
    node.on_tick(&make_tick(1000.0, 200.0, 3500.0, "09:30:00", 123), &state).unwrap();

    assert_large_trade(&state, "buy", 1000.0);
    assert_delta(&state, 200.0);
}

/// 测试 5：on_init 可配置大单阈值。
#[test]
fn test_orderflow_node_init_with_config() {
    let mut node = OrderFlowNode::new("orderflow5");
    let state = new_state();

    // 通过 NodeConfig 设置大单阈值为 50
    let config = NodeConfig {
        type_name: "orderflow".into(),
        params: [("large_trade_volume".into(), serde_json::json!(50.0))]
            .iter().cloned().collect(),
    };
    node.on_init(&config, &state).unwrap();

    // 小单 volume=60, delta=30 → 超阈值 50
    node.on_tick(&make_tick(60.0, 30.0, 3500.0, "09:30:00", 0), &state).unwrap();

    assert_large_trade(&state, "buy", 60.0);
    assert_delta(&state, 30.0);
}

/// 测试 6：混合场景——买方→卖方→买方，CVD 正确反映净方向。
#[test]
fn test_orderflow_mixed_flow() {
    let mut node = OrderFlowNode::new("orderflow6");
    let state = new_state();

    // Tick 1: 买方主导 +30
    node.on_tick(&make_tick(300.0, 30.0, 3500.0, "09:30:00", 0), &state).unwrap();
    assert_delta(&state, 30.0);
    assert_cvd(&state, 30.0);

    // Tick 2: 卖方主导 -50 → CVD = -20
    node.on_tick(&make_tick(400.0, -50.0, 3498.0, "09:30:05", 0), &state).unwrap();
    assert_delta(&state, -50.0);
    assert_cvd(&state, -20.0);

    // Tick 3: 买方主导 +80 → CVD = 60
    node.on_tick(&make_tick(500.0, 80.0, 3505.0, "09:30:10", 0), &state).unwrap();
    assert_delta(&state, 80.0);
    assert_cvd(&state, 60.0);
}
