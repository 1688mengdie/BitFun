use pyo3::prelude::*;
use std::sync::{Arc, Mutex};
use taiji_engine::config::PipelineConfig;
use taiji_engine::pipeline::Pipeline;
use taiji_engine::store::StateStore;

/// Python 端可操作的 Pipeline 包装
#[pyclass]
pub struct PipelinePy {
    inner: Mutex<Option<Pipeline>>,
    /// 缓存的 StateStore Arc（与 inner Pipeline 指向同一实例），
    /// 供 ObsBuilder / TaijiRLEnv 无锁读取。
    state: Mutex<Option<Arc<StateStore>>>,
}

#[pymethods]
impl PipelinePy {
    #[new]
    fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            state: Mutex::new(None),
        }
    }

    /// 从 YAML 字符串加载配置并创建 Pipeline
    // #[allow] — pyo3 method, not a Rust constructor; takes &self for Python ergonomics.
    #[allow(clippy::wrong_self_convention)]
    fn from_yaml(&self, yaml_str: &str) -> PyResult<()> {
        let config: PipelineConfig = serde_yaml::from_str(yaml_str)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        let pipeline = Pipeline::from_config(config)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        // 缓存 StateStore Arc（与 inner Pipeline 共享同一实例）
        let store_arc = pipeline.state_store_arc();
        *self.state.lock().unwrap() = Some(store_arc);
        *self.inner.lock().unwrap() = Some(pipeline);
        Ok(())
    }

    /// 返回 Pipeline 状态信息
    pub fn status(&self) -> PyResult<String> {
        let guard = self.inner.lock().unwrap();
        match guard.as_ref() {
            Some(p) => {
                let s = p.status();
                Ok(format!(
                    "state: {:?}, nodes: {}, ticks: {}, signals: {}",
                    s.state,
                    s.nodes.len(),
                    s.total_ticks,
                    s.total_signals
                ))
            }
            None => Ok("not initialized".into()),
        }
    }

    fn __repr__(&self) -> String {
        match self.status() {
            Ok(s) => format!("PipelinePy({})", s),
            Err(_) => "PipelinePy(error)".into(),
        }
    }
}

impl PipelinePy {
    /// 获取内部 Pipeline 的 StateStore 引用（用于观测构建）。
    /// 返回 None 当 Pipeline 尚未初始化。
    ///
    /// Safety: StateStore 使用 DashMap（全 interior mutability），
    /// 返回的引用仅在当前 Python GIL 帧内有效。
    pub fn state_store(&self) -> Option<&StateStore> {
        let guard = self.state.lock().unwrap();
        match guard.as_ref() {
            Some(arc) => {
                let ptr: *const StateStore = Arc::as_ptr(arc);
                // SAFETY: The Arc<StateStore> is owned by self (PipelinePy holds it
                // in `state: Mutex<Option<Arc<StateStore>>>`). The returned reference
                // borrows from the Arc whose lifetime is tied to self (the PyClass).
                // The Mutex guard ensures no concurrent mutation while the reference
                // is borrowed. This pointer-to-reference cast is sound because:
                // 1. The Arc keeps the StateStore alive for the lifetime of PipelinePy.
                // 2. The Mutex lock guarantees exclusive access (no data race).
                // 3. Python GIL ensures the PyClass is not dropped while this method runs.
                Some(unsafe { &*ptr })
            }
            None => None,
        }
    }
}
