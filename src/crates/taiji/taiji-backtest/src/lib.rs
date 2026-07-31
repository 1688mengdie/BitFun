//! taiji-backtest — Backtest engine (MIT)
//!
//! Provides:
//! - [`BacktestRunner`]: main backtest loop (CSV replay → Pipeline → signal matching → stats)
//! - [`PerformanceStats`]: 8-metric performance analysis (Sharpe, MaxDD, WinRate, etc.)
//! - [`WalkForwardValidator`]: walk-forward cross-validation with configurable folds
//! - [`TradeRecord`]: individual trade tracking with PnL computation
//! - [`BacktestConfig`]: YAML-driven backtest configuration
//! 参考: 量价时空/Phase-2-派发提示词.md:867 — R-2-505 — taiji-backtest 回测引擎

pub mod config;
pub mod runner;
pub mod stats;
pub mod trade_record;
pub mod walk_forward;

pub use config::{BacktestConfig, DateRange, WalkForwardConfig};
pub use runner::{BacktestResult, BacktestRunner};
pub use stats::PerformanceStats;
pub use trade_record::{Direction, TradeRecord};
pub use walk_forward::{WalkForwardReport, WalkForwardValidator};
