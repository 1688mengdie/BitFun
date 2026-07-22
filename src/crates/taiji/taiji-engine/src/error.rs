use crate::types::state::StateKey;
use crate::types::NodeId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TaijiError {
    #[error("config error: {0}")]
    Config(String),
    #[error("data source error: {0}")]
    DataSource(String),
    #[error("node '{node}' failed: {reason}")]
    NodeFailed { node: NodeId, reason: String },
    #[error("required key '{0}' not found")]
    KeyNotFound(StateKey),
    #[error("circular dependency: {0:?}")]
    CycleDetected(Vec<NodeId>),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("all sources down for '{0}'")]
    AllSourcesDown(String),
    #[error("fusion error: {0}")]
    Fusion(String),
}

pub type Result<T> = std::result::Result<T, TaijiError>;
