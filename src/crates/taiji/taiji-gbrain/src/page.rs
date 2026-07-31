//! PageManager — 基于 GBrainEngine 的页面管理封装。
//!
//! 提供便捷的页面 CRUD 操作，跟踪页面元数据变更时间。
//!
//! 参考: gbrain (MIT) core/operations.ts — R-5-201 — Rust 翻译实现

use taiji_types::knowledge::{Page, PageInput};

use crate::engine::GBrainEngine;
use crate::error::GBrainError;

/// 页面管理器 — 基于 GBrainEngine 的 CRUD 封装（R-5-201）。
pub struct PageManager {
    engine: Box<dyn GBrainEngine>,
}

impl PageManager {
    /// 创建 PageManager。
    pub fn new(engine: Box<dyn GBrainEngine>) -> Self {
        Self { engine }
    }

    /// 获取页面。
    pub async fn get(&self, slug: &str) -> Result<Option<Page>, GBrainError> {
        self.engine.get_page(slug).await
    }

    /// 创建或更新页面。返回更新后的页面。
    pub async fn put(&mut self, slug: &str, input: PageInput) -> Result<Page, GBrainError> {
        self.engine.put_page(slug, input).await?;
        self.engine
            .get_page(slug)
            .await
            .transpose()
            .unwrap_or(Err(GBrainError::not_found(
                "page not found after put".to_string(),
            )))
    }

    /// 删除页面。返回是否实际删除。
    pub async fn delete(&mut self, slug: &str) -> Result<bool, GBrainError> {
        self.engine.delete_page(slug).await
    }

    /// 列出所有页面。
    pub async fn list(&self) -> Result<Vec<Page>, GBrainError> {
        self.engine.list_pages().await
    }

    /// 页面总数。
    pub async fn count(&self) -> Result<usize, GBrainError> {
        self.engine.page_count().await
    }

    /// 获取底层引擎引用。
    pub fn engine(&self) -> &dyn GBrainEngine {
        self.engine.as_ref()
    }

    /// 获取底层引擎可变引用。
    pub fn engine_mut(&mut self) -> &mut dyn GBrainEngine {
        self.engine.as_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taiji_types::knowledge::PageInput;

    use crate::engine::mock::MockEngine;
    use crate::engine::GBrainEngine;

    async fn make_manager() -> PageManager {
        let mut engine = MockEngine::new();
        engine
            .connect(&taiji_types::knowledge::GBrainConfig::default())
            .await
            .unwrap();
        engine.init_schema().await.unwrap();
        PageManager::new(Box::new(engine))
    }

    #[tokio::test]
    async fn test_page_manager_put_and_get() {
        let mut mgr = make_manager().await;

        let input = PageInput {
            title: "测试".into(),
            content: "内容正文".into(),
            source_id: Some("test_user".into()),
            tags: vec!["tag1".into(), "tag2".into()],
            metadata: serde_json::json!({"key": "val"}),
        };
        let page = mgr.put("test-slug", input).await.unwrap();
        assert_eq!(page.title, "测试");
        assert_eq!(page.tags.len(), 2);

        let fetched = mgr.get("test-slug").await.unwrap().unwrap();
        assert_eq!(fetched.title, "测试");
    }

    #[tokio::test]
    async fn test_page_manager_delete() {
        let mut mgr = make_manager().await;

        mgr.put(
            "del-slug",
            PageInput {
                title: "待删除".into(),
                content: "内容".into(),
                source_id: None,
                tags: vec![],
                metadata: serde_json::Value::Null,
            },
        )
        .await
        .unwrap();

        assert!(mgr.delete("del-slug").await.unwrap());
        assert!(mgr.get("del-slug").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_page_manager_count() {
        let mut mgr = make_manager().await;
        assert_eq!(mgr.count().await.unwrap(), 0);

        for i in 0..3 {
            mgr.put(
                &format!("slug-{}", i),
                PageInput {
                    title: format!("页面 {}", i),
                    content: "内容".into(),
                    source_id: None,
                    tags: vec![],
                    metadata: serde_json::Value::Null,
                },
            )
            .await
            .unwrap();
        }

        assert_eq!(mgr.count().await.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_page_manager_list() {
        let mut mgr = make_manager().await;
        mgr.put(
            "list-slug",
            PageInput {
                title: "列表测试".into(),
                content: "内容".into(),
                source_id: None,
                tags: vec![],
                metadata: serde_json::Value::Null,
            },
        )
        .await
        .unwrap();

        let pages = mgr.list().await.unwrap();
        assert!(pages.iter().any(|p| p.title == "列表测试"));
    }
}
