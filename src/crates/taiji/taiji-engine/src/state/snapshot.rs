//! 快照管理器 — StateStore ↔ JSON 持久化
//!
//! 提供 StateManager（文件系统快照）和 SnapshotManager（StateStore 序列化包装）。
//! 快照通过 serde_json 序列化/反序列化，保留最多 max_keep 个历史版本。
//!
//! 来源: Phase-2-派发提示词.md:557-560 — R-2-204-02 SnapshotManager
//! 参考: 量价时空/Phase-2-派发提示词.md:527 — R-2-204 — StateStore + 快照恢复

use std::fs;
use std::path::PathBuf;

use crate::error::Result;
use crate::store::StateStore;

/// 状态快照管理器（文件系统级）
///
/// 负责快照文件的保存、加载、列表和自动清理。
/// 通过 serde_json 序列化 StateStore，存储在 snapshot_dir 目录下。
pub struct StateManager {
    snapshot_dir: PathBuf,
    max_keep: usize,
}

impl StateManager {
    /// 创建快照管理器，自动创建快照目录
    pub fn new(snapshot_dir: PathBuf) -> Self {
        fs::create_dir_all(&snapshot_dir).ok();
        Self {
            snapshot_dir,
            max_keep: 10,
        }
    }

    /// 设置最大保留快照数
    pub fn with_max_keep(mut self, max: usize) -> Self {
        self.max_keep = max.max(1);
        self
    }

    /// 保存快照（序列化 StateStore 到 JSON 文件）
    pub fn save(&self, state_json: &str, version: &str) -> Result<()> {
        let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("snapshot_{}_v{}.json", ts, version);
        let path = self.snapshot_dir.join(&filename);
        fs::write(&path, state_json)?;
        self.cleanup()?;
        Ok(())
    }

    /// 列出所有快照文件（按时间倒序）
    pub fn list_snapshots(&self) -> Result<Vec<PathBuf>> {
        let mut entries: Vec<PathBuf> = Vec::new();
        if let Ok(dir) = fs::read_dir(&self.snapshot_dir) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json") {
                    entries.push(path);
                }
            }
        }
        entries.sort_by(|a, b| b.cmp(a)); // newest first
        Ok(entries)
    }

    /// 加载最近的快照（返回 JSON 字符串）
    pub fn load(&self) -> Result<Option<String>> {
        let snapshots = self.list_snapshots()?;
        if let Some(latest) = snapshots.first() {
            let content = fs::read_to_string(latest)?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    /// 清理旧快照（保留最近 N 个）
    fn cleanup(&self) -> Result<()> {
        let snapshots = self.list_snapshots()?;
        for old in snapshots.iter().skip(self.max_keep) {
            fs::remove_file(old)?;
        }
        Ok(())
    }
}

/// StateStore 快照管理器（高级接口）
///
/// 封装 StateManager，提供 StateStore 级别的序列化/反序列化。
/// `save_snapshot()` 将 StateStore 序列化为 JSON 并落盘。
/// `load_latest_snapshot()` 从最新快照恢复 StateStore。
pub struct SnapshotManager {
    inner: StateManager,
}

impl SnapshotManager {
    /// 创建 SnapshotManager，使用默认最大保留数
    pub fn new(snapshot_dir: PathBuf) -> Self {
        Self {
            inner: StateManager::new(snapshot_dir),
        }
    }

    /// 创建 SnapshotManager，指定最大保留快照数
    pub fn with_max_keep(snapshot_dir: PathBuf, max_keep: usize) -> Self {
        Self {
            inner: StateManager::new(snapshot_dir).with_max_keep(max_keep),
        }
    }

    /// 保存 StateStore 快照
    ///
    /// 将 StateStore 序列化为 JSON，通过 `to_json()` 获取完整状态。
    pub fn save_snapshot(&self, store: &StateStore, version: &str) -> Result<()> {
        let json = store.to_json().to_string();
        self.inner.save(&json, version)
    }

    /// 加载最新快照，反序列化为 StateStore
    ///
    /// 返回 Option<StateStore>：
    /// - `Some(store)` — 成功从最新快照恢复
    /// - `None` — 没有可用的快照
    pub fn load_latest_snapshot(&self) -> Result<Option<StateStore>> {
        match self.inner.load()? {
            Some(json_str) => {
                let value: serde_json::Value = serde_json::from_str(&json_str)?;
                let store = StateStore::from_json(&value)?;
                Ok(Some(store))
            }
            None => Ok(None),
        }
    }

    /// 列出所有快照（返回路径列表）
    pub fn list_snapshots(&self) -> Result<Vec<PathBuf>> {
        self.inner.list_snapshots()
    }

    /// 强制清理旧快照
    pub fn cleanup(&self) -> Result<()> {
        // 保存一个空快照以触发 cleanup 逻辑
        self.inner.save("{}", "cleanup")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::StateStore;
    use crate::types::state::StateValue;
    use std::env;

    fn setup_test_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("taiji_test_{}", name));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn test_state_manager_save_and_list() {
        let dir = setup_test_dir("snapshot_save_list");
        let mgr = StateManager::new(dir.clone());
        mgr.save(r#"{"test": true}"#, "1.0").unwrap();
        let list = mgr.list_snapshots().unwrap();
        assert!(!list.is_empty(), "应有快照文件");
        assert!(list[0].to_string_lossy().contains("snapshot_"));
        assert!(list[0].to_string_lossy().contains("v1.0"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_state_manager_load_empty() {
        let dir = setup_test_dir("snapshot_load_empty");
        let mgr = StateManager::new(dir.clone());
        let result = mgr.load().unwrap();
        assert!(result.is_none(), "空目录应返回 None");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_state_manager_save_and_load() {
        let dir = setup_test_dir("snapshot_save_load");
        let mgr = StateManager::new(dir.clone());
        mgr.save(r#"{"key": "value", "num": 42}"#, "2.0").unwrap();
        let loaded = mgr.load().unwrap();
        assert!(loaded.is_some(), "应有快照内容");
        let json = loaded.unwrap();
        assert!(json.contains("\"key\""));
        assert!(json.contains("\"value\""));
        assert!(json.contains("42"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_state_manager_cleanup() {
        let dir = setup_test_dir("snapshot_cleanup");
        let mgr = StateManager::new(dir.clone()).with_max_keep(3);

        // 保存 5 个快照，每次 save 触发 cleanup，保留最近 max_keep=3 个
        for i in 0..5 {
            mgr.save(&format!(r#"{{"v": {}}}"#, i), &format!("{}", i)).unwrap();
        }
        let list = mgr.list_snapshots().unwrap();
        assert_eq!(list.len(), 3, "max_keep=3 应保留 3 个快照");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_snapshot_manager_roundtrip() {
        let dir = setup_test_dir("snapshot_mgr_roundtrip");
        let mgr = SnapshotManager::new(dir.clone());

        // 创建 StateStore 并填入数据
        let store = StateStore::new();
        store.set("bool_key".into(), StateValue::Bool(true), "test_node".into());
        store.set("f64_key".into(), StateValue::F64(3.14159), "test_node".into());
        store.set("str_key".into(), StateValue::Usize(42), "test_node".into());

        // 保存快照
        mgr.save_snapshot(&store, "1.0").unwrap();

        // 加载恢复
        let restored = mgr.load_latest_snapshot().unwrap();
        assert!(restored.is_some(), "应有恢复的快照");

        let restored_store = restored.unwrap();
        // 验证数据完整
        let bool_val: Option<bool> = restored_store.get(&"bool_key".into());
        assert_eq!(bool_val, Some(true), "bool 值应恢复");
        let f64_val: Option<f64> = restored_store.get(&"f64_key".into());
        assert!((f64_val.unwrap_or(0.0) - 3.14159).abs() < 1e-10, "f64 值应恢复");
        let usize_val: Option<usize> = restored_store.get(&"str_key".into());
        assert_eq!(usize_val, Some(42), "usize 值应恢复");

        // 验证 provenance — 快照不持久化 provenance 信息，恢复后应为空
        let prov = restored_store.provenance_of(&"bool_key".into());
        assert!(prov.is_none(), "provenance 不应在快照中持久化");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_snapshot_manager_empty_recovery() {
        let dir = setup_test_dir("snapshot_mgr_empty");
        let mgr = SnapshotManager::new(dir.clone());

        let result = mgr.load_latest_snapshot().unwrap();
        assert!(result.is_none(), "空目录应返回 None");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_snapshot_manager_list() {
        let dir = setup_test_dir("snapshot_mgr_list");
        let mgr = SnapshotManager::new(dir.clone());

        let store = StateStore::new();
        store.set("k".into(), StateValue::Bool(false), "n".into());

        mgr.save_snapshot(&store, "v1").unwrap();
        mgr.save_snapshot(&store, "v2").unwrap();

        let list = mgr.list_snapshots().unwrap();
        assert_eq!(list.len(), 2, "应有 2 个快照文件");

        fs::remove_dir_all(&dir).ok();
    }
}
