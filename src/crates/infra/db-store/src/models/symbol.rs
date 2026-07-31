//! 合约/品种元数据
//!
//! 来源: modules/db-store/接口设计.md:257-281 — SymbolInfo
//! 来源: lsp-index 代码智能模块需求

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// 合约/交易品种元数据
///
/// v2.4 新增。统一存储所有可交易品种的定义信息。
/// 对应 `symbols` 表。
///
/// 来源: modules/db-store/接口设计.md:264-281 — SymbolInfo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    /// 合约代码
    pub symbol: String,
    /// 交易所
    pub exchange: String,
    /// 品种组（如 "RB"、"IF"）
    pub product_group: String,
    /// 中文名称
    pub name_cn: String,
    /// 英文名称
    pub name_en: String,
    /// 合约乘数
    pub contract_multiplier: f64,
    /// 最小变动价位
    pub price_tick: f64,
    /// 保证金比例
    pub margin_rate: Option<f64>,
    /// 上市日
    pub listing_date: Option<String>,
    /// 交割日
    pub delivery_date: Option<String>,
    /// 是否活跃交易中
    pub is_active: bool,
    /// 扩展字段
    pub metadata: HashMap<String, Value>,
    /// 创建时间（ISO 8601 UTC）
    pub created_at: String,
    /// 更新时间（ISO 8601 UTC）
    pub updated_at: String,
}
