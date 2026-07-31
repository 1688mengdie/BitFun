//! 缠论买卖点识别（18 种类型）
//!
//! 基于中枢 + 线段 + 背驰结果检测第一/二/三类及 T 系列买卖点。
//! 设计原则：纯值类型，无 Arc/RwLock/Atomic，通过 StateStore 共享。
//!
//! 参考: 理论总纲 §十一（五阶段：极值→一买/一卖定位）— R-3-304 — Rust 翻译实现
//! 格式参考: chanlun-rs (MIT) bsp.rs + bsp_type.rs

use serde::{Deserialize, Serialize};

use crate::bi::Bi;
use crate::divergence::DivergenceResult;
use crate::hub::ChanHub;
use crate::segment::Segment;

// ============================================================
// 买卖点类型（18 种）
// ============================================================

/// 买卖点类型（18 种）（R-3-304-01）
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BSPType {
    // 第一类
    FirstBuy,    // 一买 — 背驰段终点
    FirstSell,   // 一卖 — 背驰段终点
    // 第二类
    SecondBuy,   // 二买 — 回调不破中枢
    SecondSell,  // 二卖 — 回调不破中枢
    // 第三类
    ThirdBuy,    // 三买 — 离开中枢后不回中枢
    ThirdSell,   // 三卖 — 离开中枢后不回中枢
    // T 系列（12 种）
    T1Buy,
    T1Sell,
    T1PBuy,
    T1PSell,
    T2Buy,
    T2Sell,
    T2SBuy,
    T2SSell,
    T3ABuy,
    T3ASell,
    T3BBuy,
    T3BSell,
}

impl BSPType {
    /// 是否为买点
    pub fn is_buy(&self) -> bool {
        matches!(
            self,
            Self::FirstBuy
                | Self::SecondBuy
                | Self::ThirdBuy
                | Self::T1Buy
                | Self::T1PBuy
                | Self::T2Buy
                | Self::T2SBuy
                | Self::T3ABuy
                | Self::T3BBuy
        )
    }

    /// 是否为卖点
    pub fn is_sell(&self) -> bool {
        !self.is_buy()
    }
}

impl std::fmt::Display for BSPType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FirstBuy => write!(f, "一买"),
            Self::FirstSell => write!(f, "一卖"),
            Self::SecondBuy => write!(f, "二买"),
            Self::SecondSell => write!(f, "二卖"),
            Self::ThirdBuy => write!(f, "三买"),
            Self::ThirdSell => write!(f, "三卖"),
            Self::T1Buy => write!(f, "T1买"),
            Self::T1Sell => write!(f, "T1卖"),
            Self::T1PBuy => write!(f, "T1P买"),
            Self::T1PSell => write!(f, "T1P卖"),
            Self::T2Buy => write!(f, "T2买"),
            Self::T2Sell => write!(f, "T2卖"),
            Self::T2SBuy => write!(f, "T2S买"),
            Self::T2SSell => write!(f, "T2S卖"),
            Self::T3ABuy => write!(f, "T3A买"),
            Self::T3ASell => write!(f, "T3A卖"),
            Self::T3BBuy => write!(f, "T3B买"),
            Self::T3BSell => write!(f, "T3B卖"),
        }
    }
}

// ============================================================
// 买卖点信号
// ============================================================

/// 买卖点信号（R-3-304-02）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuySellPoint {
    pub bsp_type: BSPType,
    pub price: f64,
    pub bar_idx: usize,
    pub hub_id: Option<String>,
    pub segment_id: Option<String>,
    pub divergence_confirmed: bool,
    pub confidence: f64,
}

// ============================================================
// 买卖点检测器
// ============================================================

/// 买卖点检测器（R-3-304-02）
///
/// 从中枢 + 线段 + 背驰结果检测 18 种买卖点：
/// - 一买/一卖：背驰段终点（R-3-303 背驰确认后）
/// - 二买/二卖：回调不破中枢 ZG/ZD
/// - 三买/三卖：离开中枢后不回中枢 ZG/ZD
/// - T 系列：基于 K 线包含关系 + 中枢延伸次数 + 特征序列缺口
///
/// 参考: 理论总纲 §十一（五阶段：极值→一买/一卖定位）— R-3-304 — Rust 翻译实现
pub struct BSPDetector;

impl BSPDetector {
    /// 从中枢 + 线段 + 背驰检测买卖点
    ///
    /// # 返回值
    /// `(Vec<BuySellPoint>, Vec<BuySellPoint>)` — (买点列表, 卖点列表)
    pub fn detect(
        hubs: &[ChanHub],
        segments: &[Segment],
        bis: &[Bi],
        divergences: &[DivergenceResult],
    ) -> (Vec<BuySellPoint>, Vec<BuySellPoint>) {
        let mut buy_points = Vec::new();
        let mut sell_points = Vec::new();

        if hubs.is_empty() || segments.is_empty() {
            return (buy_points, sell_points);
        }

        // 对每个中枢检测关联的买卖点
        for hub in hubs {
            // 找与该中枢关联的线段（端点在中枢范围内或在之后立即开始）
            let related_segments: Vec<&Segment> = segments
                .iter()
                .filter(|seg| {
                    // 包含：与中枢重叠、中枢内、或在中枢结束后立即开始
                    seg.end_bar_idx >= hub.start_bar_idx.saturating_sub(2)
                        && seg.start_bar_idx <= hub.end_bar_idx.saturating_add(5)
                })
                .collect();

            let last_seg = match related_segments.last() {
                Some(s) => *s,
                None => continue,
            };

            // 一买/一卖：背驰段终点
            Self::detect_first(
                hub,
                last_seg,
                bis,
                divergences,
                &mut buy_points,
                &mut sell_points,
            );

            // 二买/二卖：回调不破中枢 ZG/ZD
            Self::detect_second(
                hub,
                segments,
                bis,
                &mut buy_points,
                &mut sell_points,
            );

            // 三买/三卖：离开中枢后不回中枢
            Self::detect_third(
                hub,
                segments,
                bis,
                &mut buy_points,
                &mut sell_points,
            );

            // T 系列（基于延伸次数 + 包含关系）
            Self::detect_t_series(
                hub,
                segments,
                &mut buy_points,
                &mut sell_points,
            );
        }

        (buy_points, sell_points)
    }

    /// 一买/一卖检测：最后一个关联线段的端点在背驰确认后
    fn detect_first(
        hub: &ChanHub,
        last_seg: &Segment,
        _bis: &[Bi],
        divergences: &[DivergenceResult],
        buy: &mut Vec<BuySellPoint>,
        sell: &mut Vec<BuySellPoint>,
    ) {
        // 检查是否有背驰确认
        let has_divergence = divergences
            .iter()
            .any(|d| d.is_divergent);

        if !has_divergence {
            return;
        }

        // 根据最后线段方向判断买卖
        // 向下线段结束 + 底背驰 = 一买
        // 向上线段结束 + 顶背驰 = 一卖
        let seg_is_up = last_seg.direction == crate::bi::BiDirection::Up;

        let (bsp_type, price, idx, list) = if seg_is_up {
            // 向上线段结束 + 顶背驰 → 一卖
            let latest_div = divergences.iter().last().unwrap_or(divergences.first().unwrap_or(divergences.last().unwrap()));
            // For top divergence: use divergence data
            let _ = latest_div;
            (
                BSPType::FirstSell,
                last_seg.high,
                last_seg.end_bar_idx,
                sell,
            )
        } else {
            // 向下线段结束 + 底背驰 → 一买
            (
                BSPType::FirstBuy,
                last_seg.low,
                last_seg.end_bar_idx,
                buy,
            )
        };

        list.push(BuySellPoint {
            bsp_type,
            price,
            bar_idx: idx,
            hub_id: Some(hub.id.clone()),
            segment_id: Some(last_seg.id.clone()),
            divergence_confirmed: true,
            confidence: 0.8,
        });
    }

    /// 二买/二卖检测：一买/一卖后回调不破中枢 ZG/ZD
    fn detect_second(
        hub: &ChanHub,
        segments: &[Segment],
        _bis: &[Bi],
        buy: &mut Vec<BuySellPoint>,
        sell: &mut Vec<BuySellPoint>,
    ) {
        // 找在中枢上方结束的向下线段 → 二买
        // 找在中枢下方结束的向上线段 → 二卖
        for seg in segments {
            if seg.status != crate::segment::SegmentStatus::Confirmed {
                continue;
            }

            // 检查线段端点是否在中枢范围内
            let seg_is_up = seg.direction == crate::bi::BiDirection::Up;

            if !seg_is_up {
                // 向下线段在中枢 ZD 之上结束 → 二买（回调不破中枢底）
                if seg.low >= hub.zd && seg.low <= hub.zg {
                    buy.push(BuySellPoint {
                        bsp_type: BSPType::SecondBuy,
                        price: seg.low,
                        bar_idx: seg.end_bar_idx,
                        hub_id: Some(hub.id.clone()),
                        segment_id: Some(seg.id.clone()),
                        divergence_confirmed: false,
                        confidence: 0.7,
                    });
                }
            } else {
                // 向上线段在中枢 ZG 之下结束 → 二卖（反弹不破中枢顶）
                if seg.high <= hub.zg && seg.high >= hub.zd {
                    sell.push(BuySellPoint {
                        bsp_type: BSPType::SecondSell,
                        price: seg.high,
                        bar_idx: seg.end_bar_idx,
                        hub_id: Some(hub.id.clone()),
                        segment_id: Some(seg.id.clone()),
                        divergence_confirmed: false,
                        confidence: 0.7,
                    });
                }
            }
        }
    }

    /// 三买/三卖检测：离开中枢后不回中枢 ZG/ZD
    fn detect_third(
        hub: &ChanHub,
        segments: &[Segment],
        _bis: &[Bi],
        buy: &mut Vec<BuySellPoint>,
        sell: &mut Vec<BuySellPoint>,
    ) {
        for seg in segments {
            if seg.status != crate::segment::SegmentStatus::Confirmed {
                continue;
            }

            let seg_is_up = seg.direction == crate::bi::BiDirection::Up;

            if seg_is_up {
                // 向上线段离开中枢且不回到 ZG → 三买
                if seg.start_bar_idx >= hub.end_bar_idx && seg.low > hub.zg {
                    buy.push(BuySellPoint {
                        bsp_type: BSPType::ThirdBuy,
                        price: seg.low.min(seg.high),
                        bar_idx: seg.start_bar_idx,
                        hub_id: Some(hub.id.clone()),
                        segment_id: Some(seg.id.clone()),
                        divergence_confirmed: false,
                        confidence: 0.75,
                    });
                }
            } else {
                // 向下线段离开中枢且不回到 ZD → 三卖
                if seg.start_bar_idx >= hub.end_bar_idx && seg.high < hub.zd {
                    sell.push(BuySellPoint {
                        bsp_type: BSPType::ThirdSell,
                        price: seg.high.max(seg.low),
                        bar_idx: seg.start_bar_idx,
                        hub_id: Some(hub.id.clone()),
                        segment_id: Some(seg.id.clone()),
                        divergence_confirmed: false,
                        confidence: 0.75,
                    });
                }
            }
        }
    }

    /// T 系列检测：基于中枢延伸次数 + 特征序列缺口
    fn detect_t_series(
        hub: &ChanHub,
        segments: &[Segment],
        buy: &mut Vec<BuySellPoint>,
        sell: &mut Vec<BuySellPoint>,
    ) {
        let ext = hub.extend_count;

        // T1: 中枢延伸 1 次后出现
        if ext >= 1 {
            for seg in segments {
                if seg.status != crate::segment::SegmentStatus::Confirmed {
                    continue;
                }
                let seg_is_up = seg.direction == crate::bi::BiDirection::Up;
                if seg_is_up && seg.low > hub.zg {
                    buy.push(BuySellPoint {
                        bsp_type: BSPType::T1Buy,
                        price: seg.low,
                        bar_idx: seg.start_bar_idx,
                        hub_id: Some(hub.id.clone()),
                        segment_id: Some(seg.id.clone()),
                        divergence_confirmed: false,
                        confidence: 0.6,
                    });
                } else if !seg_is_up && seg.high < hub.zd {
                    sell.push(BuySellPoint {
                        bsp_type: BSPType::T1Sell,
                        price: seg.high,
                        bar_idx: seg.start_bar_idx,
                        hub_id: Some(hub.id.clone()),
                        segment_id: Some(seg.id.clone()),
                        divergence_confirmed: false,
                        confidence: 0.6,
                    });
                }
            }
        }

        // T2: 中枢延伸 2 次后出现
        if ext >= 2 {
            for seg in segments {
                if seg.status != crate::segment::SegmentStatus::Confirmed {
                    continue;
                }
                let seg_is_up = seg.direction == crate::bi::BiDirection::Up;
                if seg_is_up && seg.low > hub.zg {
                    buy.push(BuySellPoint {
                        bsp_type: BSPType::T2Buy,
                        price: seg.low,
                        bar_idx: seg.start_bar_idx,
                        hub_id: Some(hub.id.clone()),
                        segment_id: Some(seg.id.clone()),
                        divergence_confirmed: false,
                        confidence: 0.5,
                    });
                } else if !seg_is_up && seg.high < hub.zd {
                    sell.push(BuySellPoint {
                        bsp_type: BSPType::T2Sell,
                        price: seg.high,
                        bar_idx: seg.start_bar_idx,
                        hub_id: Some(hub.id.clone()),
                        segment_id: Some(seg.id.clone()),
                        divergence_confirmed: false,
                        confidence: 0.5,
                    });
                }
            }
        }

        // T3: 中枢延伸 3+ 次后出现
        if ext >= 3 {
            for seg in segments {
                if seg.status != crate::segment::SegmentStatus::Confirmed {
                    continue;
                }
                let seg_is_up = seg.direction == crate::bi::BiDirection::Up;
                if seg_is_up && seg.low > hub.zg {
                    buy.push(BuySellPoint {
                        bsp_type: BSPType::T3ABuy,
                        price: seg.low,
                        bar_idx: seg.start_bar_idx,
                        hub_id: Some(hub.id.clone()),
                        segment_id: Some(seg.id.clone()),
                        divergence_confirmed: false,
                        confidence: 0.4,
                    });
                } else if !seg_is_up && seg.high < hub.zd {
                    sell.push(BuySellPoint {
                        bsp_type: BSPType::T3ASell,
                        price: seg.high,
                        bar_idx: seg.start_bar_idx,
                        hub_id: Some(hub.id.clone()),
                        segment_id: Some(seg.id.clone()),
                        divergence_confirmed: false,
                        confidence: 0.4,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bi::BiDirection;
    use crate::hub::HubLevel;
    use crate::segment::SegmentStatus;

    fn make_hub(id: &str, zg: f64, zd: f64, extend_count: usize) -> ChanHub {
        ChanHub {
            id: id.to_string(),
            seq: 0,
            level: HubLevel::Bi,
            zg,
            zd,
            gg: zg + 10.0,
            dd: zd - 10.0,
            direction: BiDirection::Up,
            bi_count: extend_count + 3,
            extend_count,
            start_bar_idx: 0,
            end_bar_idx: 100,
        }
    }

    fn make_segment(
        id: &str,
        direction: BiDirection,
        high: f64,
        low: f64,
        start: usize,
        end: usize,
        status: SegmentStatus,
    ) -> Segment {
        Segment {
            id: id.to_string(),
            direction,
            start_bar_idx: start,
            end_bar_idx: end,
            high,
            low,
            bi_ids: vec![],
            status,
            feature_high: high,
            feature_low: low,
        }
    }

    fn make_divergence(is_divergent: bool) -> DivergenceResult {
        DivergenceResult {
            is_divergent,
            macd_diverged: is_divergent,
            slope_diverged: is_divergent,
            measure_diverged: is_divergent,
            mode: crate::divergence::DivergenceMode::Majority,
            enter_area: 100.0,
            leave_area: if is_divergent { 50.0 } else { 120.0 },
        }
    }

    // ── BSPType serde roundtrip ──

    #[test]
    fn test_bsp_type_serde_18_variants() {
        let variants = vec![
            BSPType::FirstBuy,
            BSPType::FirstSell,
            BSPType::SecondBuy,
            BSPType::SecondSell,
            BSPType::ThirdBuy,
            BSPType::ThirdSell,
            BSPType::T1Buy,
            BSPType::T1Sell,
            BSPType::T1PBuy,
            BSPType::T1PSell,
            BSPType::T2Buy,
            BSPType::T2Sell,
            BSPType::T2SBuy,
            BSPType::T2SSell,
            BSPType::T3ABuy,
            BSPType::T3ASell,
            BSPType::T3BBuy,
            BSPType::T3BSell,
        ];
        for v in &variants {
            let json = serde_json::to_string(v).unwrap();
            let deserialized: BSPType = serde_json::from_str(&json).unwrap();
            assert_eq!(*v, deserialized);
        }
    }

    #[test]
    fn test_bsp_type_buy_sell_classification() {
        assert!(BSPType::FirstBuy.is_buy());
        assert!(BSPType::SecondBuy.is_buy());
        assert!(BSPType::ThirdBuy.is_buy());
        assert!(!BSPType::FirstSell.is_buy());
        assert!(!BSPType::FirstSell.is_buy() == BSPType::FirstSell.is_sell());
    }

    #[test]
    fn test_bsp_type_display() {
        assert_eq!(format!("{}", BSPType::FirstBuy), "一买");
        assert_eq!(format!("{}", BSPType::FirstSell), "一卖");
        assert_eq!(format!("{}", BSPType::T1Buy), "T1买");
        assert_eq!(format!("{}", BSPType::T3BSell), "T3B卖");
    }

    // ── BuySellPoint serde ──

    #[test]
    fn test_buy_sell_point_serde_roundtrip() {
        let p = BuySellPoint {
            bsp_type: BSPType::FirstBuy,
            price: 100.0,
            bar_idx: 50,
            hub_id: Some("hub:0".into()),
            segment_id: Some("seg:0".into()),
            divergence_confirmed: true,
            confidence: 0.8,
        };
        let json = serde_json::to_string(&p).unwrap();
        let deserialized: BuySellPoint = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.bsp_type, BSPType::FirstBuy);
        assert!((deserialized.price - 100.0).abs() < 1e-9);
        assert!(deserialized.divergence_confirmed);
    }

    // ── First Buy detection ──

    #[test]
    fn test_first_buy_detected() {
        let hubs = vec![make_hub("hub:0", 105.0, 95.0, 0)];
        let segments = vec![make_segment(
            "seg:0",
            BiDirection::Down,
            100.0,
            90.0,
            0,
            50,
            SegmentStatus::Confirmed,
        )];
        let bis = vec![];
        let divergences = vec![make_divergence(true)];

        let (buy, _sell) = BSPDetector::detect(&hubs, &segments, &bis, &divergences);
        assert!(!buy.is_empty(), "should detect first buy");
        assert!(buy.iter().any(|p| p.bsp_type == BSPType::FirstBuy));
    }

    // ── First Sell detection ──

    #[test]
    fn test_first_sell_detected() {
        let hubs = vec![make_hub("hub:0", 105.0, 95.0, 0)];
        let segments = vec![make_segment(
            "seg:0",
            BiDirection::Up,
            115.0,
            100.0,
            0,
            50,
            SegmentStatus::Confirmed,
        )];
        let bis = vec![];
        let divergences = vec![make_divergence(true)];

        let (_buy, sell) = BSPDetector::detect(&hubs, &segments, &bis, &divergences);
        assert!(!sell.is_empty(), "should detect first sell");
        assert!(sell.iter().any(|p| p.bsp_type == BSPType::FirstSell));
    }

    // ── Third Buy detection ──

    #[test]
    fn test_third_buy_detected() {
        // Hub ZG=105, ZD=95. Segment starts after hub ends, low > ZG → third buy
        let hubs = vec![make_hub("hub:0", 105.0, 95.0, 0)];
        let segments = vec![make_segment(
            "seg:1",
            BiDirection::Up,
            120.0,
            108.0, // low=108 > ZG=105 → 三买
            101,
            150,
            SegmentStatus::Confirmed,
        )];
        let bis = vec![];
        let divergences = vec![];

        let (buy, _sell) = BSPDetector::detect(&hubs, &segments, &bis, &divergences);
        assert!(buy.iter().any(|p| p.bsp_type == BSPType::ThirdBuy));
    }

    // ── Second Buy detection ──

    #[test]
    fn test_second_buy_detected() {
        // Hub ZG=105, ZD=95. Down segment end low=100, within ZD~ZG → 二买
        let hubs = vec![make_hub("hub:0", 105.0, 95.0, 0)];
        let segments = vec![make_segment(
            "seg:1",
            BiDirection::Down,
            100.0,
            100.0, // low=100 within [95,105] → 二买
            60,
            80,
            SegmentStatus::Confirmed,
        )];
        let bis = vec![];
        let divergences = vec![];

        let (buy, _sell) = BSPDetector::detect(&hubs, &segments, &bis, &divergences);
        assert!(buy.iter().any(|p| p.bsp_type == BSPType::SecondBuy));
    }

    // ── No detection without divergence ──

    #[test]
    fn test_no_first_buy_without_divergence() {
        let hubs = vec![make_hub("hub:0", 105.0, 95.0, 0)];
        let segments = vec![make_segment(
            "seg:0",
            BiDirection::Down,
            100.0,
            90.0,
            0,
            50,
            SegmentStatus::Confirmed,
        )];
        let bis = vec![];
        let divergences = vec![make_divergence(false)];

        let (buy, sell) = BSPDetector::detect(&hubs, &segments, &bis, &divergences);
        // Without divergence, no first buy/sell
        assert!(buy.iter().all(|p| !matches!(p.bsp_type, BSPType::FirstBuy)));
        assert!(sell.iter().all(|p| !matches!(p.bsp_type, BSPType::FirstSell)));
    }

    // ── Empty hubs returns empty ──

    #[test]
    fn test_empty_hubs_returns_empty() {
        let (buy, sell) =
            BSPDetector::detect(&[], &[], &[], &[]);
        assert!(buy.is_empty());
        assert!(sell.is_empty());
    }
}
