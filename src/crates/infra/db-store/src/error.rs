//! db-store（灵脉）错误类型
//!
//! 来源: modules/db-store/接口设计.md:320-338 — DbError 枚举
//! 来源: modules/db-store/接口设计.md:560-566 — BufferError 枚举

use thiserror::Error;

/// 数据库操作错误
#[derive(Error, Debug)]
pub enum DbError {
    /// 连接失败
    #[error("连接失败: {message}")]
    Connection {
        message: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// 查询错误
    #[error("查询错误: {message}")]
    Query {
        message: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// 迁移失败
    #[error("迁移失败: {0}")]
    Migration(String),

    /// 未找到
    #[error("未找到: {0}")]
    NotFound(String),

    /// 约束冲突
    #[error("约束冲突: {0}")]
    ConstraintViolation(String),

    /// 事务错误
    #[error("事务错误: {0}")]
    Transaction(String),

    /// 序列化错误
    #[error("序列化错误: {0}")]
    Serialization(String),

    /// IO 错误
    #[error("IO 错误: {0}")]
    Io(String),
}

impl DbError {
    /// 创建连接错误
    pub fn connection(msg: String) -> Self {
        DbError::Connection { message: msg, source: None }
    }
    /// 创建查询错误
    pub fn query(msg: String) -> Self {
        DbError::Query { message: msg, source: None }
    }
    /// 带来源的连接错误
    pub fn connection_with_source(msg: String, source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        DbError::Connection { message: msg, source: Some(source) }
    }
    /// 带来源的查询错误
    pub fn query_with_source(msg: String, source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        DbError::Query { message: msg, source: Some(source) }
    }
}

impl PartialEq for DbError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Connection { message: m1, .. }, Self::Connection { message: m2, .. }) => m1 == m2,
            (Self::Query { message: m1, .. }, Self::Query { message: m2, .. }) => m1 == m2,
            _ => format!("{self:?}") == format!("{other:?}"),
        }
    }
}

impl Clone for DbError {
    fn clone(&self) -> Self {
        match self {
            Self::Connection { message, .. } => DbError::Connection { message: message.clone(), source: None },
            Self::Query { message, .. } => DbError::Query { message: message.clone(), source: None },
            Self::Migration(s) => DbError::Migration(s.clone()),
            Self::NotFound(s) => DbError::NotFound(s.clone()),
            Self::ConstraintViolation(s) => DbError::ConstraintViolation(s.clone()),
            Self::Transaction(s) => DbError::Transaction(s.clone()),
            Self::Serialization(s) => DbError::Serialization(s.clone()),
            Self::Io(s) => DbError::Io(s.clone()),
        }
    }
}

impl From<sqlx::Error> for DbError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::Database(ref db_err) => {
                if let Some(code) = db_err.code() {
                    if code.as_ref() == "1555" || code.as_ref() == "1205" {
                        return DbError::ConstraintViolation(db_err.message().to_string());
                    }
                }
                DbError::query(db_err.message().to_string())
            }
            sqlx::Error::RowNotFound => DbError::NotFound("记录未找到".to_string()),
            sqlx::Error::Io(e) => DbError::Io(e.to_string()),
            other => DbError::query(other.to_string()),
        }
    }
}

impl From<serde_json::Error> for DbError {
    fn from(e: serde_json::Error) -> Self {
        DbError::Serialization(e.to_string())
    }
}

/// L1 环缓冲错误
///
/// 来源: modules/db-store/接口设计.md:559-566 — BufferError
#[derive(Error, Debug, Clone, PartialEq)]
pub enum BufferError {
    /// 容量已满
    #[error("缓冲区容量已满: {0}")]
    CapacityFull(String),

    /// 无效参数
    #[error("无效参数: {0}")]
    InvalidParameter(String),
}
