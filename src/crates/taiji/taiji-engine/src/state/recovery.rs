//! 快照恢复 — 启动时自动恢复最近快照
//!
//! 提供 `StateRecovery` 在引擎启动时从持久化快照恢复 StateStore。
//! 支持崩溃恢复（crash recovery）和冷启动两种模式。
//!
//! 来源: Phase-2-派发提示词.md:557-560 — R-2-204-03 快照落盘 + 崩溃恢复
//! 参考: 量价时空/Phase-2-派发提示词.md:527 — R-2-204 — StateStore + 快照恢复

use std::path::PathBuf;

use crate::error::Result;
use crate::state::snapshot::SnapshotManager;
use crate::store::StateStore;

/// 快照恢复模式
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum RecoveryMode {
    /// 冷启动：不恢复快照，从空 StateStore 开始
    ColdStart,
    /// 崩溃恢复：尝试恢复最新快照，失败时静默降级为空 StateStore
    #[default]
    CrashRecovery,
    /// 强制恢复：必须从快照恢复，无快照时返回错误
    Strict,
}

/// 快照恢复结果
pub struct RecoveryResult {
    /// 恢复后的 StateStore
    pub store: StateStore,
    /// 是否从快照恢复
    pub from_snapshot: bool,
    /// 快照版本（如果已恢复）
    pub version: Option<String>,
}

impl std::fmt::Debug for RecoveryResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecoveryResult")
            .field("from_snapshot", &self.from_snapshot)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

/// 状态恢复器 — 从快照目录恢复 StateStore
///
/// 启动时自动加载最近快照。支持的恢复模式：
/// - `ColdStart`: 空 StateStore（不加载快照）
/// - `CrashRecovery`: 尝试加载，失败或不存在时返回空 StateStore
/// - `Strict`: 必须从快照恢复，否则返回错误
pub struct StateRecovery {
    snapshot_dir: PathBuf,
    mode: RecoveryMode,
}

impl StateRecovery {
    /// 创建状态恢复器
    pub fn new(snapshot_dir: PathBuf, mode: RecoveryMode) -> Self {
        Self { snapshot_dir, mode }
    }

    /// 使用默认恢复模式创建（CrashRecovery）
    pub fn with_default(snapshot_dir: PathBuf) -> Self {
        Self::new(snapshot_dir, RecoveryMode::default())
    }

    /// 执行恢复，返回恢复结果
    pub fn recover(&self) -> Result<RecoveryResult> {
        match self.mode {
            RecoveryMode::ColdStart => Ok(RecoveryResult {
                store: StateStore::new(),
                from_snapshot: false,
                version: None,
            }),
            RecoveryMode::CrashRecovery => {
                match self.try_load_snapshot() {
                    Ok(Some((store, version))) => {
                        tracing::info!(
                            "StateRecovery: 从快照 '{}' 恢复 StateStore 成功",
                            version
                        );
                        Ok(RecoveryResult {
                            store,
                            from_snapshot: true,
                            version: Some(version),
                        })
                    }
                    Ok(None) => {
                        tracing::info!("StateRecovery: 无可恢复快照，冷启动");
                        Ok(RecoveryResult {
                            store: StateStore::new(),
                            from_snapshot: false,
                            version: None,
                        })
                    }
                    Err(e) => {
                        tracing::warn!(
                            "StateRecovery: 快照恢复失败 ({}), 降级为冷启动",
                            e
                        );
                        Ok(RecoveryResult {
                            store: StateStore::new(),
                            from_snapshot: false,
                            version: None,
                        })
                    }
                }
            }
            RecoveryMode::Strict => {
                match self.try_load_snapshot()? {
                    Some((store, version)) => {
                        tracing::info!(
                            "StateRecovery: 严格模式从快照 '{}' 恢复成功",
                            version
                        );
                        Ok(RecoveryResult {
                            store,
                            from_snapshot: true,
                            version: Some(version),
                        })
                    }
                    None => Err(crate::error::TaijiError::KeyNotFound(
                        "snapshot".into()
                    )),
                }
            }
        }
    }

    /// 尝试从最新快照加载 StateStore
    fn try_load_snapshot(&self) -> Result<Option<(StateStore, String)>> {
        let mgr = SnapshotManager::new(self.snapshot_dir.clone());

        // 列出快照文件获取最新版本名
        let snapshots = mgr.list_snapshots()?;
        let version = snapshots.first().and_then(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        });

        match mgr.load_latest_snapshot()? {
            Some(store) => Ok(Some((store, version.unwrap_or_else(|| "unknown".into())))),
            None => Ok(None),
        }
    }

    /// 获取当前恢复模式
    pub fn mode(&self) -> RecoveryMode {
        self.mode
    }

    /// 获取快照目录路径
    pub fn snapshot_dir(&self) -> &PathBuf {
        &self.snapshot_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::snapshot::SnapshotManager;
    use crate::store::StateStore;
    use crate::types::state::StateValue;
    use std::env;

    fn setup_test_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("taiji_test_recovery_{}", name));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn test_cold_start() {
        let dir = setup_test_dir("cold_start");
        let recovery = StateRecovery::new(dir.clone(), RecoveryMode::ColdStart);
        let result = recovery.recover().unwrap();
        assert!(!result.from_snapshot, "冷启动不应从快照恢复");
        assert!(result.version.is_none());
        // 验证是空 StateStore
        let val: Option<bool> = result.store.get(&"any_key".into());
        assert_eq!(val, None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_crash_recovery_no_snapshot() {
        let dir = setup_test_dir("crash_no_snapshot");
        let recovery = StateRecovery::with_default(dir.clone());
        let result = recovery.recover().unwrap();
        assert!(!result.from_snapshot, "无快照时应为冷启动");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_crash_recovery_with_snapshot() {
        let dir = setup_test_dir("crash_with_snapshot");
        // 先创建快照
        let mgr = SnapshotManager::new(dir.clone());
        let store = StateStore::new();
        store.set("k1".into(), StateValue::F64(1.618), "node_a".into());
        store.set("k2".into(), StateValue::Bool(true), "node_b".into());
        mgr.save_snapshot(&store, "test_v1").unwrap();

        // 执行恢复
        let recovery = StateRecovery::with_default(dir.clone());
        let result = recovery.recover().unwrap();
        assert!(result.from_snapshot, "应从快照恢复");
        assert!(result.version.is_some(), "应有版本号");

        // 验证数据
        let val_f64: Option<f64> = result.store.get(&"k1".into());
        assert!((val_f64.unwrap_or(0.0) - 1.618).abs() < 1e-10, "f64 值应恢复");
        let val_bool: Option<bool> = result.store.get(&"k2".into());
        assert_eq!(val_bool, Some(true), "bool 值应恢复");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_strict_mode_no_snapshot() {
        let dir = setup_test_dir("strict_no_snapshot");
        let recovery = StateRecovery::new(dir.clone(), RecoveryMode::Strict);
        let result = recovery.recover();
        assert!(result.is_err(), "严格模式无快照应返回错误");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_strict_mode_with_snapshot() {
        let dir = setup_test_dir("strict_with_snapshot");
        let mgr = SnapshotManager::new(dir.clone());
        let store = StateStore::new();
        store.set("k".into(), StateValue::Usize(100), "node".into());
        mgr.save_snapshot(&store, "v1").unwrap();

        let recovery = StateRecovery::new(dir.clone(), RecoveryMode::Strict);
        let result = recovery.recover().unwrap();
        assert!(result.from_snapshot, "严格模式应从快照恢复");
        let val: Option<usize> = result.store.get(&"k".into());
        assert_eq!(val, Some(100));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_recovery_roundtrip_preserves_all_types() {
        let dir = setup_test_dir("recovery_all_types");
        let mgr = SnapshotManager::new(dir.clone());
        let store = StateStore::new();
        store.set("b".into(), StateValue::Bool(true), "n1".into());
        store.set("f".into(), StateValue::F64(2.71828), "n2".into());
        store.set("u".into(), StateValue::Usize(999), "n3".into());
        mgr.save_snapshot(&store, "v1").unwrap();

        let recovery = StateRecovery::with_default(dir.clone());
        let result = recovery.recover().unwrap();

        let b: Option<bool> = result.store.get(&"b".into());
        assert_eq!(b, Some(true));
        let f: Option<f64> = result.store.get(&"f".into());
        assert!((f.unwrap_or(0.0) - 2.71828).abs() < 1e-10);
        let u: Option<usize> = result.store.get(&"u".into());
        assert_eq!(u, Some(999));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_recovery_mode_default() {
        assert_eq!(RecoveryMode::default(), RecoveryMode::CrashRecovery);
    }
}
