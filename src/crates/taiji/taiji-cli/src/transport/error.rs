//! 传输错误类型

use std::fmt;

/// 传输层错误
#[derive(Debug)]
pub(crate) enum TransportError {
    /// 连接错误（后端不可用、网络断开）
    Connection(String),
    /// 协议错误（JSON-RPC 格式错误、响应异常）
    Protocol(String),
    /// 认证错误
    #[allow(dead_code)]
    Auth(String),
    /// 超时
    #[allow(dead_code)]
    Timeout(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection(msg) => write!(f, "连接错误: {}", msg),
            Self::Protocol(msg) => write!(f, "协议错误: {}", msg),
            Self::Auth(msg) => write!(f, "认证错误: {}", msg),
            Self::Timeout(msg) => write!(f, "超时: {}", msg),
        }
    }
}

impl std::error::Error for TransportError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_error_display() {
        let err = TransportError::Connection(" refused".to_string());
        assert_eq!(format!("{}", err), "连接错误:  refused");
    }

    #[test]
    fn test_protocol_error_display() {
        let err = TransportError::Protocol("invalid JSON".to_string());
        assert_eq!(format!("{}", err), "协议错误: invalid JSON");
    }

    #[test]
    fn test_auth_error_display() {
        let err = TransportError::Auth("token expired".to_string());
        assert_eq!(format!("{}", err), "认证错误: token expired");
    }

    #[test]
    fn test_timeout_error_display() {
        let err = TransportError::Timeout("30s".to_string());
        assert_eq!(format!("{}", err), "超时: 30s");
    }

    #[test]
    fn test_error_trait_implemented() {
        let err = TransportError::Connection("test".to_string());
        let trait_obj: &dyn std::error::Error = &err;
        assert_eq!(trait_obj.to_string(), "连接错误: test");
    }

    #[test]
    fn test_debug_output() {
        let err = TransportError::Protocol("bad request".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("Protocol"));
        assert!(debug.contains("bad request"));
    }
}
