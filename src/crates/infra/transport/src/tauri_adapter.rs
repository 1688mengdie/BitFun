//! TauriTransportAdapter — 基于 Tauri Emitter 的桌面端传输适配器
//!
//! 封装 `tauri::Emitter` 标准 API，通过 `app_handle.emit()` 将消息推送到 Tauri WebView。
//!
//! # 设计原则
//! - feature gate 条件编译：仅在 `#[cfg(feature = "tauri")]` 下编译
//! - 封装 tauri::Emitter 标准 API，LVPA 从零适配（非 Cargo 依赖 tauri）
//! - 不引入 tauri 以外的 GUI 框架依赖
//!
//! # 参考来源
//! - H8: modules/transport/接口设计.md:58-75 — TauriTransportAdapter 定义
//! - H8: tauri::Emitter trait 标准 API

#[cfg(feature = "tauri")]
use crate::{TransportAdapter, TransportMessage};
#[cfg(feature = "tauri")]
use async_trait::async_trait;
#[cfg(feature = "tauri")]
use std::fmt;
#[cfg(feature = "tauri")]
use tauri::Emitter;

/// 基于 Tauri `Emitter` 的桌面端传输适配器。
///
/// 使用 `app_handle.emit(event_name, payload)` 将消息推送到 Tauri WebView 前端。
/// 仅适用于 Tauri 桌面端环境。
///
/// # Feature gate
/// 仅在 `feature = "tauri"` 启用时编译。
/// 不启用时，整个模块被编译器排除。
#[cfg(feature = "tauri")]
pub struct TauriTransportAdapter {
    /// Tauri 应用句柄，用于 emit 事件到 WebView
    app_handle: tauri::AppHandle,
}

#[cfg(feature = "tauri")]
impl fmt::Debug for TauriTransportAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TauriTransportAdapter")
            .field("app_handle", &self.app_handle)
            .finish()
    }
}

#[cfg(feature = "tauri")]
impl TauriTransportAdapter {
    /// 创建新的 TauriTransportAdapter。
    ///
    /// # 参数
    /// - `app_handle`: Tauri 应用句柄（通常从 `tauri::AppHandle` 或 `tauri::App` 获取）
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }
}

#[cfg(feature = "tauri")]
#[async_trait]
impl TransportAdapter for TauriTransportAdapter {
    async fn send(&self, msg: TransportMessage) -> anyhow::Result<()> {
        self.app_handle
            .emit(&msg.event_name, msg.payload)
            .map_err(|e| anyhow::anyhow!("Tauri emit failed: {}", e))?;
        Ok(())
    }
}

#[cfg(feature = "tauri")]
#[cfg(test)]
mod tests {
    use super::*;

    /// TauriTransportAdapter 编译检查测试。
    /// 注意：完整集成测试需要 Tauri 运行时环境。
    #[test]
    fn test_compile_check() {
        // 编译期验证：确保模块在 feature=tauri 下编译通过
        // 实际实例化需要 tauri::test::mock_app()，不在单元测试中执行
        assert!(true, "tauri feature enabled, module compiled successfully");
    }
}
