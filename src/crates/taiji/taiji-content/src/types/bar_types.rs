use chrono::{DateTime, Utc};

/// K-line bar data (minimal subset for rendering).
/// Mirrors taiji_engine::types::bar::RawBar fields needed by KLineRenderer.
#[derive(Debug, Clone)]
pub struct RawBar {
    pub symbol: String,
    pub dt: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub vol: f64,
}
