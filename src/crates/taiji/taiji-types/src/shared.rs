//! 跨模块共享的原子类型别名。
//!
//! 统一类型别名，避免各模块各自定义相同概念。
//! 所有类型为简单包装/别名，零运行时开销。

use chrono::{DateTime, Utc};

/// 统一时间戳类型（UTC，纳秒精度）。
pub type Timestamp = DateTime<Utc>;

/// 统一版本号类型（语义化版本）。
pub type Version = semver::Version;

/// 统一元数据类型（任意 JSON 值）。
pub type Metadata = serde_json::Value;
