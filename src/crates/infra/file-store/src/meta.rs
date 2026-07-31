//! file-store（储物阁）— 文件元数据
//!
//! 来源: gbrain storage.ts (MIT)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 文件元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMeta {
    /// 文件路径
    pub path: String,
    /// 文件大小（字节）
    pub size: u64,
    /// MIME 类型
    pub mime_type: String,
    /// 内容哈希（SHA256）
    pub content_hash: String,
    /// 最后修改时间
    pub last_modified: DateTime<Utc>,
    /// 是否目录
    pub is_dir: bool,
}
