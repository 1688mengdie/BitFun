//! 天眼阵 — 健康检查引擎。
//!
//! 模块健康状态管理：注册、心跳续期、超时检测、注销。
//!
//! 设计参考：BitFun `crates/assembly/core/src/service/config/service.rs:36-45`
//! ConfigHealthStatus 健康检查模式，Rust 扩展为全系统健康报告，非 Cargo 依赖。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// 模块健康状态枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleHealth {
    /// 正常运行。
    Healthy,
    /// 部分功能降级。
    Degraded,
    /// 不可用。
    Down,
}

impl ModuleHealth {
    /// 返回健康等级数值（Healthy=2, Degraded=1, Down=0）。
    pub fn as_score(&self) -> u8 {
        match self {
            ModuleHealth::Healthy => 2,
            ModuleHealth::Degraded => 1,
            ModuleHealth::Down => 0,
        }
    }
}

/// 单个模块健康报告。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleHealthReport {
    /// 模块名称。
    pub module_name: String,
    /// 健康状态。
    pub health: ModuleHealth,
    /// 最后心跳时间。
    pub last_heartbeat: Option<chrono::DateTime<chrono::Utc>>,
    /// 状态消息。
    pub message: String,
}

/// 系统健康检查报告。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthReport {
    /// 整体健康状态。
    pub overall: ModuleHealth,
    /// 各模块健康报告。
    pub modules: HashMap<String, ModuleHealthReport>,
    /// 报告生成时间。
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 警告列表。
    pub warnings: Vec<String>,
    /// 错误列表。
    pub errors: Vec<String>,
}

/// 心跳超时阈值（秒）。
const HEARTBEAT_TIMEOUT_SECS: u64 = 30;
/// 降级阈值（秒，超过此时间未心跳标记为 Degraded）。
const HEARTBEAT_DEGRADED_SECS: u64 = 15;

/// 模块心跳内部状态。
#[derive(Debug)]
pub(crate) struct ModuleState {
    pub name: String,
    pub last_heartbeat: Instant,
}

/// 模块心跳句柄——drop 时自动注销。
#[derive(Debug)]
pub struct ModuleHandle {
    pub(crate) module_name: String,
    pub(crate) alive: Arc<AtomicBool>,
}

impl ModuleHandle {
    /// 创建模块句柄。
    pub fn new(name: impl Into<String>) -> (Self, Arc<AtomicBool>) {
        let alive = Arc::new(AtomicBool::new(true));
        let handle = Self {
            module_name: name.into(),
            alive: alive.clone(),
        };
        (handle, alive)
    }

    /// 获取模块名称。
    pub fn name(&self) -> &str {
        &self.module_name
    }
}

impl Drop for ModuleHandle {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::SeqCst);
    }
}

/// 模块句柄条目。
type HandleEntry = (String, Arc<AtomicBool>);

/// 健康检查引擎。
#[derive(Debug)]
pub struct HealthEngine {
    modules: Arc<Mutex<HashMap<String, ModuleState>>>,
    handles: Arc<Mutex<Vec<HandleEntry>>>,
}

impl HealthEngine {
    /// 创建健康检查引擎。
    pub fn new() -> Self {
        Self {
            modules: Arc::new(std::sync::Mutex::new(HashMap::new())),
            handles: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// 注册模块并返回 ModuleHandle（drop 时自动注销）。
    pub fn register_module(&self, name: &str) -> ModuleHandle {
        let (handle, alive) = ModuleHandle::new(name);
        let mut modules = self.modules.lock().unwrap_or_else(|e| e.into_inner());
        modules.insert(name.to_string(), ModuleState {
            name: name.to_string(),
            last_heartbeat: Instant::now(),
        });
        let mut handles = self.handles.lock().unwrap_or_else(|e| e.into_inner());
        handles.push((name.to_string(), alive));
        handle
    }

    /// 模块心跳续期。
    pub fn heartbeat(&self, name: &str) {
        if let Ok(mut modules) = self.modules.lock() {
            if let Some(state) = modules.get_mut(name) {
                state.last_heartbeat = Instant::now();
            }
        }
    }

    /// 生成全系统健康报告。
    pub fn health_report(&self) -> SystemHealthReport {
        // 先清理已注销的模块
        self.purge_dead();

        let mut modules = self.modules.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let mut reports = HashMap::new();
        let mut overall = ModuleHealth::Healthy;
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        // 收集已注销的名称
        let dead_names: Vec<String> = {
            let handles = self.handles.lock().unwrap_or_else(|e| e.into_inner());
            handles.iter()
                .filter(|(_, alive)| !alive.load(Ordering::SeqCst))
                .map(|(name, _)| name.clone())
                .collect()
        };

        // 移除已注销的模块
        for name in &dead_names {
            modules.remove(name);
        }

        for (_name, state) in modules.iter() {
            let elapsed = now.duration_since(state.last_heartbeat);
            let (health, msg) = if elapsed > Duration::from_secs(HEARTBEAT_TIMEOUT_SECS) {
                overall = ModuleHealth::Down;
                (ModuleHealth::Down, format!("心跳超时 ({:.0}s)", elapsed.as_secs_f64()))
            } else if elapsed > Duration::from_secs(HEARTBEAT_DEGRADED_SECS) {
                if overall != ModuleHealth::Down {
                    overall = ModuleHealth::Degraded;
                }
                (ModuleHealth::Degraded, format!("心跳延迟 ({:.0}s)", elapsed.as_secs_f64()))
            } else {
                (ModuleHealth::Healthy, "正常运行".to_string())
            };

            let report = ModuleHealthReport {
                module_name: state.name.clone(),
                health,
                last_heartbeat: Some(chrono::Utc::now()),
                message: msg,
            };
            reports.insert(state.name.clone(), report);
        }

        if overall == ModuleHealth::Down {
            errors.push("存在离线模块".to_string());
        } else if overall == ModuleHealth::Degraded {
            warnings.push("存在降级模块".to_string());
        }

        SystemHealthReport {
            overall,
            modules: reports,
            timestamp: chrono::Utc::now(),
            warnings,
            errors,
        }
    }

    /// 清理已注销的模块。
    fn purge_dead(&self) {
        let dead_names: Vec<String> = {
            let handles = self.handles.lock().unwrap_or_else(|e| e.into_inner());
            handles.iter()
                .filter(|(_, alive)| !alive.load(Ordering::SeqCst))
                .map(|(name, _)| name.clone())
                .collect()
        };
        if !dead_names.is_empty() {
            let mut modules = self.modules.lock().unwrap_or_else(|e| e.into_inner());
            for name in &dead_names {
                modules.remove(name);
            }
        }
    }
}

impl Default for HealthEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_healthy() {
        let engine = HealthEngine::new();
        let _handle = engine.register_module("test_module");
        let report = engine.health_report();
        assert_eq!(report.overall, ModuleHealth::Healthy);
        assert!(report.modules.contains_key("test_module"));
    }

    #[test]
    fn test_handle_drop_deregisters() {
        let engine = HealthEngine::new();
        let module_name = "temp_module";
        {
            let _handle = engine.register_module(module_name);
            let report = engine.health_report();
            assert!(report.modules.contains_key(module_name));
        }
        // handle dropped, module should be removed
        let report = engine.health_report();
        assert!(!report.modules.contains_key(module_name));
    }
}
