//! file-store（储物阁）— 存储配置
//!
//! 来源: gbrain storage.ts:17-30 (MIT)

/// 存储后端类型
#[derive(Debug, Clone, PartialEq)]
pub enum FileStoreKind {
    /// 本地文件系统
    Local,
    /// AWS S3
    S3,
    /// MinIO（S3 兼容）
    MinIO,
}

/// 文件存储配置
#[derive(Debug, Clone)]
pub struct FileStoreConfig {
    /// 存储后端类型
    pub backend: FileStoreKind,
    /// 根路径（本地文件系统）
    pub local_path: Option<String>,
    /// S3/MinIO 兼容配置
    pub s3: Option<S3Config>,
}

/// S3/MinIO 连接配置
#[derive(Debug, Clone)]
pub struct S3Config {
    /// 存储桶名称
    pub bucket: String,
    /// 区域
    pub region: String,
    /// 端点 URL（MinIO 需要）
    pub endpoint: Option<String>,
    /// Access Key
    pub access_key_id: String,
    /// Secret Key
    pub secret_access_key: String,
}
