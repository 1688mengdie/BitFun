//! GBrainError — gbrain 引擎错误类型。

use thiserror::Error;

/// gbrain 引擎错误。
#[derive(Debug, Error)]
pub enum GBrainError {
    /// 连接失败 — 数据库/网络层错误。
    #[error("connection error: {0}")]
    Connection(String),

    /// 查询错误 — SQL/搜索查询执行失败。
    #[error("query error: {0}")]
    Query(String),

    /// 未找到 — 页面/分块不存在。
    #[error("not found: {0}")]
    NotFound(String),

    /// 配置错误 — 无效配置或参数。
    #[error("config error: {0}")]
    Config(String),

    /// 引擎未初始化 — connect() 尚未调用。
    #[error("engine not initialized")]
    NotInitialized,

    /// IO 错误 — 文件/磁盘操作失败。
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON 序列化/反序列化错误。
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl GBrainError {
    pub fn connection(msg: impl Into<String>) -> Self {
        Self::Connection(msg.into())
    }
    pub fn query(msg: impl Into<String>) -> Self {
        Self::Query(msg.into())
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
}
