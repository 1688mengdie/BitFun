//! # taiji-card-system — LVPA 卡片装备系统
//!
//! 提供卡牌定义→装备→词条→套装→境界锁定的完整卡片生命周期管理。
//!
//! ## 模块结构
//!
//! - `card_manager`: CardManager trait + 装备/境界校验引擎（Wave 1）
//! - `modifier_engine`: 词条修饰器引擎 + 属性聚合（Wave 1）
//! - `set_system`: 套装检测 + 套装属性加成（Wave 1）
//! - `title_manager`: 荣誉称号系统（称号激活/聚合/cap 上限）
//! - `repository`: 持久化 trait + 内存实现（Wave 2）
//! - `synthesis`: 卡片合成 + 分解引擎（Wave 3）

pub mod card_manager;
pub mod modifier_engine;
pub mod set_system;
pub mod title_manager;
pub mod repository;
pub mod synthesis;
