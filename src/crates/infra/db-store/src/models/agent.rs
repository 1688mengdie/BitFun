//! Agent（修士）数据模型
//!
//! 来源: modules/db-store/接口设计.md:214-231 — Agent struct
//! 来源: 架构总纲 §5.1 — 修士系统

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Agent（修士）数据模型
///
/// 对应 `agents` 表。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// UUID v7，道号
    pub id: String,
    /// 道号名称
    pub name: String,
    /// 灵根（gold/wood/water/fire/earth）
    pub class: String,
    /// 境界（qi_refining ~ ascension）
    pub realm: String,
    /// 评分/贡献（0~100）
    pub credit: f64,
    /// 灵石（整数分，单位：分）
    pub spirit_stones: i64,
    /// 扩展字段
    pub metadata: HashMap<String, Value>,
    /// 创建时间（ISO 8601 UTC）
    pub created_at: String,
    /// 更新时间（ISO 8601 UTC）
    pub updated_at: String,
}
