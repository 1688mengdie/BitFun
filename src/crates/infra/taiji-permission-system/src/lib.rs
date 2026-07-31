//! taiji-permission-system — LVPA 灵根系统（权限管理面）。
//!
//! 架构总纲 §5.2 管理面/数据面分离：
//!
//! - **管理面（本 crate）**：灵根白名单定义、灵石配额设置、称号特权管理、境界→工具映射、收费 tier 映射
//! - **数据面（harness）**：运行时门禁执行，从本 crate 预加载配置
//! - **边界服务（gateway/ledger）**：认证入口 + 审计日志
//!
//! # 模块
//!
//! - [`manager`] — PermissionConfigManager trait + InMemoryPermissionManager + ProductTier/resolve_tier
//! - [`error`] — PermissionSystemError

mod error;
mod manager;

pub use error::PermissionSystemError;
pub use manager::{
    can_access_workshop, resolve_tier, InMemoryPermissionManager, PermissionConfigManager,
    ProductTier,
};
