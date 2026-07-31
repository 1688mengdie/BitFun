//! file-store（储物阁）— 上传流水线
//!
//! 来源: biliup upload.rs:44-88 (MIT)

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::FileStoreError;
use crate::meta::FileMeta;

/// 上传任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadTask {
    /// 来源路径（本地文件系统路径）
    pub source_path: PathBuf,
    /// 目标路径（存储后端中的路径）
    pub target_path: String,
    /// MIME 类型
    pub mime_type: String,
    /// 上传完成后是否删除源文件
    pub delete_source: bool,
}

/// 上传结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResult {
    /// 文件元数据
    pub file_meta: FileMeta,
    /// 访问 URL（如可用）
    pub url: Option<String>,
    /// 源文件是否已删除
    pub source_deleted: bool,
}

/// 上传流水线
#[async_trait]
pub trait UploadPipeline: Send + Sync {
    /// 执行上传任务
    async fn upload(&self, task: UploadTask) -> Result<UploadResult, FileStoreError>;

    /// 批量上传
    async fn upload_batch(
        &self,
        tasks: Vec<UploadTask>,
    ) -> Result<Vec<UploadResult>, FileStoreError>;

    /// 上传后处理（回调 Hook）
    async fn post_process(&self, result: &UploadResult) -> Result<(), FileStoreError>;
}
