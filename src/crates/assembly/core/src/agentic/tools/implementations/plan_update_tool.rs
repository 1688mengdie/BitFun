//! PlanUpdate tool implementation
//!
//! Updates todo statuses inside an existing plan file (YAML frontmatter),
//! preserving every other frontmatter field and the markdown body byte-for-byte.

use crate::agentic::tools::framework::{Tool, ToolExposure, ToolResult, ToolUseContext};
use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::fs;

/// PlanUpdate tool - update todo statuses in a plan file
pub struct PlanUpdateTool;

impl PlanUpdateTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlanUpdateTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a plan file body into its YAML frontmatter (kept as a JSON value so
/// round-trip writes preserve key order and formatting) and markdown body.
pub(crate) fn parse_plan_file(content: &str) -> BitFunResult<(Value, String)> {
    let trimmed = content.trim_start();
    let after_open = trimmed
        .strip_prefix("---")
        .ok_or_else(|| BitFunError::tool("Plan file is missing the YAML frontmatter opener '---'"))?;
    let end = after_open.find("\n---").ok_or_else(|| {
        BitFunError::tool("Plan file is missing the YAML frontmatter closer '---'")
    })?;
    let yaml_part = &after_open[..end];
    let body_start = end + "\n---".len();
    let body = after_open[body_start..]
        .trim_start_matches(['\n', '\r'])
        .to_string();

    let frontmatter: Value = serde_yaml::from_str(yaml_part).map_err(|error| {
        BitFunError::tool(format!(
            "Failed to parse plan YAML frontmatter: {}",
            error
        ))
    })?;
    Ok((frontmatter, body))
}

/// One todo update: id plus any subset of status/content/dependencies. At
/// least one of the three fields must be present (enforced at input parsing).
/// `pub(crate)` so the backend scheduler (plan-todo binding) can construct
/// single-status updates without a ToolUseContext.
pub(crate) struct TodoUpdate {
    pub(crate) id: String,
    pub(crate) status: Option<String>,
    pub(crate) content: Option<String>,
    pub(crate) dependencies: Option<Vec<String>>,
}

/// Validate todo updates against a parsed frontmatter. Every update is checked
/// before anything is written: the status value (when present) must be legal
/// and the todo id must exist. Returns the applied updates for the tool result.
pub(crate) fn validate_updates(frontmatter: &Value, updates: &[TodoUpdate]) -> BitFunResult<Vec<Value>> {
    let mut applied = Vec::with_capacity(updates.len());
    for update in updates {
        if let Some(status) = &update.status {
            if !matches!(status.as_str(), "pending" | "in_progress" | "completed") {
                return Err(BitFunError::validation(format!(
                    "Invalid todo status '{}' for id '{}': expected one of pending, in_progress, completed",
                    status, update.id
                )));
            }
        }
        let found = frontmatter
            .get("todos")
            .and_then(|value| value.as_array())
            .map(|todos| {
                todos
                    .iter()
                    .any(|todo| todo.get("id").and_then(|v| v.as_str()) == Some(update.id.as_str()))
            })
            .unwrap_or(false);
        if !found {
            return Err(BitFunError::tool(format!(
                "Todo id not found in plan: {}",
                update.id
            )));
        }
        let mut applied_item = json!({ "id": update.id });
        if let Some(status) = &update.status {
            applied_item["status"] = Value::String(status.clone());
        }
        if let Some(content) = &update.content {
            applied_item["content"] = Value::String(content.clone());
        }
        if let Some(dependencies) = &update.dependencies {
            applied_item["dependencies"] =
                Value::Array(dependencies.iter().map(|d| Value::String(d.clone())).collect());
        }
        applied.push(applied_item);
    }
    Ok(applied)
}

/// Quote a single-line YAML scalar value so it can be written back safely as
/// `  content: <value>`. Values with YAML special characters (or control
/// chars) are double-quoted with escaping; plain values stay bare so the
/// common create_plan_tool.rs layout is preserved.
fn yaml_quote_single_line(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let special = value.chars().any(|c| {
        c.is_control()
            || matches!(
                c,
                ':' | '#'
                    | '"'
                    | '\''
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | ','
                    | '&'
                    | '*'
                    | '!'
                    | '|'
                    | '>'
                    | '%'
                    | '@'
                    | '`'
            ) || (c == ' ' && value.starts_with(' '))
            || (c == '-' && value.starts_with('-'))
    });
    if !special {
        return value.to_string();
    }
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{}\"", escaped)
}

/// Preserve a trailing CR from CRLF files when rebuilding a line.
fn line_tail_cr(line: &str) -> &str {
    if line.ends_with('\r') {
        "\r"
    } else {
        ""
    }
}

/// Apply validated updates at the text level: only the matching `  status:`,
/// `  content:` and `  dependencies:` lines inside the `todos:` block are
/// replaced, so every other byte of the plan file (frontmatter key order,
/// indentation, markdown body) stays exactly as it was. The serde_yaml Value
/// round-trip is NOT used here because it reorders YAML mapping keys, which
/// would violate the format-preservation contract.
///
/// Multi-line `content: |`/`content: >` blocks are collapsed: the block
/// header is replaced with a single-line content value and the indented body
/// lines (4+ spaces) are dropped. Old dependency list items (`  - x`) are
/// dropped when the dependencies field is replaced.
pub(crate) fn apply_updates_text(content: &str, updates: &[TodoUpdate]) -> BitFunResult<String> {
    let targets: std::collections::HashMap<&str, &TodoUpdate> = updates
        .iter()
        .map(|update| (update.id.as_str(), update))
        .collect();
    let mut expected_fields = 0usize;
    for update in updates {
        expected_fields += usize::from(update.status.is_some())
            + usize::from(update.content.is_some())
            + usize::from(update.dependencies.is_some());
    }

    let mut out: Vec<String> = Vec::new();
    let mut in_todos = false;
    let mut current_id: Option<String> = None;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut replaced = 0usize;

    for line in content.split('\n') {
        // Tolerate CRLF files: the trailing \r must not break structural
        // matching (it is preserved when rebuilding the line).
        let structural = line.trim_end_matches('\r');
        if !in_todos {
            // The todos block starts at the top-level `todos:` key.
            if structural == "todos:" || structural.starts_with("todos: ") {
                in_todos = true;
            }
            out.push(line.to_string());
            continue;
        }
        // A new todo item starts.
        if let Some(id) = structural.strip_prefix("- id: ") {
            current_id = Some(id.trim().to_string());
            seen.clear();
            out.push(line.to_string());
            continue;
        }
        // A top-level key (unindented, not a list item) ends the todos block.
        if !structural.starts_with(' ') && !structural.starts_with('\t') && !structural.starts_with('-') && !structural.is_empty()
        {
            in_todos = false;
            out.push(line.to_string());
            continue;
        }
        let is_target = current_id
            .as_deref()
            .is_some_and(|id| targets.contains_key(id));

        // Old content block body lines (4+ spaces indentation): drop them once
        // the content field of this target todo has been replaced.
        if structural.starts_with("    ") || structural.starts_with('\t') {
            if is_target && seen.contains("content") {
                continue;
            }
            out.push(line.to_string());
            continue;
        }
        // Old dependency list items (`  - x`): drop them once the dependencies
        // field of this target todo has been replaced.
        if structural.starts_with("  - ") || structural.starts_with("  -") {
            if is_target && seen.contains("dependencies") {
                continue;
            }
            out.push(line.to_string());
            continue;
        }
        // content field (single line or block header).
        if structural.starts_with("  content: ") || structural == "  content:" {
            if is_target && !seen.contains("content") {
                if let Some(update) = targets.get(current_id.as_deref().expect("is_target")) {
                    if let Some(new_content) = &update.content {
                        out.push(format!(
                            "  content: {}{}",
                            yaml_quote_single_line(new_content),
                            line_tail_cr(line)
                        ));
                        seen.insert("content".to_string());
                        replaced += 1;
                        continue;
                    }
                }
            }
            out.push(line.to_string());
            continue;
        }
        // status field.
        if structural.starts_with("  status: ") {
            if is_target && !seen.contains("status") {
                if let Some(update) = targets.get(current_id.as_deref().expect("is_target")) {
                    if let Some(new_status) = &update.status {
                        let prefix_len = "  status: ".len();
                        let tail = &line[prefix_len..];
                        // Keep everything after the old value (e.g. a trailing
                        // CR from CRLF files) byte-identical.
                        let old_value_len = tail.trim_end_matches(['\r', ' ', '\t']).len();
                        out.push(format!("  status: {}{}", new_status, &tail[old_value_len..]));
                        seen.insert("status".to_string());
                        replaced += 1;
                        continue;
                    }
                }
            }
            out.push(line.to_string());
            continue;
        }
        // dependencies field.
        if structural.starts_with("  dependencies:") {
            if is_target && !seen.contains("dependencies") {
                if let Some(update) = targets.get(current_id.as_deref().expect("is_target")) {
                    if let Some(new_dependencies) = &update.dependencies {
                        let cr = line_tail_cr(line);
                        if new_dependencies.is_empty() {
                            out.push(format!("  dependencies: []{}", cr));
                        } else {
                            out.push(format!("  dependencies:{}", cr));
                            for dependency in new_dependencies {
                                out.push(format!("  - {}{}", dependency, cr));
                            }
                        }
                        seen.insert("dependencies".to_string());
                        replaced += 1;
                        continue;
                    }
                }
            }
            out.push(line.to_string());
            continue;
        }
        // Any other line (unknown nested fields, blank lines).
        out.push(line.to_string());
    }

    if replaced != expected_fields {
        return Err(BitFunError::tool(format!(
            "Failed to locate all requested todo fields (found {} of {})",
            replaced, expected_fields
        )));
    }
    Ok(out.join("\n"))
}

/// Resolve the plan file argument to a concrete filesystem path. Absolute
/// paths (or paths containing a separator) are used as-is; bare file names
/// are resolved against the current workspace plans directory. Minimal copy of
/// plan_read_tool.rs::resolve_plan_path (kept local so plan files stay
/// self-contained per tool module).
async fn resolve_plan_path(plan_file: &str, context: &ToolUseContext) -> BitFunResult<PathBuf> {
    let supplied = PathBuf::from(plan_file);
    let looks_like_path = plan_file.contains('/') || plan_file.contains('\\');
    if supplied.is_absolute() || looks_like_path {
        // Note: extension() only returns the last suffix ("md" for
        // "xxx.plan.md"), so validate the full file name suffix instead.
        let file_name = supplied
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if !file_name.ends_with(".plan.md") {
            return Err(BitFunError::tool(format!(
                "Plan file must end with .plan.md: {}",
                plan_file
            )));
        }
        if !supplied.exists() {
            return Err(BitFunError::tool(format!(
                "Plan file not found: {}",
                plan_file
            )));
        }
        return Ok(supplied);
    }

    let runtime_context = context.ensure_current_workspace_runtime().await?;
    let plans_dir = runtime_context.plans_dir.clone();
    let plan_path = plans_dir.join(plan_file);
    if !plan_path.exists() {
        return Err(BitFunError::tool(format!(
            "Plan file not found in plans directory: {} (plans_dir={})",
            plan_file,
            plans_dir.to_string_lossy()
        )));
    }
    Ok(plan_path)
}

/// Resolve the plan file argument to a concrete filesystem path WITHOUT a
/// ToolUseContext (backend scheduler use, e.g. plan-todo binding). Absolute
/// paths (or paths containing a separator) are used as-is; bare file names are
/// resolved against the plans directory derived from the given workspace root
/// (`~/.bitfun/projects/<workspace-slug>/plans`). Mirrors `resolve_plan_path`
/// semantics: `.plan.md` suffix validation + exists check for supplied paths,
/// plans-dir join + exists check for bare names. Remote workspaces must be
/// filtered by the caller: their plan files live on the remote host, not in
/// the local mirror.
pub(crate) async fn resolve_plan_path_for_backend(
    plan_file: &str,
    workspace_path: Option<&Path>,
) -> BitFunResult<PathBuf> {
    let supplied = PathBuf::from(plan_file);
    let looks_like_path = plan_file.contains('/') || plan_file.contains('\\');
    if supplied.is_absolute() || looks_like_path {
        let file_name = supplied
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if !file_name.ends_with(".plan.md") {
            return Err(BitFunError::tool(format!(
                "Plan file must end with .plan.md: {}",
                plan_file
            )));
        }
        if !supplied.exists() {
            return Err(BitFunError::tool(format!(
                "Plan file not found: {}",
                plan_file
            )));
        }
        return Ok(supplied);
    }

    let workspace_path = workspace_path.ok_or_else(|| {
        BitFunError::tool(
            "A workspace path is required to resolve a plan file in the plans directory"
                .to_string(),
        )
    })?;
    let plans_dir = get_path_manager_arc().project_plans_dir(workspace_path);
    let plan_path = plans_dir.join(plan_file);
    if !plan_path.exists() {
        return Err(BitFunError::tool(format!(
            "Plan file not found in plans directory: {} (plans_dir={})",
            plan_file,
            plans_dir.to_string_lossy()
        )));
    }
    Ok(plan_path)
}

/// Apply a single todo status update to a plan file at the given path (backend
/// scheduler use, e.g. plan-todo binding). Reads, validates and rewrites the
/// file atomically (same write path as the PlanUpdate tool); returns the
/// applied update for logging. Errors are surfaced to the caller, which owns
/// the failure policy (the scheduler treats them as best-effort).
pub(crate) async fn apply_todo_status_update(
    plan_path: &Path,
    todo_id: &str,
    status: &str,
) -> BitFunResult<Value> {
    let content = fs::read_to_string(plan_path)
        .await
        .map_err(|error| BitFunError::tool(format!("Failed to read plan file: {}", error)))?;
    let (frontmatter, _body) = parse_plan_file(&content)?;
    let updates = vec![TodoUpdate {
        id: todo_id.to_string(),
        status: Some(status.to_string()),
        content: None,
        dependencies: None,
    }];
    let applied = validate_updates(&frontmatter, &updates)?;
    let new_content = apply_updates_text(&content, &updates)?;

    // Atomic write: write a sibling temp file, then rename over the target.
    let tmp_path = PathBuf::from(format!("{}.tmp", plan_path.to_string_lossy()));
    fs::write(&tmp_path, new_content.as_bytes())
        .await
        .map_err(|error| BitFunError::tool(format!("Failed to write plan file: {}", error)))?;
    if let Err(error) = fs::rename(&tmp_path, plan_path).await {
        let _ = fs::remove_file(&tmp_path).await;
        return Err(BitFunError::tool(format!(
            "Failed to replace plan file: {}",
            error
        )));
    }
    Ok(applied
        .into_iter()
        .next()
        .unwrap_or_else(|| json!({ "id": todo_id })))
}

#[async_trait]
impl Tool for PlanUpdateTool {
    fn name(&self) -> &str {
        "PlanUpdate"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r###"Update todos in an existing plan file. The input accepts the plan file name (for example "my_plan_1234abcd.plan.md") or a full path to a .plan.md file, plus an array of todo updates. Each update has an id and at least one of: status ("pending", "in_progress" or "completed"), content (new todo description), or dependencies (new array of dependency todo ids; an empty array clears them). Reads the plan file, validates that every todo id exists and every status is legal, updates the matching todo fields in the YAML frontmatter, and writes the file back atomically while preserving every other frontmatter field and the markdown body unchanged. Errors clearly when the plan file does not exist, a todo id is not found, or a status value is invalid."###
            .to_string())
    }

    fn short_description(&self) -> String {
        "Update todo status, content or dependencies in a plan file.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        // 2026-08-04 user calibration: the plan tool family is a commander
        // staple; Direct so no GetToolSpec unlock round-trip is needed
        // (mirrored by `shared_coding_mode_tool_exposure_overrides()`).
        ToolExposure::Direct
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["plan_file", "updates"],
            "properties": {
                "plan_file": {
                    "type": "string",
                    "description": "Plan file name (e.g. my_plan_1234abcd.plan.md) or an absolute path to a .plan.md file"
                },
                "updates": {
                    "type": "array",
                    "description": "Array of todo updates; at least one is required",
                    "items": {
                        "type": "object",
                        "required": ["id"],
                        "properties": {
                            "id": {
                                "type": "string",
                                "description": "Id of the todo to update (must exist in the plan)"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"],
                                "description": "New todo status"
                            },
                            "content": {
                                "type": "string",
                                "description": "New todo content (replaces the existing content)"
                            },
                            "dependencies": {
                                "type": "array",
                                "description": "New dependency todo ids (replaces the existing list; an empty array clears them)",
                                "items": {
                                    "type": "string"
                                }
                            }
                        }
                    }
                }
            }
        })
    }

    fn is_readonly(&self) -> bool {
        // Only writes the plan file, does not modify code (same semantics as
        // CreatePlan: the plan family is a commander staple, not a code tool).
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let plan_file = input
            .get("plan_file")
            .and_then(|value| value.as_str())
            .ok_or(BitFunError::validation(
                "Missing required field: plan_file",
            ))?;
        let plan_file = plan_file.trim();
        if plan_file.is_empty() {
            return Err(BitFunError::validation(
                "Missing required field: plan_file",
            ));
        }

        let updates_value = input
            .get("updates")
            .and_then(|value| value.as_array())
            .ok_or(BitFunError::validation("Missing required field: updates"))?;
        if updates_value.is_empty() {
            return Err(BitFunError::validation(
                "updates must contain at least one todo update",
            ));
        }
        let mut updates = Vec::with_capacity(updates_value.len());
        for update in updates_value {
            let id = update
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or(BitFunError::validation(
                    "Each update requires an 'id' field",
                ))?;
            let status = update
                .get("status")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let content = update
                .get("content")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let dependencies = update
                .get("dependencies")
                .and_then(|value| value.as_array())
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.as_str().map(String::from))
                        .collect::<Vec<_>>()
                });
            if status.is_none() && content.is_none() && dependencies.is_none() {
                return Err(BitFunError::validation(
                    "Each update requires at least one of 'status', 'content' or 'dependencies'",
                ));
            }
            updates.push(TodoUpdate {
                id: id.to_string(),
                status,
                content,
                dependencies,
            });
        }

        let plan_path = resolve_plan_path(plan_file, context).await?;
        let content = fs::read_to_string(&plan_path)
            .await
            .map_err(|error| BitFunError::tool(format!("Failed to read plan file: {}", error)))?;
        let (frontmatter, _body) = parse_plan_file(&content)?;
        let applied = validate_updates(&frontmatter, &updates)?;
        let new_content = apply_updates_text(&content, &updates)?;

        // Atomic write: write a sibling temp file, then rename over the target.
        let tmp_path = PathBuf::from(format!("{}.tmp", plan_path.to_string_lossy()));
        fs::write(&tmp_path, new_content.as_bytes())
            .await
            .map_err(|error| BitFunError::tool(format!("Failed to write plan file: {}", error)))?;
        if let Err(error) = fs::rename(&tmp_path, &plan_path).await {
            let _ = fs::remove_file(&tmp_path).await;
            return Err(BitFunError::tool(format!(
                "Failed to replace plan file: {}",
                error
            )));
        }

        let plan_reference = context.build_runtime_artifact_reference(&format!(
            "plans/{}",
            plan_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default()
        ))?;

        let result = json!({
            "success": true,
            "plan_file_name": plan_path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(),
            "plan_file_path": plan_reference,
            "updated": applied
        });

        Ok(vec![ToolResult::Result {
            data: result,
            result_for_assistant: None,
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_update(id: &str, status: &str) -> TodoUpdate {
        TodoUpdate {
            id: id.to_string(),
            status: Some(status.to_string()),
            content: None,
            dependencies: None,
        }
    }

    #[test]
    fn apply_updates_text_preserves_every_other_byte() {
        let content = "---\nname: My Plan\noverview: An overview\ntodos:\n- id: setup-auth\n  content: Set up auth\n  status: pending\n- id: implement-ui\n  content: Implement the UI\n  status: pending\n  dependencies:\n  - setup-auth\n---\n\n# My Plan\n\nBody text here.\n";
        let updates = vec![status_update("setup-auth", "completed")];
        let updated = apply_updates_text(content, &updates).expect("apply updates");

        // Every non-status byte stays identical: key order, indentation and
        // the markdown body must all be preserved exactly.
        let expected = "---\nname: My Plan\noverview: An overview\ntodos:\n- id: setup-auth\n  content: Set up auth\n  status: completed\n- id: implement-ui\n  content: Implement the UI\n  status: pending\n  dependencies:\n  - setup-auth\n---\n\n# My Plan\n\nBody text here.\n";
        assert_eq!(updated, expected);

        // Cross-check through the parser as well.
        let (frontmatter, body) = parse_plan_file(&updated).expect("re-parse updated file");
        assert!(body.contains("Body text here."));
        assert_eq!(frontmatter["name"].as_str(), Some("My Plan"));
        assert_eq!(frontmatter["overview"].as_str(), Some("An overview"));
        let todos = frontmatter["todos"].as_array().expect("todos array");
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0]["id"].as_str(), Some("setup-auth"));
        assert_eq!(todos[0]["content"].as_str(), Some("Set up auth"));
        assert_eq!(todos[0]["status"].as_str(), Some("completed"));
        assert_eq!(todos[1]["status"].as_str(), Some("pending"));
        assert_eq!(
            todos[1]["dependencies"].as_array().map(|deps| deps[0].as_str()),
            Some(Some("setup-auth"))
        );
    }

    #[test]
    fn apply_updates_text_updates_multiple_todos() {
        let content = "---\ntodos:\n- id: a\n  content: A\n  status: pending\n- id: b\n  content: B\n  status: pending\n- id: c\n  content: C\n  status: pending\n---\n\nbody";
        let updates = vec![
            status_update("a", "in_progress"),
            status_update("c", "completed"),
        ];
        let updated = apply_updates_text(content, &updates).expect("apply updates");
        let expected = "---\ntodos:\n- id: a\n  content: A\n  status: in_progress\n- id: b\n  content: B\n  status: pending\n- id: c\n  content: C\n  status: completed\n---\n\nbody";
        assert_eq!(updated, expected);
    }

    #[test]
    fn apply_updates_text_keeps_crlf_line_endings() {
        let content = "---\r\ntodos:\r\n- id: a\r\n  content: A\r\n  status: pending\r\n---\r\n\r\nbody\r\n";
        let updates = vec![status_update("a", "completed")];
        let updated = apply_updates_text(content, &updates).expect("apply updates");
        let expected = "---\r\ntodos:\r\n- id: a\r\n  content: A\r\n  status: completed\r\n---\r\n\r\nbody\r\n";
        assert_eq!(updated, expected);
    }

    #[test]
    fn apply_updates_text_updates_content_single_line() {
        let content = "---\ntodos:\n- id: a\n  content: Old content\n  status: pending\n---\n\nbody";
        let updates = vec![TodoUpdate {
            id: "a".to_string(),
            status: None,
            content: Some("New content".to_string()),
            dependencies: None,
        }];
        let updated = apply_updates_text(content, &updates).expect("apply updates");
        let expected = "---\ntodos:\n- id: a\n  content: New content\n  status: pending\n---\n\nbody";
        assert_eq!(updated, expected);

        // Parser agrees on the new content.
        let (frontmatter, _) = parse_plan_file(&updated).expect("re-parse");
        assert_eq!(frontmatter["todos"][0]["content"].as_str(), Some("New content"));
        assert_eq!(frontmatter["todos"][0]["status"].as_str(), Some("pending"));
    }

    #[test]
    fn apply_updates_text_collapses_multiline_content_block() {
        // Hand-edited plan with a literal block content.
        let content = "---\ntodos:\n- id: a\n  content: |\n    Line one\n    Line two\n  status: pending\n---\n\nbody";
        let updates = vec![TodoUpdate {
            id: "a".to_string(),
            status: None,
            content: Some("Replaced".to_string()),
            dependencies: None,
        }];
        let updated = apply_updates_text(content, &updates).expect("apply updates");
        let expected = "---\ntodos:\n- id: a\n  content: Replaced\n  status: pending\n---\n\nbody";
        assert_eq!(updated, expected);

        let (frontmatter, _) = parse_plan_file(&updated).expect("re-parse");
        assert_eq!(frontmatter["todos"][0]["content"].as_str(), Some("Replaced"));
    }

    #[test]
    fn apply_updates_text_quotes_special_content() {
        let content = "---\ntodos:\n- id: a\n  content: plain\n  status: pending\n---\n\nbody";
        let updates = vec![TodoUpdate {
            id: "a".to_string(),
            status: None,
            content: Some("needs: quoting".to_string()),
            dependencies: None,
        }];
        let updated = apply_updates_text(content, &updates).expect("apply updates");
        assert!(updated.contains("  content: \"needs: quoting\""));

        let (frontmatter, _) = parse_plan_file(&updated).expect("re-parse");
        assert_eq!(
            frontmatter["todos"][0]["content"].as_str(),
            Some("needs: quoting")
        );
    }

    #[test]
    fn apply_updates_text_updates_dependencies() {
        let content = "---\ntodos:\n- id: a\n  content: A\n  status: pending\n  dependencies:\n  - x\n  - y\n---\n\nbody";
        let updates = vec![TodoUpdate {
            id: "a".to_string(),
            status: None,
            content: None,
            dependencies: Some(vec!["new-dep".to_string(), "other".to_string()]),
        }];
        let updated = apply_updates_text(content, &updates).expect("apply updates");
        let expected = "---\ntodos:\n- id: a\n  content: A\n  status: pending\n  dependencies:\n  - new-dep\n  - other\n---\n\nbody";
        assert_eq!(updated, expected);

        let (frontmatter, _) = parse_plan_file(&updated).expect("re-parse");
        assert_eq!(
            frontmatter["todos"][0]["dependencies"]
                .as_array()
                .map(|deps| deps[0].as_str()),
            Some(Some("new-dep"))
        );
    }

    #[test]
    fn apply_updates_text_clears_dependencies_with_empty_array() {
        let content = "---\ntodos:\n- id: a\n  content: A\n  status: pending\n  dependencies:\n  - x\n  - y\n---\n\nbody";
        let updates = vec![TodoUpdate {
            id: "a".to_string(),
            status: None,
            content: None,
            dependencies: Some(Vec::new()),
        }];
        let updated = apply_updates_text(content, &updates).expect("apply updates");
        let expected = "---\ntodos:\n- id: a\n  content: A\n  status: pending\n  dependencies: []\n---\n\nbody";
        assert_eq!(updated, expected);

        let (frontmatter, _) = parse_plan_file(&updated).expect("re-parse");
        assert_eq!(
            frontmatter["todos"][0]["dependencies"]
                .as_array()
                .map(|deps| deps.len()),
            Some(0)
        );
    }

    #[test]
    fn apply_updates_text_combines_status_content_and_dependencies() {
        let content = "---\ntodos:\n- id: a\n  content: Old\n  status: pending\n  dependencies:\n  - x\n---\n\nbody";
        let updates = vec![TodoUpdate {
            id: "a".to_string(),
            status: Some("completed".to_string()),
            content: Some("New".to_string()),
            dependencies: Some(vec!["y".to_string()]),
        }];
        let updated = apply_updates_text(content, &updates).expect("apply updates");
        let expected = "---\ntodos:\n- id: a\n  content: New\n  status: completed\n  dependencies:\n  - y\n---\n\nbody";
        assert_eq!(updated, expected);
    }

    #[test]
    fn validate_updates_rejects_invalid_status() {
        let content = "---\ntodos:\n- id: a\n  content: A\n  status: pending\n---\n\nbody";
        let (frontmatter, _) = parse_plan_file(content).expect("parse plan file");
        let error = validate_updates(&frontmatter, &[status_update("a", "done")])
            .expect_err("invalid status must error");
        let message = error.to_string();
        assert!(
            message.contains("Invalid todo status 'done'"),
            "unexpected error: {}",
            message
        );
    }

    #[test]
    fn validate_updates_rejects_unknown_id() {
        let content = "---\ntodos:\n- id: a\n  content: A\n  status: pending\n---\n\nbody";
        let (frontmatter, _) = parse_plan_file(content).expect("parse plan file");
        let error = validate_updates(&frontmatter, &[status_update("missing-id", "completed")])
            .expect_err("unknown id must error");
        let message = error.to_string();
        assert!(
            message.contains("Todo id not found in plan: missing-id"),
            "unexpected error: {}",
            message
        );
    }

    #[test]
    fn validate_updates_rejects_plan_without_todos() {
        let content = "---\nname: Legacy\n---\n\nbody";
        let (frontmatter, _) = parse_plan_file(content).expect("parse plan file");
        let error = validate_updates(&frontmatter, &[status_update("anything", "completed")])
            .expect_err("plan without todos must error");
        let message = error.to_string();
        assert!(
            message.contains("Todo id not found in plan: anything"),
            "unexpected error: {}",
            message
        );
    }

    #[test]
    fn validate_updates_accepts_content_only_update() {
        let content = "---\ntodos:\n- id: a\n  content: A\n  status: pending\n---\n\nbody";
        let (frontmatter, _) = parse_plan_file(content).expect("parse plan file");
        let update = TodoUpdate {
            id: "a".to_string(),
            status: None,
            content: Some("Changed".to_string()),
            dependencies: None,
        };
        let applied = validate_updates(&frontmatter, &[update]).expect("content-only update");
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0]["id"].as_str(), Some("a"));
        assert_eq!(applied[0]["content"].as_str(), Some("Changed"));
        assert!(applied[0].get("status").is_none());
    }

    #[test]
    fn parse_plan_file_missing_delimiters_errors() {
        // Damaged or empty files surface a clear parse error; missing files are
        // rejected earlier by resolve_plan_path (exists check).
        assert!(parse_plan_file("no frontmatter here").is_err());
        assert!(parse_plan_file("").is_err());
        assert!(parse_plan_file("---\nname: x").is_err());
    }
}
