//! file-store（储物阁）— 文件存储接口定义
//!
//! 来源: gbrain storage.ts:8-15 (MIT)

use tokio::io::AsyncRead;

use async_trait::async_trait;

use crate::config::{FileStoreConfig, FileStoreKind};
use crate::error::FileStoreError;
use crate::local::LocalFileStorage;
use crate::meta::FileMeta;

#[cfg(feature = "s3")]
use crate::s3_storage::S3FileStorage;

/// 统一文件存储后端 trait
///
/// 支持本地文件系统 / S3 / MinIO
#[async_trait]
pub trait FileStorage: Send + Sync {
    /// 写入文件（创建或覆盖）
    async fn write(&self, path: &str, data: &[u8]) -> Result<FileMeta, FileStoreError>;

    /// 流式写入（大文件）
    async fn write_stream(
        &self,
        path: &str,
        reader: Box<dyn AsyncRead + Send + Unpin>,
        size_hint: Option<u64>,
    ) -> Result<FileMeta, FileStoreError>;

    /// 读取文件全部内容
    async fn read(&self, path: &str) -> Result<Vec<u8>, FileStoreError>;

    /// 流式读取
    async fn read_stream(
        &self,
        path: &str,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, FileStoreError>;

    /// 删除文件
    async fn delete(&self, path: &str) -> Result<bool, FileStoreError>;

    /// 检查文件是否存在
    async fn exists(&self, path: &str) -> Result<bool, FileStoreError>;

    /// 获取文件元数据
    async fn metadata(&self, path: &str) -> Result<FileMeta, FileStoreError>;

    /// 列出指定前缀下的所有文件
    async fn list(&self, prefix: &str) -> Result<Vec<FileMeta>, FileStoreError>;

    /// 获取文件公开 URL
    async fn get_url(&self, path: &str) -> Result<String, FileStoreError>;

    /// 复制文件
    async fn copy(&self, from: &str, to: &str) -> Result<FileMeta, FileStoreError>;

    /// 移动/重命名文件
    async fn move_file(&self, from: &str, to: &str) -> Result<FileMeta, FileStoreError>;
}

/// 创建文件存储后端实例
///
/// 来源: gbrain storage.ts:35-52 (MIT)
pub async fn create_file_storage(
    config: &FileStoreConfig,
) -> Result<Box<dyn FileStorage>, FileStoreError> {
    match config.backend {
        FileStoreKind::Local => {
            let path = config
                .local_path
                .clone()
                .ok_or_else(|| FileStoreError::InvalidPath("local_path is required".into()))?;
            Ok(Box::new(LocalFileStorage::new(path)))
        }
        FileStoreKind::S3 | FileStoreKind::MinIO => {
            #[cfg(feature = "s3")]
            {
                let s3_config = config.s3.clone().ok_or_else(|| {
                    FileStoreError::InvalidPath("s3 config is required".into())
                })?;
                let storage = S3FileStorage::new(s3_config).await?;
                return Ok(Box::new(storage));
            }
            #[cfg(not(feature = "s3"))]
            {
                let _ = config;
                Err(FileStoreError::InvalidPath(
                    "S3/MinIO support requires feature \"s3\"".into(),
                ))
            }
        }
    }
}
