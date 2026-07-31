//! BarNode — ComputeNode 适配层。
//!
//! 将 BarGenerator 包装为 ComputeNode trait，可注册到 Pipeline。
//! 参考: taiji-engine/pipeline/bar_gen.rs BarNode 模式
//! 参考: 量价时空/Phase-2-派发提示词.md:429 — R-2-203 — BarGenerator tick→K线

use std::sync::Arc;

use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar, Symbol};
use taiji_engine::types::state::{StateKey, StateValue};
use taiji_engine::types::tick::TickData;

use crate::bargen::BarGenerator;
use crate::modes::AggMode;

/// Bar 生成节点。
///
/// 实现 `ComputeNode`，通过 `on_tick` 接收逐笔 tick，按时间边界聚合为 `RawBar`，
/// 写入 `StateStore`（key = `"bars:{freq_key}"`，如 `"bars:1m"`）。
///
/// 内部委托给 `BarGenerator` 做实际的 tick→bar 聚合。
pub struct BarNode {
    id: NodeId,
    freq: Freq,
    generator: Option<BarGenerator>,
}

impl BarNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            freq: Freq::F1,
            generator: None,
        }
    }

    fn output_key(&self) -> StateKey {
        format!("bars:{}", self.freq.freq_key())
    }
}

impl ComputeNode for BarNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "BarNode"
    }

    fn input_keys(&self) -> Vec<StateKey> {
        vec![]
    }

    fn output_keys(&self) -> Vec<StateKey> {
        vec![self.output_key()]
    }

    fn on_init(&mut self, config: &NodeConfig, _state: &StateStore) -> Result<()> {
        if let Some(freq_str) = config.get_str("freq") {
            self.freq = Freq::from_key(freq_str).unwrap_or(Freq::F1);
        }
        Ok(())
    }

    fn on_tick(&mut self, tick: &TickData, state: &StateStore) -> Result<()> {
        if self.generator.is_none() {
            let symbol = Symbol::from(tick.instrument.as_str());
            self.generator = Some(BarGenerator::new(
                symbol,
                vec![AggMode::Time],
                vec![self.freq],
            ));
        }

        let bg = self.generator.as_mut().unwrap();
        let closed = bg.update_tick(tick);

        for (_freq, bar) in &closed {
            let key = self.output_key();
            let bars: Arc<Vec<Arc<RawBar>>> =
                state.get(&key).unwrap_or_else(|| Arc::new(Vec::new()));
            let mut new_bars: Vec<Arc<RawBar>> = (*bars).clone();
            new_bars.push(Arc::new(bar.clone()));
            state.set(key, StateValue::Bars(Arc::new(new_bars)), self.id());
        }

        Ok(())
    }

    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
        Ok(())
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![self.freq]
    }
}
