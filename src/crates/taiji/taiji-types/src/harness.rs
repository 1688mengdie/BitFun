//! 护山大阵核心类型 — Harness trait + 门令体系。
//!
//! 定义运行时权限门控的核心接口，供 agent-system、gateway、ledger 等模块实现。
//!
//! 参考: modules/harness/接口设计.md §2-3（v2.4 定义）
//!       agentscope permission/ 体系

use serde::{Deserialize, Serialize};

use crate::permission::{GateCommand, GuardLevel, PermissionRule};

/// 护山大阵核心 trait — 运行时权限门控。
///
/// 定义在共享类型层，由 harness crate 实现具体逻辑。
/// 各模块通过此 trait 进行权限检查，不直接依赖 harness 实现。
pub trait Harness: Send + Sync {
    /// 执行权限检查 — 裁决工具调用是否允许。
    fn check(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
    ) -> impl std::future::Future<Output = GateCommand> + Send;

    /// 添加门规 — 注册一条权限规则。
    fn add_rule(&mut self, rule: PermissionRule);

    /// 获取当前戒备等级。
    fn guard_level(&self) -> GuardLevel;

    /// 设置戒备等级。
    fn set_guard_level(&mut self, mode: GuardLevel);
}

/// Harness 配置 — 护山大阵初始化参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessConfig {
    /// 初始戒备等级。
    pub guard_level: GuardLevel,
    /// 预设门规列表。
    pub rules: Vec<PermissionRule>,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            guard_level: GuardLevel::Default,
            rules: Vec::new(),
        }
    }
}
