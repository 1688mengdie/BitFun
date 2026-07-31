//! Taiji trading engine — DAG-based compute pipeline.
//! Architecture: tick → BarGenerator → DAG (ComputeNode graph) → signals.
//! 参考: 量价时空/Phase-2-派发提示词.md:188 — R-2-201 — ComputeNode trait + Pipeline DAG

pub mod compliance;
pub mod config;
pub mod dag;
pub mod debate;
pub mod error;
pub mod factory;
pub mod fusion;
pub mod node;
pub mod pipeline;
pub mod risk;
pub mod safe_json;
pub mod signal;
pub mod source;
pub mod state;
pub mod store;
pub mod types;
