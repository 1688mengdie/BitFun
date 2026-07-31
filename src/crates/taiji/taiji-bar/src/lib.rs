//! Tick-to-KLine 聚合引擎 — BarNode 实现 ComputeNode。
//! 参考: czsc BarGenerator (Apache 2.0)
//! 参考: 量价时空/Phase-2-派发提示词.md:429 — R-2-203 — BarGenerator tick→K线

pub mod bargen;
pub mod composer;
pub mod modes;
pub mod node;

pub use bargen::BarGenerator;
pub use composer::BarComposer;
pub use modes::{AggMode, AggParams};
pub use node::BarNode;
