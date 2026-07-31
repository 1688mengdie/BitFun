//! taiji-pattern — Chart pattern recognition via multi-dimensional DTW.
//!
//! # Modules
//!
//! - [`dtw`] — DtwEngine: weighted Euclidean DTW + LB_Keogh lower bound
//! - [`index`] — PatternIndex: three-layer index (signature → LB_Keogh → DTW)
//! - [`node`] — PatternMatchNode + ChanNode: ComputeNode implementations
//! - [`fractal`] — 缠论分型检测 (Fractal detection)
//! - [`bi`] — 缠论笔识别 (Bi stroke recognition)
//! - [`hub`] — 缠论中枢检测 (ChanHub detection)
//! - [`divergence`] — 背驰分析 (MACD/斜率/测度)
//! - [`bsp`] — 买卖点识别 (18 种类型)
//!
//! 参考: 量价时空/Phase-2-派发提示词.md:770 — R-2-501 — taiji-pattern ComputeNode

pub mod dtw;
pub mod fractal;
pub mod bi;
pub mod hub;
pub mod divergence;
pub mod segment;
pub mod bsp;
pub mod index;
pub mod node;

pub use bsp::{BSPDetector, BSPType, BuySellPoint};

pub use dtw::DtwEngine;
pub use fractal::{detect_fractals, Fractal, FractalDirection};
pub use bi::{detect_bi, Bi, BiDirection};
pub use hub::{ChanHub, ChanHubDetector, ChanHubNode, HubLevel};
pub use segment::{
    FeatureFractal, FeatureSequence, FourPhases, Segment, SegmentDivider, SegmentStatus,
};
pub use index::{PatternIndex, PatternMatch};
pub use node::{PatternMatchNode, ChanNode};
