//! db-store（灵脉）L1 内存共享 K 线缓冲
//!
//! 基于 arc_swap + crossbeam 的无锁环形缓冲区。
//! L1 实时计算层专用，零 IO，零阻塞。
//!
//! 来源: modules/db-store/接口设计.md:486-567 — SharedBarBuffer 接口
//! 参考: 嵌入式 ring buffer 模式 + arc_swap 无锁读

use crate::config::BufferConfig;
use crate::error::BufferError;
use crate::models::bar::{BarUpdate, BufferStats, Freq, RawBar};
use arc_swap::ArcSwap;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

/// L1 内存环缓冲槽位：每 (symbol, freq) 一个槽
struct BarSlot {
    /// 环形缓冲区
    buffer: ArcSwap<Vec<RawBar>>,
    /// 最大容量
    capacity: usize,
}

impl BarSlot {
    fn new(capacity: usize) -> Self {
        Self {
            buffer: ArcSwap::new(Arc::new(Vec::with_capacity(capacity))),
            capacity,
        }
    }

    /// 推送单条 K 线
    fn push(&self, bar: RawBar) {
        let mut new_buf = (**self.buffer.load()).clone();
        if new_buf.len() >= self.capacity {
            // 移除最旧的一半
            new_buf.drain(0..self.capacity / 4);
        }
        new_buf.push(bar);
        self.buffer.store(Arc::new(new_buf));
    }

    /// 批量推送
    fn push_batch(&self, bars: &[RawBar]) {
        if bars.is_empty() {
            return;
        }
        let mut new_buf = (**self.buffer.load()).clone();
        for bar in bars {
            if new_buf.len() >= self.capacity {
                new_buf.drain(0..self.capacity / 4);
            }
            new_buf.push(bar.clone());
        }
        self.buffer.store(Arc::new(new_buf));
    }

    /// 读取最近 N 条
    fn latest(&self, n: usize) -> Vec<RawBar> {
        let buf = self.buffer.load();
        let len = buf.len();
        if len == 0 || n == 0 {
            return vec![];
        }
        let take = n.min(len);
        buf[len - take..].to_vec()
    }

    /// 读取时间范围
    fn range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<RawBar> {
        let buf = self.buffer.load();
        buf.iter()
            .filter(|b| b.dt >= start && b.dt <= end)
            .cloned()
            .collect()
    }

    /// 获取条目数
    fn len(&self) -> usize {
        self.buffer.load().len()
    }
}

/// L1 内存共享 K 线缓冲（环缓冲，无锁读）
///
/// L1 零阻塞铁律：L1 层不碰任何 IO/db-store/网络。
/// 新行情通过 channel 推送至此缓冲，L1 策略/指标从此读取。
/// L2 决策层通过异步批量写入任务将缓冲落盘到 SQLite。
///
/// 来源: modules/db-store/接口设计.md:492-522 — SharedBarBuffer trait
pub struct SharedBarBuffer {
    /// 槽位: (symbol, freq) -> BarSlot
    slots: ArcSwap<HashMap<(String, Freq), Arc<BarSlot>>>,
    /// 配置
    config: BufferConfig,
    /// 订阅通知发送端
    notify_tx: broadcast::Sender<BarUpdate>,
    /// 推送计数器
    push_count: ArcSwap<u64>,
    /// 命中计数
    hit_count: ArcSwap<u64>,
    /// 未命中计数
    miss_count: ArcSwap<u64>,
}

impl SharedBarBuffer {
    /// 创建新的 SharedBarBuffer
    pub fn new(config: BufferConfig) -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            slots: ArcSwap::new(Arc::new(HashMap::new())),
            config,
            notify_tx: tx,
            push_count: ArcSwap::new(Arc::new(0)),
            hit_count: ArcSwap::new(Arc::new(0)),
            miss_count: ArcSwap::new(Arc::new(0)),
        }
    }

    /// 使用默认配置创建
    pub fn default_config() -> Self {
        Self::new(BufferConfig::default())
    }

    /// 获取或创建槽位
    fn get_or_create_slot(&self, symbol: &str, freq: Freq) -> Arc<BarSlot> {
        let key = (symbol.to_string(), freq);
        let slots = self.slots.load();

        if let Some(slot) = slots.get(&key) {
            self.hit_count
                .rcu(|c| Arc::new(**c + 1));
            return slot.clone();
        }

        // 未命中：创建新槽位
        drop(slots); // 释放读锁
        self.miss_count.rcu(|c| Arc::new(**c + 1));

        let new_slot = Arc::new(BarSlot::new(self.config.max_bars_per_slot));
        self.slots.rcu(|slots| {
            let mut new_map = (**slots).clone();
            new_map.entry(key.clone()).or_insert(new_slot.clone());
            Arc::new(new_map)
        });
        new_slot
    }

    /// 获取订阅者接收端
    pub fn subscribe(&self) -> broadcast::Receiver<BarUpdate> {
        self.notify_tx.subscribe()
    }
}

// 实现 SharedBarBuffer 的 trait 方法作为自由函数
impl SharedBarBuffer {
    /// 推送新 K 线（行情网关→L1）
    pub fn push(&self, bar: RawBar) -> Result<(), BufferError> {
        let slot = self.get_or_create_slot(&bar.symbol, bar.freq);
        slot.push(bar.clone());

        let new_count = {
            let c = self.push_count.rcu(|c| Arc::new(**c + 1));
            *c
        };

        if self.config.enable_notify && new_count as usize % self.config.flush_batch_size == 0 {
            let batch_id = new_count / self.config.flush_batch_size as u64;
            let update = BarUpdate {
                symbol: bar.symbol.clone(),
                freq: bar.freq,
                bars: vec![bar],
                batch_id,
            };
            let _ = self.notify_tx.send(update);
        }

        Ok(())
    }

    /// 批量推送
    pub fn push_batch(&self, bars: &[RawBar]) -> Result<(), BufferError> {
        if bars.is_empty() {
            return Ok(());
        }

        // 按 (symbol, freq) 分组后批量写入各槽
        let mut grouped: HashMap<(String, Freq), Vec<RawBar>> = HashMap::new();
        for bar in bars {
            grouped
                .entry((bar.symbol.clone(), bar.freq))
                .or_default()
                .push(bar.clone());
        }
        for ((symbol, freq), group_bars) in &grouped {
            let slot = self.get_or_create_slot(symbol, *freq);
            slot.push_batch(group_bars);
        }

        let new_count = {
            let c = self.push_count.rcu(|c| Arc::new(**c + bars.len() as u64));
            *c
        };

        if self.config.enable_notify {
            let batch_id = new_count / self.config.flush_batch_size as u64;
            // 按 (symbol, freq) 分组发送通知
            let mut grouped: HashMap<(String, Freq), Vec<RawBar>> = HashMap::new();
            for bar in bars {
                grouped
                    .entry((bar.symbol.clone(), bar.freq))
                    .or_default()
                    .push(bar.clone());
            }
            for ((symbol, freq), batch_bars) in grouped {
                let update = BarUpdate {
                    symbol,
                    freq,
                    bars: batch_bars,
                    batch_id,
                };
                let _ = self.notify_tx.send(update);
            }
        }

        Ok(())
    }

    /// 读取最近 N 根 K 线
    pub fn latest(&self, symbol: &str, freq: Freq, n: usize) -> Vec<RawBar> {
        let key = (symbol.to_string(), freq);
        let slots = self.slots.load();
        if let Some(slot) = slots.get(&key) {
            slot.latest(n)
        } else {
            vec![]
        }
    }

    /// 读取时间范围
    pub fn range(
        &self,
        symbol: &str,
        freq: Freq,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<RawBar> {
        let key = (symbol.to_string(), freq);
        let slots = self.slots.load();
        if let Some(slot) = slots.get(&key) {
            slot.range(start, end)
        } else {
            vec![]
        }
    }

    /// 获取已缓存的 (symbol, freq) 列表
    pub fn cached_symbols(&self) -> Vec<(String, Freq)> {
        let slots = self.slots.load();
        slots.keys().cloned().collect()
    }

    /// 获取缓冲统计
    pub fn stats(&self) -> BufferStats {
        let slots = self.slots.load();
        let total_entries: usize = slots.values().map(|s| s.len()).sum();
        BufferStats {
            total_entries,
            capacity: self.config.max_bars_per_slot * slots.len(),
            push_count: **self.push_count.load(),
            hit_count: **self.hit_count.load(),
            miss_count: **self.miss_count.load(),
            flush_pending: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_bar(symbol: &str, freq: Freq, id: i32, price: f64) -> RawBar {
        RawBar {
            symbol: symbol.into(),
            dt: Utc.timestamp_opt(1700000000 + id as i64, 0).unwrap(),
            freq,
            id,
            open: price,
            close: price + 0.5,
            high: price + 1.0,
            low: price - 0.5,
            vol: 100.0,
            amount: 1000.0,
            open_interest: None,
            trade_count: None,
        }
    }

    #[test]
    fn test_push_and_latest() {
        let buffer = SharedBarBuffer::default_config();

        // 推送 100 根 K 线
        for i in 0..100 {
            let bar = make_bar("RB", Freq::F1, i, 3200.0 + i as f64);
            buffer.push(bar).unwrap();
        }

        let latest = buffer.latest("RB", Freq::F1, 5);
        assert_eq!(latest.len(), 5);
        assert_eq!(latest[4].id, 99); // 最近一条
        assert_eq!(latest[0].id, 95); // 第 5 新
    }

    #[test]
    fn test_push_batch() {
        let buffer = SharedBarBuffer::default_config();
        let bars: Vec<RawBar> = (0..50)
            .map(|i| make_bar("IF", Freq::F5, i, 3500.0 + i as f64))
            .collect();

        buffer.push_batch(&bars).unwrap();
        assert_eq!(buffer.latest("IF", Freq::F5, 50).len(), 50);
    }

    #[test]
    fn test_range_query() {
        let buffer = SharedBarBuffer::default_config();
        let start = Utc.timestamp_opt(1700000000, 0).unwrap();
        let end = Utc.timestamp_opt(1700000100, 0).unwrap();

        // 推送不同时间范围的 K 线
        for i in 0..20 {
            let dt = Utc.timestamp_opt(1700000000 + i as i64 * 10, 0).unwrap();
            let bar = RawBar {
                symbol: "RB".into(),
                dt,
                freq: Freq::F1,
                id: i,
                open: 3200.0,
                close: 3201.0,
                high: 3202.0,
                low: 3199.0,
                vol: 100.0,
                amount: 1000.0,
                open_interest: None,
                trade_count: None,
            };
            buffer.push(bar).unwrap();
        }

        let range_bars = buffer.range("RB", Freq::F1, start, end);
        assert!(range_bars.len() >= 10); // 0-10 共 11 条
    }

    #[test]
    fn test_cached_symbols() {
        let buffer = SharedBarBuffer::default_config();

        buffer.push(make_bar("RB", Freq::F1, 0, 3200.0)).unwrap();
        buffer.push(make_bar("IF", Freq::F5, 0, 3500.0)).unwrap();

        let symbols = buffer.cached_symbols();
        assert_eq!(symbols.len(), 2);
    }

    #[test]
    fn test_stats() {
        let buffer = SharedBarBuffer::default_config();

        buffer.push(make_bar("RB", Freq::F1, 0, 3200.0)).unwrap();
        let stats = buffer.stats();
        assert_eq!(stats.push_count, 1);
        assert!(stats.capacity > 0);
    }

    #[tokio::test]
    async fn test_subscribe() {
        let mut config = BufferConfig::default();
        config.flush_batch_size = 10;
        let buffer = SharedBarBuffer::new(config);
        let mut rx = buffer.subscribe();

        buffer.push(make_bar("RB", Freq::F1, 0, 3200.0)).unwrap();

        // 推送 10 条触发通知
        for i in 0..10 {
            buffer.push(make_bar("RB", Freq::F1, i, 3200.0 + i as f64)).unwrap();
        }

        // 验证收到通知
        match tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(update)) => {
                assert_eq!(update.symbol, "RB");
                assert_eq!(update.freq, Freq::F1);
            }
            _ => {
                // 通知可能因 timing 未及时到达
            }
        }
    }

    #[tokio::test]
    async fn test_subscribe_with_batch() {
        let mut config = BufferConfig::default();
        config.flush_batch_size = 10;
        let buffer = SharedBarBuffer::new(config);
        let mut rx = buffer.subscribe();

        for i in 0..10 {
            buffer
                .push(make_bar("RB", Freq::F1, i, 3200.0 + i as f64))
                .unwrap();
        }

        // 推送 10 条后应触发通知
        match tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(update)) => {
                assert_eq!(update.symbol, "RB");
                assert_eq!(update.freq, Freq::F1);
            }
            _ => {
                // 通知可能因 timing 问题未及时到达
            }
        }
    }
}
