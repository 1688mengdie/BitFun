//! 修士核心 trait — AgentTrait 定义 + Agent 实现。

use async_trait::async_trait;
use futures::stream::BoxStream;
use taiji_types::agent::{AgentConfig, AgentId, AgentState, AgentStatus, SpiritRoot};
use taiji_types::message::Message;
use taiji_types::realm::Realm;

use crate::error::AgentSystemError;
use crate::event::AgentEvent;

/// 修士核心 trait — 定义 Agent 的完整生命周期。
///
/// 参考: AgentScope (Apache 2.0) agent/_agent.py:250-341 Agent 生命周期
///       BitFun agent-runtime (MIT) session.rs Agent 核心方法
#[async_trait]
pub trait AgentTrait: Send + Sync {
    /// 道号 — 唯一标识。
    fn agent_id(&self) -> &AgentId;

    /// 灵根 — 职业方向，不可更换。
    fn spirit_root(&self) -> SpiritRoot;

    /// 境界 — 当前修为等级。
    fn realm(&self) -> Realm;

    /// 当前状态。
    fn status(&self) -> AgentStatus;

    /// 配置。
    fn config(&self) -> &AgentConfig;

    /// 状态快照。
    fn state(&self) -> &AgentState;

    /// 回复入口（LVPA: 修士出手）— 接收输入，返回最终回复。
    async fn reply(&mut self, input: &str) -> Result<String, AgentSystemError> {
        let _ = input;
        Err(AgentSystemError::Unimplemented("reply not implemented".into()))
    }

    /// 流式回复（LVPA: 修士出手·流式）— 接收输入，产出事件流。
    async fn reply_stream(
        &mut self,
        input: &str,
    ) -> Result<BoxStream<'_, AgentEvent>, AgentSystemError> {
        let _ = input;
        Err(AgentSystemError::Unimplemented("reply_stream not implemented".into()))
    }

    /// 观察（LVPA: 修士感知）— 接收外部消息。
    async fn observe(&mut self, msg: Message) -> Result<(), AgentSystemError> {
        let _ = msg;
        Ok(())
    }

    /// 压缩上下文（LVPA: 记忆结晶）— 将上下文压缩为摘要。
    async fn compress_context(&mut self) -> Result<(), AgentSystemError> {
        Ok(())
    }

    /// 转世重生（LVPA: 重置到某历史点）— 重置状态到指定 commit。
    async fn reincarnate(&mut self, target_commit: &str) -> Result<(), AgentSystemError> {
        let _ = target_commit;
        Err(AgentSystemError::Unimplemented("reincarnate not implemented".into()))
    }

    /// 身外化身（fork 生子 Agent）— 创建继承本命魂卡的子 Agent。
    async fn fork(&self, child_name: &str, spirit_root: SpiritRoot) -> Result<Box<dyn AgentTrait>, AgentSystemError> {
        let _ = (child_name, spirit_root);
        Err(AgentSystemError::Unimplemented("fork not implemented".into()))
    }
}
