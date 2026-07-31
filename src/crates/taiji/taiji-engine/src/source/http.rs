//! HttpDataSource — HTTP 数据源（QQ/SINA/AkShare）。
//!
//! 设计参考：akshare（MIT License, https://github.com/akfamily/akshare）
//! QQ / SINA / AkShare 三套 HTTP 行情 API，统一适配为 DataSource trait。
//! 当前实现为骨架占位，运行时通过配置选择具体 API 类型。
//! 参考: 量价时空/Phase-2-派发提示词.md:316 — R-2-202 — DataSource 18源→4实现

use crate::error::{Result, TaijiError};
use crate::source::datasource::{DataSource, DataSourceConfig, FieldDef, SourceHealth};
use crate::types::tick::{RawTick, SourceId};

/// HTTP 数据源类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpSourceType {
    /// 腾讯行情 API
    QQ,
    /// 新浪行情 API
    SINA,
    /// AkShare Python 桥接
    AkShare,
}

impl HttpSourceType {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "qq" => Self::QQ,
            "sina" => Self::SINA,
            "akshare" | "ak" => Self::AkShare,
            _ => Self::QQ,
        }
    }
}

/// HTTP 数据源。
///
/// 覆盖数据源：QQ / SINA / AkShare
/// 所有源通过 HTTP REST API 获取行情，差异仅在于 URL 格式和返回字段解析。
pub struct HttpDataSource {
    #[allow(dead_code)]
    source_id: SourceId,
    source_type: HttpSourceType,
    connected: bool,
    subscribed: Vec<String>,
}

impl HttpDataSource {
    /// 创建 HTTP 数据源。
    pub fn new(source_id: SourceId) -> Self {
        Self {
            source_id,
            source_type: HttpSourceType::QQ,
            connected: false,
            subscribed: Vec::new(),
        }
    }
}

impl DataSource for HttpDataSource {
    fn name(&self) -> &'static str {
        match self.source_type {
            HttpSourceType::QQ => "http_qq",
            HttpSourceType::SINA => "http_sina",
            HttpSourceType::AkShare => "http_akshare",
        }
    }

    fn schema(&self) -> Vec<FieldDef> {
        vec![
            FieldDef { name: "last_price".into(), required: true },
            FieldDef { name: "volume".into(), required: true },
            FieldDef { name: "turnover".into(), required: false },
            FieldDef { name: "open_interest".into(), required: false },
        ]
    }

    fn connect(&mut self, config: &DataSourceConfig) -> Result<()> {
        let type_str = config
            .params
            .get("source_type")
            .and_then(|v| v.as_str())
            .unwrap_or("qq");
        self.source_type = HttpSourceType::from_str(type_str);

        // TODO: Phase 3 实现 HTTP 连接建立
        // QQ:   http://qt.gtimg.cn/q={symbol}
        // SINA: http://hq.sinajs.cn/list={symbol}
        // AkShare: Python bridge via PyO3
        self.connected = true;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        self.subscribed.clear();
        Ok(())
    }

    fn subscribe(&mut self, instruments: &[&str]) -> Result<()> {
        if !self.connected {
            return Err(TaijiError::DataSource("HTTP source not connected".into()));
        }
        self.subscribed = instruments.iter().map(|s| s.to_string()).collect();
        Ok(())
    }

    fn next_raw(&mut self) -> Result<Option<RawTick>> {
        if !self.connected {
            return Err(TaijiError::DataSource("HTTP source not connected".into()));
        }
        // TODO: Phase 3 实现 HTTP 轮询或长连接行情获取
        Err(TaijiError::Unimplemented(
            "HttpDataSource::next_raw — requires HTTP client runtime".into(),
        ))
    }

    fn health_check(&self) -> SourceHealth {
        if self.connected {
            SourceHealth::Healthy
        } else {
            SourceHealth::Down
        }
    }
}
