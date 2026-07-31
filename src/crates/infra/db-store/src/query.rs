//! db-store（灵脉）查询过滤器与分页
//!
//! 来源: modules/db-store/接口设计.md:287-311 — QueryFilter 枚举
//! 来源: modules/db-store/接口设计.md:65-73 — PaginatedResult
//! 来源: modules/db-store/接口设计.md:285 — SQLite JSON1 适配

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// SQLite 兼容的查询过滤器
///
/// v2.4：使用 SQLite JSON1 扩展函数（json_extract）
/// 替代 Postgres JSONB @> 操作符。
#[derive(Debug, Clone)]
pub enum QueryFilter {
    /// 字段 = 值
    Eq(String, Value),
    /// 字段 != 值
    Ne(String, Value),
    /// 字段 > 值
    Gt(String, Value),
    /// 字段 < 值
    Lt(String, Value),
    /// 字段 IN 列表
    In(String, Vec<Value>),
    /// JSON 路径存在检查（json_extract(field, '$.key') IS NOT NULL）
    FieldExists(String),
    /// JSON 路径值匹配（json_extract(field, '$.path') = value）
    JsonEq {
        field: String,
        path: String,
        value: Value,
    },
    /// AND 组合
    And(Vec<QueryFilter>),
    /// OR 组合
    Or(Vec<QueryFilter>),
}

impl QueryFilter {
    /// 将 QueryFilter 转换为 (WHERE 子句, 参数列表)
    ///
    /// # 注意
    ///
    /// 简单的 WHERE 子句生成，复杂场景建议使用 `StorageBackend::list` 的默认实现。
    pub fn to_sql(&self, param_index: &mut usize) -> (String, Vec<Value>) {
        match self {
            QueryFilter::Eq(field, value) => {
                let idx = *param_index;
                *param_index += 1;
                (format!("{} = ${}", field, idx), vec![value.clone()])
            }
            QueryFilter::Ne(field, value) => {
                let idx = *param_index;
                *param_index += 1;
                (format!("{} != ${}", field, idx), vec![value.clone()])
            }
            QueryFilter::Gt(field, value) => {
                let idx = *param_index;
                *param_index += 1;
                (format!("{} > ${}", field, idx), vec![value.clone()])
            }
            QueryFilter::Lt(field, value) => {
                let idx = *param_index;
                *param_index += 1;
                (format!("{} < ${}", field, idx), vec![value.clone()])
            }
            QueryFilter::In(field, values) => {
                if values.is_empty() {
                    return ("1=0".into(), vec![]);
                }
                let placeholders: Vec<String> = values
                    .iter()
                    .map(|_| {
                        let idx = *param_index;
                        *param_index += 1;
                        format!("${}", idx)
                    })
                    .collect();
                (
                    format!("{} IN ({})", field, placeholders.join(", ")),
                    values.clone(),
                )
            }
            QueryFilter::FieldExists(field) => {
                // SQLite: json_extract(field, '$.key') IS NOT NULL
                // 当 field 本身包含 '.key' 路径时，特殊处理
                if field.contains('.') {
                    let parts: Vec<&str> = field.splitn(2, '.').collect();
                    (
                        format!(
                            "json_extract({}, '$.{}') IS NOT NULL",
                            parts[0], parts[1]
                        ),
                        vec![],
                    )
                } else {
                    (format!("{} IS NOT NULL", field), vec![])
                }
            }
            QueryFilter::JsonEq {
                field,
                path,
                value,
            } => {
                let idx = *param_index;
                *param_index += 1;
                (
                    format!(
                        "json_extract({}, '$.{}') = ${}",
                        field, path, idx
                    ),
                    vec![value.clone()],
                )
            }
            QueryFilter::And(filters) => {
                if filters.is_empty() {
                    return ("1=1".into(), vec![]);
                }
                let mut clauses = Vec::with_capacity(filters.len());
                let mut params = Vec::new();
                for f in filters {
                    let (clause, mut p) = f.to_sql(param_index);
                    clauses.push(format!("({})", clause));
                    params.append(&mut p);
                }
                (clauses.join(" AND "), params)
            }
            QueryFilter::Or(filters) => {
                if filters.is_empty() {
                    return ("1=0".into(), vec![]);
                }
                let mut clauses = Vec::with_capacity(filters.len());
                let mut params = Vec::new();
                for f in filters {
                    let (clause, mut p) = f.to_sql(param_index);
                    clauses.push(format!("({})", clause));
                    params.append(&mut p);
                }
                (clauses.join(" OR "), params)
            }
        }
    }
}

/// 分页查询结果
///
/// 来源: modules/db-store/接口设计.md:65-73 — PaginatedResult
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult {
    /// 当前页数据
    pub items: Vec<Value>,
    /// 总记录数
    pub total: u64,
    /// 当前页码（从 1 开始）
    pub page: u32,
    /// 每页条数
    pub per_page: u32,
    /// 总页数
    pub total_pages: u32,
}

impl PaginatedResult {
    /// 创建空分页结果
    pub fn empty(page: u32, per_page: u32) -> Self {
        Self {
            items: vec![],
            total: 0,
            page,
            per_page,
            total_pages: 0,
        }
    }

    /// 计算总页数
    pub fn with_items(items: Vec<Value>, total: u64, page: u32, per_page: u32) -> Self {
        let total_pages = if per_page == 0 {
            0
        } else {
            (total as f64 / per_page as f64).ceil() as u32
        };
        Self {
            items,
            total,
            page,
            per_page,
            total_pages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eq_filter() {
        let mut idx = 0;
        let f = QueryFilter::Eq("status".into(), Value::String("active".into()));
        let (sql, params) = f.to_sql(&mut idx);
        assert_eq!(sql, "status = $0");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_and_filter() {
        let mut idx = 0;
        let f = QueryFilter::And(vec![
            QueryFilter::Eq("status".into(), Value::String("active".into())),
            QueryFilter::Gt("credit".into(), Value::Number(serde_json::Number::from_f64(50.0).unwrap())),
        ]);
        let (sql, params) = f.to_sql(&mut idx);
        assert!(sql.contains("status = $0"));
        assert!(sql.contains("credit > $1"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_json_eq_filter() {
        let mut idx = 0;
        let f = QueryFilter::JsonEq {
            field: "metadata".into(),
            path: "class".into(),
            value: Value::String("gold".into()),
        };
        let (sql, params) = f.to_sql(&mut idx);
        assert!(sql.contains("json_extract(metadata, '$.class') = $0"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_field_exists() {
        let mut idx = 0;
        let f = QueryFilter::FieldExists("metadata.class".into());
        let (sql, params) = f.to_sql(&mut idx);
        assert!(sql.contains("json_extract(metadata, '$.class') IS NOT NULL"));
        assert!(params.is_empty());
    }

    #[test]
    fn test_paginated_result() {
        let items = vec![Value::Null; 25];
        let result = PaginatedResult::with_items(items, 100, 1, 25);
        assert_eq!(result.total_pages, 4);
        assert_eq!(result.page, 1);
    }

    #[test]
    fn test_paginated_empty() {
        let result = PaginatedResult::empty(1, 10);
        assert_eq!(result.total, 0);
        assert_eq!(result.total_pages, 0);
    }

    #[test]
    fn test_empty_in_filter() {
        let mut idx = 0;
        let f = QueryFilter::In("id".into(), vec![]);
        let (sql, _) = f.to_sql(&mut idx);
        assert_eq!(sql, "1=0");
    }
}
