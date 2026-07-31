//! 参考: 量价时空/Phase-2-派发提示词.md:707 — R-2-206 — taiji-engine-py PyO3 绑定

use pyo3::prelude::*;

#[pyclass]
#[derive(Clone)]
pub struct TickDataPy {
    #[pyo3(get)]
    pub instrument: String,
    #[pyo3(get)]
    pub last_price: f64,
    #[pyo3(get)]
    pub open_price: f64,
    #[pyo3(get)]
    pub highest_price: f64,
    #[pyo3(get)]
    pub lowest_price: f64,
    #[pyo3(get)]
    pub volume: f64,
    #[pyo3(get)]
    pub open_interest: f64,
    #[pyo3(get)]
    pub timestamp_ms: i64,
}

#[pyclass]
#[derive(Clone)]
pub struct RawBarPy {
    #[pyo3(get)]
    pub symbol: String,
    #[pyo3(get)]
    pub open: f64,
    #[pyo3(get)]
    pub high: f64,
    #[pyo3(get)]
    pub low: f64,
    #[pyo3(get)]
    pub close: f64,
    #[pyo3(get)]
    pub vol: f64,
    #[pyo3(get)]
    pub amount: f64,
    #[pyo3(get)]
    pub open_interest: Option<f64>,
    #[pyo3(get)]
    pub delta: Option<f64>,
}

#[pyclass]
#[derive(Clone)]
pub struct SignalPy {
    #[pyo3(get)]
    pub instrument: String,
    #[pyo3(get)]
    pub action: String,
    #[pyo3(get)]
    pub entry: Option<f64>,
    #[pyo3(get)]
    pub stop_loss: Option<f64>,
    #[pyo3(get)]
    pub take_profit: Option<f64>,
    #[pyo3(get)]
    pub size: Option<f64>,
    #[pyo3(get)]
    pub confidence: f64,
    #[pyo3(get)]
    pub source: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_data_py_construction() {
        let tick = TickDataPy {
            instrument: "rb2501".into(),
            last_price: 3500.0,
            open_price: 3498.0,
            highest_price: 3510.0,
            lowest_price: 3495.0,
            volume: 1000.0,
            open_interest: 50000.0,
            timestamp_ms: 1700000000000,
        };
        assert_eq!(tick.instrument, "rb2501");
        assert!((tick.last_price - 3500.0).abs() < 1e-9);
        assert!((tick.highest_price - 3510.0).abs() < 1e-9);
        assert!((tick.volume - 1000.0).abs() < 1e-9);
        assert!((tick.open_interest - 50000.0).abs() < 1e-9);
        assert_eq!(tick.timestamp_ms, 1700000000000);
    }

    #[test]
    fn test_raw_bar_py_construction() {
        let bar = RawBarPy {
            symbol: "rb2501".into(),
            open: 3500.0,
            high: 3510.0,
            low: 3490.0,
            close: 3505.0,
            vol: 2000.0,
            amount: 7_000_000.0,
            open_interest: Some(50000.0),
            delta: Some(100.0),
        };
        assert_eq!(bar.symbol, "rb2501");
        assert!((bar.open - 3500.0).abs() < 1e-9);
        assert!((bar.high - 3510.0).abs() < 1e-9);
        assert!((bar.low - 3490.0).abs() < 1e-9);
        assert!((bar.close - 3505.0).abs() < 1e-9);
        assert!((bar.vol - 2000.0).abs() < 1e-9);
        assert_eq!(bar.open_interest, Some(50000.0));
        assert_eq!(bar.delta, Some(100.0));
    }

    #[test]
    fn test_raw_bar_py_option_fields_none() {
        let bar = RawBarPy {
            symbol: "test".into(),
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close: 0.0,
            vol: 0.0,
            amount: 0.0,
            open_interest: None,
            delta: None,
        };
        assert!(bar.open_interest.is_none());
        assert!(bar.delta.is_none());
    }

    #[test]
    fn test_signal_py_construction() {
        let sig = SignalPy {
            instrument: "rb2501".into(),
            action: "Long".into(),
            entry: Some(3500.0),
            stop_loss: Some(3480.0),
            take_profit: Some(3550.0),
            size: Some(10.0),
            confidence: 0.85,
            source: "ma_cross".into(),
        };
        assert_eq!(sig.instrument, "rb2501");
        assert_eq!(sig.action, "Long");
        assert_eq!(sig.entry, Some(3500.0));
        assert_eq!(sig.stop_loss, Some(3480.0));
        assert_eq!(sig.take_profit, Some(3550.0));
        assert_eq!(sig.size, Some(10.0));
        assert!((sig.confidence - 0.85).abs() < 1e-9);
        assert_eq!(sig.source, "ma_cross");
    }

    #[test]
    fn test_signal_py_default_values() {
        let sig = SignalPy {
            instrument: "test".into(),
            action: "Hold".into(),
            entry: None,
            stop_loss: None,
            take_profit: None,
            size: None,
            confidence: 0.0,
            source: "".into(),
        };
        assert!(sig.entry.is_none());
        assert!(sig.stop_loss.is_none());
        assert!(sig.take_profit.is_none());
        assert!(sig.size.is_none());
        assert!((sig.confidence - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_tick_data_py_clone() {
        let tick = TickDataPy {
            instrument: "rb2501".into(),
            last_price: 3500.0,
            open_price: 3498.0,
            highest_price: 3510.0,
            lowest_price: 3495.0,
            volume: 1000.0,
            open_interest: 50000.0,
            timestamp_ms: 1700000000000,
        };
        let cloned = tick.clone();
        assert_eq!(tick.instrument, cloned.instrument);
        assert!((cloned.last_price - 3500.0).abs() < 1e-9);
    }
}
