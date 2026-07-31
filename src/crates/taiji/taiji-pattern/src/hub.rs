//! 缠论中枢检测 — 纯值类型实现
//!
//! 中枢由三段连续有重叠的笔构成，支持延伸和级别升级。
//! 设计原则：纯值类型，无 Arc/RwLock/Atomic，通过 StateStore 共享。
//!
//! 参考: chanlun-rs (MIT) hub.rs — 仅作算法格式参考
//! 理论: 理论总纲 §四（波段结构）+ 量化总纲 §3 节点7（换筹区间识别）

use serde::{Deserialize, Serialize};

use crate::bi::{Bi, BiDirection};

// ============================================================
// 中枢级别
// ============================================================

/// 中枢级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HubLevel {
    Bi,        // 笔中枢（level 0）
    Segment,   // 线段中枢（level 1）
    Multi,     // 多级联立（level 2+）
}

// ============================================================
// 中枢结构体
// ============================================================

/// 中枢 — 三段虚线重叠区间构成的价格中枢
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChanHub {
    pub id: String,                  // 中枢标识 "hub:<seq>"
    pub seq: u64,                    // 中枢序号
    pub level: HubLevel,             // 中枢级别
    pub zg: f64,                     // 中枢上沿（min 前三段 high）
    pub zd: f64,                     // 中枢下沿（max 前三段 low）
    pub gg: f64,                     // 中枢最高点（max 所有段 high）
    pub dd: f64,                     // 中枢最低点（min 所有段 low）
    pub direction: BiDirection,      // 中枢方向（第一段方向翻转）
    pub bi_count: usize,             // 构成中枢的笔数量
    pub extend_count: usize,         // 延伸次数（>0 表示延伸中枢）
    pub start_bar_idx: usize,        // 中枢起始 K 线索引
    pub end_bar_idx: usize,          // 中枢结束 K 线索引
}

// ============================================================
// 笔的极值辅助
// ============================================================

/// 获取笔的最高价（端点极值）
fn bi_high(bi: &Bi) -> f64 {
    bi.start_price.max(bi.end_price)
}

/// 获取笔的最低价（端点极值）
fn bi_low(bi: &Bi) -> f64 {
    bi.start_price.min(bi.end_price)
}

// ============================================================
// 中枢检测器
// ============================================================

/// 中枢检测器 — 纯函数式算法，无内部状态
pub struct ChanHubDetector;

impl ChanHubDetector {
    /// 从 Bi 序列检测中枢
    ///
    /// # 参数
    /// - `bis`: 按时间升序排列的 Bi 切片
    ///
    /// # 返回
    /// 按时间升序的中枢列表
    pub fn detect(bis: &[Bi]) -> Vec<ChanHub> {
        if bis.len() < 3 {
            return vec![];
        }

        let mut hubs: Vec<ChanHub> = Vec::new();
        let mut hub_bi_indices: Vec<Vec<usize>> = Vec::new(); // 每个中枢对应的 Bi 索引列表
        let mut i = 0;

        // 滑动窗口：寻找第一个三笔重叠区间
        while i + 3 <= bis.len() {
            let window = &bis[i..i + 3];
            let lows: Vec<f64> = window.iter().map(bi_low).collect();
            let highs: Vec<f64> = window.iter().map(bi_high).collect();

            let max_low = lows
                .iter()
                .cloned()
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.0);
            let min_high = highs
                .iter()
                .cloned()
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.0);

            // 重叠判定：max(low) < min(high) 时构成中枢
            if max_low < min_high {
                // 找到中枢！检查此中枢是否与已有中枢重叠/延伸
                let zg = min_high;
                let zd = max_low;
                let gg = highs
                    .iter()
                    .cloned()
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(0.0);
                let dd = lows
                    .iter()
                    .cloned()
                    .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(0.0);

                let first_dir = window[0].direction;
                let direction = match first_dir {
                    BiDirection::Up => BiDirection::Down,
                    BiDirection::Down => BiDirection::Up,
                };

                let start_idx = window[0].start_index;
                let end_idx = window[2].end_index;

                let mut hub = ChanHub {
                    id: format!("hub:{}", hubs.len() + 1),
                    seq: hubs.len() as u64 + 1,
                    level: HubLevel::Bi,
                    zg,
                    zd,
                    gg,
                    dd,
                    direction,
                    bi_count: 3,
                    extend_count: 0,
                    start_bar_idx: start_idx,
                    end_bar_idx: end_idx,
                };

                let mut bi_indices: Vec<usize> = (i..i + 3).collect();

                // 延伸检测：检查后续 Bi 是否在此中枢范围内
                let mut j = i + 3;
                while j < bis.len() {
                    if Self::is_within_hub_impl(&bis[j], zg, zd) {
                        // 更新中枢范围
                        let b_high = bi_high(&bis[j]);
                        let b_low = bi_low(&bis[j]);
                        if b_high > hub.gg {
                            hub.gg = b_high;
                        }
                        if b_low < hub.dd {
                            hub.dd = b_low;
                        }
                        hub.bi_count += 1;
                        hub.extend_count += 1;
                        hub.end_bar_idx = bis[j].end_index;
                        bi_indices.push(j);
                        j += 1;
                    } else {
                        break;
                    }
                }

                // 检查是否与上一个中枢合并（相邻中枢重叠时合并）
                if let Some(_last_hub_indices) = hub_bi_indices.last() {
                    if let Some(last_hub) = hubs.last() {
                        if Self::hubs_overlap(&hub, last_hub) {
                            // 合并到上一个中枢
                            let merged = Self::merge_hubs(last_hub, &hub);
                            if let Some(last) = hubs.last_mut() {
                                *last = merged;
                            }
                            // 更新索引
                            if let Some(last_indices) = hub_bi_indices.last_mut() {
                                last_indices.extend(bi_indices);
                            }
                            i = j;
                            continue;
                        }
                    }
                }

                hubs.push(hub);
                hub_bi_indices.push(bi_indices);
                i = j;
            } else {
                i += 1;
            }
        }

        hubs
    }

    /// 检测中枢延伸 — 指定 Bi 是否在中枢范围内
    pub fn is_within_hub(bi: &Bi, hub: &ChanHub) -> bool {
        Self::is_within_hub_impl(bi, hub.zg, hub.zd)
    }

    /// 内部实现：Bi 的 low ≤ zg 且 high ≥ zd
    fn is_within_hub_impl(bi: &Bi, zg: f64, zd: f64) -> bool {
        let b_high = bi_high(bi);
        let b_low = bi_low(bi);
        b_low <= zg && b_high >= zd
    }

    /// 中枢级别升级检测 — 延伸段数 ≥ 9 时级别+1
    pub fn try_upgrade(hub: &ChanHub) -> Option<HubLevel> {
        if hub.extend_count >= 9 {
            Some(HubLevel::Segment)
        } else {
            None
        }
    }

    /// 两个中枢是否有重叠区域
    fn hubs_overlap(a: &ChanHub, b: &ChanHub) -> bool {
        // 在中枢 ZG/ZD 层面检查重叠
        a.zd < b.zg && b.zd < a.zg
    }

    /// 合并两个重叠的中枢
    fn merge_hubs(existing: &ChanHub, incoming: &ChanHub) -> ChanHub {
        ChanHub {
            id: existing.id.clone(),
            seq: existing.seq,
            level: existing.level,
            zg: existing.zg.min(incoming.zg),
            zd: existing.zd.max(incoming.zd),
            gg: existing.gg.max(incoming.gg),
            dd: existing.dd.min(incoming.dd),
            direction: existing.direction,
            bi_count: existing.bi_count + incoming.bi_count,
            extend_count: existing.extend_count + incoming.extend_count + 1, // +1 for merge itself
            start_bar_idx: existing.start_bar_idx.min(incoming.start_bar_idx),
            end_bar_idx: existing.end_bar_idx.max(incoming.end_bar_idx),
        }
    }
}

// ============================================================
// ChanHubNode — ComputeNode 封装
// ============================================================

use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::signal::Signal;
use taiji_engine::types::state::{StateKey, StateValue};
use taiji_engine::types::tick::TickData;
use taiji_engine::types::NodeId;

/// ComputeNode 封装：从 Bi 序列检测中枢
///
/// input: `chan:bis`（笔序列，由 ChanNode 写入）
/// output: `hubs`（中枢列表）
/// 纯数值计算，无 async，符合 L1 合规
pub struct ChanHubNode {
    node_id: NodeId,
}

impl ChanHubNode {
    pub fn new(node_id: NodeId) -> Self {
        Self { node_id }
    }
}

impl ComputeNode for ChanHubNode {
    fn id(&self) -> NodeId {
        self.node_id.clone()
    }

    fn name(&self) -> &'static str {
        "ChanHub"
    }

    fn input_keys(&self) -> Vec<StateKey> {
        vec!["chan:bis".into()]
    }

    fn output_keys(&self) -> Vec<StateKey> {
        vec!["hubs".into()]
    }

    fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> {
        Ok(())
    }

    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
        Ok(())
    }

    fn on_tick(&mut self, _tick: &TickData, _state: &StateStore) -> Result<()> {
        Ok(())
    }

    fn on_calculate(&mut self, state: &StateStore) -> Result<Vec<Signal>> {
        // 从 StateStore 读取 bis（JSON 格式）
        let bis_json = match state.get_raw(&"chan:bis".into()) {
            Some(StateValue::Json(val)) => val,
            _ => return Ok(vec![]),
        };

        // 反序列化为 Vec<Bi>
        let bis: Vec<Bi> = match serde_json::from_value(bis_json) {
            Ok(b) => b,
            Err(_) => return Ok(vec![]),
        };

        // 检测中枢
        let hubs = ChanHubDetector::detect(&bis);

        // 写入 StateStore
        state.set(
            "hubs".into(),
            StateValue::Json(serde_json::to_value(&hubs)?),
            self.node_id.clone(),
        );

        // 中枢本身不产生交易信号（由买卖点节点处理）
        Ok(vec![])
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::F5]
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bi(
        start_index: usize,
        end_index: usize,
        direction: BiDirection,
        start_price: f64,
        end_price: f64,
    ) -> Bi {
        Bi {
            start_index,
            end_index,
            direction,
            start_price,
            end_price,
        }
    }

    fn up_bi(s: usize, e: usize, sp: f64, ep: f64) -> Bi {
        make_bi(s, e, BiDirection::Up, sp, ep)
    }

    fn down_bi(s: usize, e: usize, sp: f64, ep: f64) -> Bi {
        make_bi(s, e, BiDirection::Down, sp, ep)
    }

    // ========== 中枢检测测试 ==========

    #[test]
    fn test_three_overlapping_bis_form_hub() {
        // 三段重叠 Bi：上→下→上，在相近价格区间内
        let bis = vec![
            up_bi(0, 5, 100.0, 110.0),    // up: 100→110
            down_bi(5, 10, 110.0, 103.0), // down: 110→103
            up_bi(10, 15, 103.0, 108.0),  // up: 103→108
        ];
        let hubs = ChanHubDetector::detect(&bis);
        assert_eq!(hubs.len(), 1, "three overlapping bis should form one hub");

        // ZG = min(highs) = min(110, 110, 108) = 108
        assert!((hubs[0].zg - 108.0).abs() < 1e-8, "zg should be 108");
        // ZD = max(lows) = max(100, 103, 103) = 103
        assert!((hubs[0].zd - 103.0).abs() < 1e-8, "zd should be 103");
        // GG = max(highs) = max(110, 110, 108) = 110
        assert!((hubs[0].gg - 110.0).abs() < 1e-8, "gg should be 110");
        // DD = min(lows) = min(100, 103, 103) = 100
        assert!((hubs[0].dd - 100.0).abs() < 1e-8, "dd should be 100");
    }

    #[test]
    fn test_no_overlap_returns_empty() {
        // 三段完全不重叠
        let bis = vec![
            up_bi(0, 5, 100.0, 105.0),     // 100→105
            down_bi(5, 10, 105.0, 102.0),  // 105→102 (overlaps with first)
            up_bi(10, 15, 200.0, 210.0),   // 200→210 (way above, no overlap with previous)
        ];
        let hubs = ChanHubDetector::detect(&bis);
        assert!(hubs.is_empty(), "non-overlapping bis should return empty");
    }

    #[test]
    fn test_less_than_three_bis_returns_empty() {
        let bis = vec![up_bi(0, 5, 100.0, 110.0)];
        assert!(ChanHubDetector::detect(&bis).is_empty());

        let bis = vec![
            up_bi(0, 5, 100.0, 110.0),
            down_bi(5, 10, 110.0, 103.0),
        ];
        assert!(ChanHubDetector::detect(&bis).is_empty());
    }

    #[test]
    fn test_empty_bis_returns_empty() {
        let bis: Vec<Bi> = vec![];
        assert!(ChanHubDetector::detect(&bis).is_empty());
    }

    #[test]
    fn test_five_bis_with_extension() {
        // 前 3 个重叠构成中枢，后 2 个在中枢范围内 → extend_count=2
        let bis = vec![
            up_bi(0, 5, 100.0, 110.0),     // up: 100→110, high=110, low=100
            down_bi(5, 10, 110.0, 103.0),  // down: 110→103, high=110, low=103
            up_bi(10, 15, 103.0, 108.0),   // up: 103→108, high=108, low=103
            // ZG=108, ZD=103
            down_bi(15, 20, 108.0, 104.0), // down: 108→104, high=108, low=104 → low(104) > zd(103)? No, 104>103
            // high=108 > zd=103 ✓, low=104 < zg=108 ✓ → within hub
            up_bi(20, 25, 104.0, 107.0),   // up: 104→107, high=107, low=104 → within hub
        ];
        let hubs = ChanHubDetector::detect(&bis);
        assert_eq!(hubs.len(), 1, "should form one hub with extensions");
        assert_eq!(hubs[0].extend_count, 2, "should have 2 extensions");
        assert_eq!(hubs[0].bi_count, 5, "should have 5 bis total");
    }

    #[test]
    fn test_is_within_hub() {
        let hub = ChanHub {
            id: "hub:1".into(),
            seq: 1,
            level: HubLevel::Bi,
            zg: 108.0,
            zd: 103.0,
            gg: 110.0,
            dd: 100.0,
            direction: BiDirection::Down,
            bi_count: 3,
            extend_count: 0,
            start_bar_idx: 0,
            end_bar_idx: 15,
        };

        // Bi 在中枢范围内：low(104) ≤ zg(108) && high(106) ≥ zd(103) ✓
        let inside = down_bi(15, 20, 106.0, 104.0);
        assert!(ChanHubDetector::is_within_hub(&inside, &hub));

        // Bi 完全在中枢上方：low(109) > zg(108) → not within
        let above = up_bi(15, 20, 109.0, 115.0);
        assert!(!ChanHubDetector::is_within_hub(&above, &hub));

        // Bi 完全在中枢下方：high(102) < zd(103) → not within
        let below = down_bi(15, 20, 102.0, 100.0);
        assert!(!ChanHubDetector::is_within_hub(&below, &hub));
    }

    // ========== 级别升级测试 ==========

    #[test]
    fn test_try_upgrade_returns_none_when_under_9() {
        let hub = ChanHub {
            id: "hub:1".into(),
            seq: 1,
            level: HubLevel::Bi,
            zg: 108.0,
            zd: 103.0,
            gg: 110.0,
            dd: 100.0,
            direction: BiDirection::Down,
            bi_count: 5,
            extend_count: 5,
            start_bar_idx: 0,
            end_bar_idx: 30,
        };
        assert_eq!(ChanHubDetector::try_upgrade(&hub), None);
    }

    #[test]
    fn test_try_upgrade_returns_segment_when_9_or_more() {
        let hub = ChanHub {
            id: "hub:1".into(),
            seq: 1,
            level: HubLevel::Bi,
            zg: 108.0,
            zd: 103.0,
            gg: 110.0,
            dd: 100.0,
            direction: BiDirection::Down,
            bi_count: 12,
            extend_count: 9,
            start_bar_idx: 0,
            end_bar_idx: 60,
        };
        assert_eq!(
            ChanHubDetector::try_upgrade(&hub),
            Some(HubLevel::Segment)
        );
    }

    // ========== Serde 测试 ==========

    #[test]
    fn test_chan_hub_serde_roundtrip() {
        let hub = ChanHub {
            id: "hub:1".into(),
            seq: 1,
            level: HubLevel::Bi,
            zg: 108.0,
            zd: 103.0,
            gg: 110.0,
            dd: 100.0,
            direction: BiDirection::Down,
            bi_count: 3,
            extend_count: 0,
            start_bar_idx: 0,
            end_bar_idx: 15,
        };
        let json = serde_json::to_string(&hub).unwrap();
        let back: ChanHub = serde_json::from_str(&json).unwrap();
        assert_eq!(hub.id, back.id);
        assert!((hub.zg - back.zg).abs() < 1e-8);
        assert!((hub.zd - back.zd).abs() < 1e-8);
        assert!((hub.gg - back.gg).abs() < 1e-8);
        assert!((hub.dd - back.dd).abs() < 1e-8);
        assert_eq!(hub.direction, back.direction);
        assert_eq!(hub.level, back.level);
    }

    #[test]
    fn test_hub_level_serde() {
        for level in &[HubLevel::Bi, HubLevel::Segment, HubLevel::Multi] {
            let json = serde_json::to_string(level).unwrap();
            let back: HubLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(*level, back);
        }
    }

    // ========== 方向测试 ==========

    #[test]
    fn test_hub_direction_is_opposite_of_first_bi() {
        // 第一段 Up → 中枢方向 Down
        let bis = vec![
            up_bi(0, 5, 100.0, 110.0),     // Up
            down_bi(5, 10, 110.0, 103.0),  // Down
            up_bi(10, 15, 103.0, 108.0),   // Up
        ];
        let hubs = ChanHubDetector::detect(&bis);
        assert_eq!(hubs[0].direction, BiDirection::Down);

        // 第一段 Down → 中枢方向 Up
        let bis = vec![
            down_bi(0, 5, 110.0, 100.0),   // Down
            up_bi(5, 10, 100.0, 108.0),    // Up
            down_bi(10, 15, 108.0, 102.0), // Down
        ];
        let hubs = ChanHubDetector::detect(&bis);
        assert_eq!(hubs[0].direction, BiDirection::Up);
    }

    // ========== 合并测试 ==========

    #[test]
    fn test_two_separate_hubs() {
        // 第一组中枢：100-110 区间
        // 第二组中枢：200-210 区间（不重叠）
        let bis = vec![
            up_bi(0, 5, 100.0, 110.0),
            down_bi(5, 10, 110.0, 103.0),
            up_bi(10, 15, 103.0, 108.0),
            // 跳到上方形成第二个中枢
            up_bi(20, 25, 200.0, 210.0),
            down_bi(25, 30, 210.0, 203.0),
            up_bi(30, 35, 203.0, 208.0),
        ];
        let hubs = ChanHubDetector::detect(&bis);
        assert_eq!(hubs.len(), 2, "should detect two separate hubs");
    }
}
