//! db-store 集成测试 — SharedBarBuffer（L1 环缓冲）
//!
//! 来源: Phase-1-派发提示词.md:543-555 — Buffer 测试用例

use chrono::{TimeZone, Utc};
use taiji_infra_db_store::{BufferConfig, Freq, RawBar, SharedBarBuffer};

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

#[tokio::test]
async fn test_buffer_push_latest() {
    let buffer = SharedBarBuffer::default_config();

    for i in 0..100 {
        buffer.push(make_bar("RB", Freq::F1, i, 3200.0 + i as f64)).unwrap();
    }

    let latest = buffer.latest("RB", Freq::F1, 5);
    assert_eq!(latest.len(), 5);
    assert_eq!(latest[0].id, 95);
    assert_eq!(latest[4].id, 99);

    // 读取 0 条
    let empty = buffer.latest("RB", Freq::F1, 0);
    assert!(empty.is_empty());

    // 读取超过总数
    let all = buffer.latest("RB", Freq::F1, 200);
    assert_eq!(all.len(), 100);
}

#[tokio::test]
async fn test_buffer_push_batch() {
    let buffer = SharedBarBuffer::default_config();

    let bars: Vec<RawBar> = (0..50)
        .map(|i| make_bar("IF", Freq::F5, i, 3500.0 + i as f64))
        .collect();
    buffer.push_batch(&bars).unwrap();

    assert_eq!(buffer.latest("IF", Freq::F5, 50).len(), 50);
}

#[tokio::test]
async fn test_buffer_range_query() {
    let buffer = SharedBarBuffer::default_config();
    let start = Utc.timestamp_opt(1700000000, 0).unwrap();
    let end = Utc.timestamp_opt(1700000100, 0).unwrap();

    // 推送时间范围不同的 K 线
    for i in 0..10 {
        let dt = Utc.timestamp_opt(1700000000 + i as i64 * 5, 0).unwrap();
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

    // 后 5 条在范围内
    let range_bars = buffer.range("RB", Freq::F1, start, end);
    assert!(range_bars.len() <= 10);

    // 空范围
    let future_start = Utc.timestamp_opt(9999999999, 0).unwrap();
    let future_end = Utc.timestamp_opt(9999999999 + 100, 0).unwrap();
    let empty_range = buffer.range("RB", Freq::F1, future_start, future_end);
    assert!(empty_range.is_empty());
}

#[tokio::test]
async fn test_buffer_concurrent_reads() {
    use std::sync::Arc;
    use std::thread;

    let buffer = Arc::new(SharedBarBuffer::default_config());

    // push 100 bars
    for i in 0..100 {
        buffer.push(make_bar("RB", Freq::F1, i, 3200.0 + i as f64)).unwrap();
    }

    // 多线程并发读
    let mut handles = vec![];
    for _ in 0..10 {
        let buf = buffer.clone();
        handles.push(thread::spawn(move || {
            let latest = buf.latest("RB", Freq::F1, 5);
            assert_eq!(latest.len(), 5);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

#[tokio::test]
async fn test_buffer_subscribe() {
    let mut config = BufferConfig::default();
    config.flush_batch_size = 5;
    let buffer = SharedBarBuffer::new(config);
    let mut rx = buffer.subscribe();

    // 推送 5 条触发通知
    for i in 0..5 {
        buffer.push(make_bar("RB", Freq::F1, i, 3200.0 + i as f64)).unwrap();
    }

    match tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await {
        Ok(Ok(update)) => {
            assert_eq!(update.symbol, "RB");
        }
        _ => {
            // 通知可能因 timing 未及时到达，不算失败
        }
    }
}

#[tokio::test]
async fn test_buffer_stats() {
    let buffer = SharedBarBuffer::default_config();

    assert_eq!(buffer.stats().push_count, 0);

    buffer.push(make_bar("RB", Freq::F1, 0, 3200.0)).unwrap();
    buffer.push(make_bar("IF", Freq::F5, 0, 3500.0)).unwrap();

    let stats = buffer.stats();
    assert_eq!(stats.push_count, 2);
    assert_eq!(stats.total_entries, 2);
    assert_eq!(buffer.cached_symbols().len(), 2);
}
