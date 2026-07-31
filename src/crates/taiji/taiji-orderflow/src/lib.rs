//! taiji-orderflow — Order flow analysis (MIT)
//!
//! Tick microstructure analysis for futures markets.
//! Provides VPIN (toxicity / informed-trading probability) and
//! OFI (order flow imbalance) as pluggable [`ComputeNode`]s,
//! built on Welford's online statistics for streaming operation.
//!
//! # Modules
//! - [`welford`] — Single-pass mean/variance/CDF (O(1) space).
//! - [`vpin`]  — VPIN via volume-bucket approach, with CDF-based toxicity scoring.
//! - [`ofi`]   — 5-level order flow imbalance and buy/sell direction signal.
//! 参考: 量价时空/Phase-2-派发提示词.md:795 — R-2-502 — taiji-orderflow ComputeNode

pub mod ofi;
pub mod orderflow;
pub mod vpin;
pub mod welford;

pub use ofi::OfiNode;
pub use orderflow::OrderFlowNode;
pub use vpin::VpinNode;
pub use welford::WelfordStats;
