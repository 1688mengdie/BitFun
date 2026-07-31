//! Prompt 模板管理 —— 支持 `{{变量}}` 占位符替换。
//!
//! 提供 [`PromptTemplate`] 结构体，用于定义和管理 LLM prompt 模板。
//! 渲染时自动替换 `{{变量}}` 占位符，缺失变量返回错误。
//!
//! # 使用示例
//!
//! ```ignore
//! use taiji_llm::prompt::PromptTemplate;
//! use std::collections::HashMap;
//!
//! let tmpl = PromptTemplate::new("分析 {{symbol}} 的 {{direction}} 方向，周期 {{timeframe}}");
//! let mut vars = HashMap::new();
//! vars.insert("symbol".into(), "rb9999".into());
//! vars.insert("direction".into(), "趋势".into());
//! vars.insert("timeframe".into(), "日线".into());
//!
//! let result = tmpl.render(&vars).unwrap();
//! assert_eq!(result, "分析 rb9999 的趋势方向，周期 日线");
//! ```
//! 参考: 量价时空/Phase-2-派发提示词.md:891 — R-2-506 — taiji-llm LLM 集成

use std::collections::HashMap;

/// Prompt 模板 —— 支持 `{{变量}}` 占位符替换。
///
/// # 模板语法
///
/// 使用双花括号 `{{变量名}}` 标记占位符，渲染时通过 [`render`](Self::render) 替换。
///
/// ```ignore
/// let tmpl = PromptTemplate::new("{{symbol}} {{action}}");
/// let vars = HashMap::from([("symbol".into(), "rb9999".into()), ("action".into(), "long".into())]);
/// assert_eq!(tmpl.render(&vars).unwrap(), "rb9999 long");
/// ```
///
/// # 错误处理
///
/// - 如果传入的变量在模板中不存在，返回 [`anyhow::Error`]
/// - 如果模板中有未替换的占位符，返回 [`anyhow::Error`]
#[derive(Debug, Clone)]
pub struct PromptTemplate {
    /// 原始模板字符串
    template: String,
}

impl PromptTemplate {
    /// 创建新的 Prompt 模板。
    ///
    /// `template` 中可包含 `{{变量名}}` 占位符。
    pub fn new(template: impl Into<String>) -> Self {
        Self {
            template: template.into(),
        }
    }

    /// 渲染模板，将所有 `{{变量}}` 替换为传入的值。
    ///
    /// # 错误
    ///
    /// - 如果某个传入的变量在模板中不存在对应的占位符
    /// - 如果模板中有未替换的 `{{...}}` 占位符
    pub fn render(&self, vars: &HashMap<String, String>) -> anyhow::Result<String> {
        let mut result = self.template.clone();

        // 替换所有已知变量
        for (key, value) in vars {
            let placeholder = format!("{{{{{}}}}}", key);
            if !result.contains(&placeholder) {
                anyhow::bail!(
                    "模板中未找到占位符 `{}`，模板: {}",
                    placeholder,
                    self.template
                );
            }
            result = result.replace(&placeholder, value);
        }

        // 检查是否还有未替换的占位符（用户未提供的变量）
        if let Some(start) = result.find("{{") {
            let end = result[start..]
                .find("}}")
                .map(|e| start + e + 2)
                .unwrap_or(start + 2);
            let remaining = &result[start..end];
            anyhow::bail!(
                "模板中存在未替换的占位符 {}，模板: {}",
                remaining,
                self.template
            );
        }

        Ok(result)
    }

    /// 获取原始模板字符串。
    pub fn template(&self) -> &str {
        &self.template
    }
}

// ── 测试 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_template_basic_render() {
        let tmpl = PromptTemplate::new("分析 {{symbol}} 的趋势方向");
        let mut vars = HashMap::new();
        vars.insert("symbol".into(), "rb9999".into());

        let result = tmpl.render(&vars).unwrap();
        assert_eq!(result, "分析 rb9999 的趋势方向");
    }

    #[test]
    fn test_prompt_template_multiple_vars() {
        let tmpl = PromptTemplate::new("{{symbol}} {{direction}} {{timeframe}}");
        let mut vars = HashMap::new();
        vars.insert("symbol".into(), "ag2506".into());
        vars.insert("direction".into(), "做多".into());
        vars.insert("timeframe".into(), "15分钟".into());

        let result = tmpl.render(&vars).unwrap();
        assert_eq!(result, "ag2506 做多 15分钟");
    }

    #[test]
    fn test_prompt_template_repeated_var() {
        let tmpl = PromptTemplate::new("{{symbol}} 现价 {{symbol}}");
        let mut vars = HashMap::new();
        vars.insert("symbol".into(), "rb9999".into());

        let result = tmpl.render(&vars).unwrap();
        assert_eq!(result, "rb9999 现价 rb9999");
    }

    #[test]
    fn test_prompt_template_no_vars() {
        let tmpl = PromptTemplate::new("纯文本模板无变量");
        let vars = HashMap::new();
        let result = tmpl.render(&vars).unwrap();
        assert_eq!(result, "纯文本模板无变量");
    }

    #[test]
    fn test_prompt_template_missing_var_in_template() {
        let tmpl = PromptTemplate::new("分析 {{symbol}}");
        let mut vars = HashMap::new();
        vars.insert("nonexistent".into(), "rb9999".into());

        let err = tmpl.render(&vars).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("未找到占位符"), "错误信息应提示未找到占位符: {}", msg);
    }

    #[test]
    fn test_prompt_template_unreplaced_placeholder() {
        let tmpl = PromptTemplate::new("分析 {{symbol}} 的 {{direction}}");
        let mut vars = HashMap::new();
        vars.insert("symbol".into(), "rb9999".into());
        // direction 未提供

        let err = tmpl.render(&vars).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("未替换的占位符"), "错误信息应提示未替换占位符: {}", msg);
    }

    #[test]
    fn test_prompt_template_escape_literal_braces() {
        // 非占位符的花括号应保持原样
        let tmpl = PromptTemplate::new("分析 {{symbol}}（置信度 {{confidence}}%）");
        let mut vars = HashMap::new();
        vars.insert("symbol".into(), "rb9999".into());
        vars.insert("confidence".into(), "85".into());

        let result = tmpl.render(&vars).unwrap();
        assert_eq!(result, "分析 rb9999（置信度 85%）");
    }

    #[test]
    fn test_prompt_template_template_accessor() {
        let tmpl = PromptTemplate::new("你好 {{name}}");
        assert_eq!(tmpl.template(), "你好 {{name}}");
    }

    #[test]
    fn test_prompt_template_empty_value() {
        let tmpl = PromptTemplate::new("分析 {{symbol}}");
        let mut vars = HashMap::new();
        vars.insert("symbol".into(), "".into());

        let result = tmpl.render(&vars).unwrap();
        assert_eq!(result, "分析 ");
    }

    #[test]
    fn test_prompt_template_chinese_var_names() {
        let tmpl = PromptTemplate::new("{{品种}} {{方向}}");
        let mut vars = HashMap::new();
        vars.insert("品种".into(), "rb9999".into());
        vars.insert("方向".into(), "long".into());

        let result = tmpl.render(&vars).unwrap();
        assert_eq!(result, "rb9999 long");
    }
}
