//! CtpDataSource — 一套代码覆盖 11 个柜台。
//!
//! 设计参考：openctp（BSD License, https://github.com/openctp/openctp）
//! CTP/XTP/EMT/TORA/TAP/QDP/FEMAS/IB/YD/TTS 共 11 个柜台共享同一套 CTPAPI 兼容接口。
//! 当前实现为骨架占位，运行时通过配置选择具体柜台类型。
//! 参考: 量价时空/Phase-2-派发提示词.md:316 — R-2-202 — DataSource 18源→4实现

use crate::error::{Result, TaijiError};
use crate::source::datasource::{DataSource, DataSourceConfig, FieldDef, SourceHealth};
use crate::types::tick::{RawTick, SourceId};

/// CTP 系列数据源。
///
/// 覆盖柜台：CTP / XTP / EMT / TORA / TAP / QDP / FEMAS / IB / YD / TTS
///
/// 所有柜台通过 openctp 统一 CTPAPI 兼容接口访问，
/// 差异仅在于配置参数（交易前置地址、行情前置地址、BrokerID、AppID 等）。
pub struct CtpDataSource {
    #[allow(dead_code)]
    source_id: SourceId,
    connected: bool,
    subscribed: Vec<String>,
    config: Option<DataSourceConfig>,
}

impl CtpDataSource {
    /// 创建 CTP 数据源。
    pub fn new(source_id: SourceId) -> Self {
        Self {
            source_id,
            connected: false,
            subscribed: Vec::new(),
            config: None,
        }
    }
}

impl DataSource for CtpDataSource {
    fn name(&self) -> &'static str {
        "ctp"
    }

    fn schema(&self) -> Vec<FieldDef> {
        vec![
            FieldDef { name: "last_price".into(), required: true },
            FieldDef { name: "volume".into(), required: true },
            FieldDef { name: "turnover".into(), required: true },
            FieldDef { name: "open_interest".into(), required: true },
            FieldDef { name: "bid_price1".into(), required: false },
            FieldDef { name: "ask_price1".into(), required: false },
        ]
    }

    fn connect(&mut self, config: &DataSourceConfig) -> Result<()> {
        // 检查柜台类型参数
        let _broker_type = config
            .params
            .get("broker_type")
            .and_then(|v| v.as_str())
            .unwrap_or("ctp");

        // TODO: Phase 3 实现 openctp CTPAPI 实际连接
        // 参考 openctp CTPAPI 兼容层：
        //   - CThostFtdcMdApi::CreateFtdcMdApi(address)
        //   - CThostFtdcMdSpi (OnRtnDepthMarketData callback)
        self.config = Some(config.clone());
        self.connected = true;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        self.subscribed.clear();
        self.config = None;
        Ok(())
    }

    fn subscribe(&mut self, instruments: &[&str]) -> Result<()> {
        if !self.connected {
            return Err(TaijiError::DataSource("CTP not connected".into()));
        }
        self.subscribed = instruments.iter().map(|s| s.to_string()).collect();
        // TODO: Phase 3 调用 CThostFtdcMdApi::SubscribeMarketData
        Ok(())
    }

    fn next_raw(&mut self) -> Result<Option<RawTick>> {
        if !self.connected {
            return Err(TaijiError::DataSource("CTP not connected".into()));
        }
        // TODO: Phase 3 从回调队列中取下一个 tick
        Err(TaijiError::Unimplemented(
            "CtpDataSource::next_raw — requires openctp CTPAPI runtime".into(),
        ))
    }

    fn health_check(&self) -> SourceHealth {
        if self.connected {
            SourceHealth::Healthy
        } else {
            SourceHealth::Down
        }
    }

    fn supports_resume(&self) -> bool {
        true
    }

    fn last_sequence(&self, _instrument: &str) -> Option<u64> {
        // TODO: Phase 3 从回调序列号追踪
        None
    }

    fn resume_from(&mut self, _instrument: &str, _seq: u64) -> Result<()> {
        // TODO: Phase 3 断线续传
        Ok(())
    }
}
