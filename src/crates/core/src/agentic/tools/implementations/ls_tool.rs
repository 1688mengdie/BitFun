//! LS tool implementation
//!
//! Provides functionality similar to Unix ls command for listing files and subdirectories in a directory

use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::util::list_files::{format_files_list, list_files};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use chrono::{DateTime, Local};
use serde_json::{json, Value};
use std::path::Path;
use std::time::SystemTime;

/// LS tool - list directory tree
pub struct LSTool {
    /// Default maximum number of entries to return
    default_limit: usize,
}

impl LSTool {
    pub fn new() -> Self {
        Self { default_limit: 200 }
    }
}

/// Format system time as readable string
fn format_time(time: SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[async_trait]
impl Tool for LSTool {
    fn name(&self) -> &str {
        "LS"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Recursively lists files and directories in a given path.

Usage:
- The path parameter must be an absolute path, not a relative path
- You can optionally provide an array of glob patterns to ignore with the ignore parameter
- Hidden files (files starting with '.') are automatically excluded
- Results are sorted by modification time (newest first)"#
            .to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The absolute path to the directory to list (must be absolute, not relative)"
                },
                "ignore": {
                    "type": "array",
                    "items": {
                        "type": "string",
                    },
                    "description": "List of glob patterns (relative to `path`) to ignore. Examples: \"*.js\" ignores all .js files."
                },
                "limit": {
                    "type": "number",
                    "description": "The maximum number of entries to return. Defaults to 100."
                },
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
            if path.is_empty() {
                return ValidationResult {
                    result: false,
                    message: Some("path cannot be empty".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }

            let path_obj = Path::new(path);

            // Validate if path is absolute
            if !path_obj.is_absolute() {
                return ValidationResult {
                    result: false,
                    message: Some(format!("path must be an absolute path, got: {}", path)),
                    error_code: Some(400),
                    meta: None,
                };
            }

            if !path_obj.exists() {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Directory does not exist: {}", path)),
                    error_code: Some(404),
                    meta: None,
                };
            }

            if !path_obj.is_dir() {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Path is not a directory: {}", path)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        } else {
            return ValidationResult {
                result: false,
                message: Some("path is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
            if options.verbose {
                format!("Listing directory: {}", path)
            } else {
                format!("List {}", path)
            }
        } else {
            "Listing directory".to_string()
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("path is required".to_string()))?;

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(self.default_limit);

        // Parse ignore parameter
        let ignore_patterns = input.get("ignore").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        });

        let entries = list_files(path, limit, ignore_patterns).map_err(|e| BitFunError::tool(e))?;

        // Build JSON data
        let entries_json = entries
            .iter()
            .filter(|entry| entry.depth == 1)
            .map(|entry| {
                json!({
                    "name": entry.path.file_name().unwrap_or_default().to_string_lossy(),
                    "path": entry.path.to_string_lossy(),
                    "is_dir": entry.is_dir,
                    "modified_time": format_time(entry.modified_time)
                })
            })
            .collect::<Vec<Value>>();
        let total_entries = entries.len();

        let mut result_text = format_files_list(entries, path);
        if total_entries == 0 {
            result_text.push_str("\n(no entries found)");
        } else if total_entries >= limit {
            result_text.push_str(&format!("\n(showing up to {} entries)", limit));
        }

        let result = ToolResult::Result {
            data: json!({
                "path": path,
                "entries": entries_json,
                "total": total_entries,
                "limit": limit
            }),
            result_for_assistant: Some(result_text),
        };

        Ok(vec![result])
    }
}
