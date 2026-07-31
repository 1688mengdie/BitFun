//! file-store（储物阁）— S3 存储实现
//!
//! 来源: gbrain storage/s3.ts (MIT)
//! 仅在 feature "s3" 启用时编译。

use std::io::AsyncRead;

use async_trait::async_trait;

use crate::error::FileStoreError;
use crate::meta::FileMeta;
use crate::storage::FileStorage;
use crate::config::S3Config;

/// S3 文件存储实现
///
/// 基于 aws-sdk-s3，兼容 AWS S3 和 MinIO。
pub struct S3FileStorage {
    /// 存储桶名称
    bucket: String,
    /// AWS SDK S3 客户端
    client: aws_sdk_s3::Client,
}

impl S3FileStorage {
    /// 创建 S3 文件存储
    pub async fn new(config: S3Config) -> Result<Self, FileStoreError> {
        use aws_sdk_s3::config::{Credentials, Region};

        let credentials = Credentials::new(
            &config.access_key_id,
            &config.secret_access_key,
            None,
            None,
            None,
        );

        let mut builder = aws_sdk_s3::config::Builder::new()
            .credentials_provider(credentials)
            .region(Region::new(config.region));

        if let Some(endpoint) = &config.endpoint {
            builder = builder.endpoint_url(endpoint);
        }

        let client = aws_sdk_s3::Client::from_conf(builder.build());
        Ok(Self {
            bucket: config.bucket,
            client,
        })
    }
}

#[async_trait]
impl FileStorage for S3FileStorage {
    async fn write(&self, path: &str, data: &[u8]) -> Result<FileMeta, FileStoreError> {
        use aws_sdk_s3::primitives::ByteStream;

        let body = ByteStream::from(data.to_vec());
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(path)
            .body(body)
            .send()
            .await
            .map_err(|e| FileStoreError::S3(e.to_string()))?;
        self.metadata(path).await
    }

    async fn write_stream(
        &self,
        path: &str,
        _reader: Box<dyn AsyncRead + Send + Unpin>,
        _size_hint: Option<u64>,
    ) -> Result<FileMeta, FileStoreError> {
        // S3 流式上传需要 MultipartUpload，暂存简化骨架
        Err(FileStoreError::S3("流式写入暂未实现，请使用 write()".into()))
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, FileStoreError> {
        use aws_sdk_s3::primitives::ByteStream;

        let output = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(|e| FileStoreError::S3(e.to_string()))?;

        let data = output
            .body
            .collect()
            .await
            .map_err(|e| FileStoreError::S3(e.to_string()))?;
        Ok(data.to_vec())
    }

    async fn read_stream(
        &self,
        path: &str,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, FileStoreError> {
        let output = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(|e| FileStoreError::S3(e.to_string()))?;

        let byte_stream = output.body;
        let reader = tokio_util::io::StreamReader::new(
            byte_stream.map(|chunk| {
                chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            }),
        );
        Ok(Box::new(reader))
    }

    async fn delete(&self, path: &str) -> Result<bool, FileStoreError> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(|e| FileStoreError::S3(e.to_string()))?;
        Ok(true)
    }

    async fn exists(&self, path: &str) -> Result<bool, FileStoreError> {
        let result = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await;
        match result {
            Ok(_) => Ok(true),
            Err(err) => {
                if err.into_service_error().meta().code()
                    == Some("NotFound")
                {
                    Ok(false)
                } else {
                    Ok(false)
                }
            }
        }
    }

    async fn metadata(&self, path: &str) -> Result<FileMeta, FileStoreError> {
        let output = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(|e| FileStoreError::NotFound(format!("{}: {}", path, e)))?;

        let size = output.content_length() as u64;
        let mime_type = output
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        Ok(FileMeta {
            path: path.to_string(),
            size,
            mime_type,
            content_hash: String::new(), // S3 ETag 不一定是 MD5
            last_modified: output
                .last_modified()
                .map(|t| chrono::DateTime::from(t))
                .unwrap_or_else(chrono::Utc::now),
            is_dir: false,
        })
    }

    async fn list(&self, prefix: &str) -> Result<Vec<FileMeta>, FileStoreError> {
        let mut result = Vec::new();
        let mut token = None;

        loop {
            let mut req = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(prefix);

            if let Some(t) = token {
                req = req.continuation_token(t);
            }

            let output = req
                .send()
                .await
                .map_err(|e| FileStoreError::S3(e.to_string()))?;

            if let Some(contents) = output.contents() {
                for obj in contents {
                    if let Some(key) = obj.key() {
                        if let Ok(meta) = self.metadata(key).await {
                            result.push(meta);
                        }
                    }
                }
            }

            token = output.next_continuation_token();
            if token.is_none() {
                break;
            }
        }

        Ok(result)
    }

    async fn get_url(&self, path: &str) -> Result<String, FileStoreError> {
        // 对于 S3，返回公开 URL 格式（需要桶配置公开访问）
        Ok(format!(
            "https://{}.s3.amazonaws.com/{}",
            self.bucket, path
        ))
    }

    async fn copy(&self, from: &str, to: &str) -> Result<FileMeta, FileStoreError> {
        use aws_sdk_s3::operation::copy_object::CopyObjectRequest;

        let source_key = format!("{}/{}", self.bucket, from);
        self.client
            .copy_object()
            .copy_source(source_key)
            .bucket(&self.bucket)
            .key(to)
            .send()
            .await
            .map_err(|e| FileStoreError::S3(e.to_string()))?;
        self.metadata(to).await
    }

    async fn move_file(&self, from: &str, to: &str) -> Result<FileMeta, FileStoreError> {
        let result = self.copy(from, to).await?;
        self.delete(from).await?;
        Ok(result)
    }
}
