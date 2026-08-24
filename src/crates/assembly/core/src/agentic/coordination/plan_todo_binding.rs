//! Plan-todo binding between agent sessions and plan todos.
//!
//! `SessionMessage` can bind a dispatched session to a plan todo by carrying
//! `planFile` / `todoId` in the forwarded turn metadata (see
//! `session_message_tool.rs`). The scheduler reads that binding and issues
//! best-effort plan-file todo status changes:
//! - when a bound execution turn starts            -> todo `in_progress`
//! - when a bound execution turn finishes as OK    -> todo `completed`
//!
//! Every failure is logged and swallowed: the binding layer must never break,
//! delay, or block a dialog turn (best-effort semantics). Callers gate on
//! `reply_route.is_some()` so reply turns (which inherit the binding metadata)
//! never re-trigger the hooks.

use bitfun_agent_runtime::scheduler::{TurnOutcome, TurnOutcomeStatus};
use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, info, warn};
use serde_json::Value;
use serde_yaml::Value as YamlValue;
use std::path::Path;
use tokio::fs;

/// Metadata key injected by `SessionMessage` when a dispatch is bound to a plan file.
pub(crate) const PLAN_FILE_METADATA_KEY: &str = "planFile";
/// Metadata key injected by `SessionMessage` when a dispatch is bound to a plan todo.
pub(crate) const TODO_ID_METADATA_KEY: &str = "todoId";

/// Read the optional plan-todo binding from turn metadata. Returns
/// `(plan_file, todo_id)` when both keys are present and non-empty.
pub(crate) fn read_todo_binding(metadata: Option<&Value>) -> Option<(String, String)> {
    let metadata = metadata?;
    let plan_file = metadata.get(PLAN_FILE_METADATA_KEY)?.as_str()?;
    let todo_id = metadata.get(TODO_ID_METADATA_KEY)?.as_str()?;
    let plan_file = plan_file.trim();
    let todo_id = todo_id.trim();
    if plan_file.is_empty() || todo_id.is_empty() {
        return None;
    }
    Some((plan_file.to_string(), todo_id.to_string()))
}

/// Pure decision: should the auto-complete hook fire for this outcome? Only
/// `Completed` outcomes advance the todo; `Failed` / `Cancelled` outcomes are
/// kept pending for the commander to adjudicate.
pub(crate) fn should_auto_complete_todo(outcome: &TurnOutcome) -> bool {
    outcome.status() == TurnOutcomeStatus::Completed
}

/// Best-effort: mark the bound todo `in_progress` when the turn metadata
/// carries a plan-todo binding. Caller gates on `reply_route.is_some()` so
/// only execution turns (never reply turns) reach this hook.
pub(crate) async fn auto_mark_todo_in_progress_if_bound(
    metadata: Option<&Value>,
    workspace_path: Option<&str>,
    remote_connection_id: Option<&str>,
    remote_ssh_host: Option<&str>,
) {
    mark_todo_status_if_bound(
        metadata,
        workspace_path,
        remote_connection_id,
        remote_ssh_host,
        "in_progress",
        "auto_mark_todo_in_progress",
    )
    .await;
}

/// Best-effort: mark the bound todo `completed` when the finished turn carried
/// a plan-todo binding AND completed normally. `Failed` / `Cancelled` outcomes
/// are left untouched. Caller gates on `reply_route.is_some()` so reply turns
/// (which inherit the binding metadata) never re-mark.
pub(crate) async fn auto_mark_todo_completed_if_bound(
    metadata: Option<&Value>,
    workspace_path: Option<&str>,
    remote_connection_id: Option<&str>,
    remote_ssh_host: Option<&str>,
    outcome: &TurnOutcome,
) {
    if !should_auto_complete_todo(outcome) {
        return;
    }
    mark_todo_status_if_bound(
        metadata,
        workspace_path,
        remote_connection_id,
        remote_ssh_host,
        "completed",
        "auto_mark_todo_completed",
    )
    .await;
}

/// Apply a single todo status update when the turn metadata carries a plan-todo
/// binding. Remote workspaces keep their plan files on the remote host, so the
/// local scheduler cannot read or write them — skip instead of failing noisily.
async fn mark_todo_status_if_bound(
    metadata: Option<&Value>,
    workspace_path: Option<&str>,
    remote_connection_id: Option<&str>,
    remote_ssh_host: Option<&str>,
    status: &str,
    hook: &str,
) {
    let Some((plan_file, todo_id)) = read_todo_binding(metadata) else {
        return;
    };
    if remote_connection_id.is_some() || remote_ssh_host.is_some() {
        debug!(
            "{}: skipping plan-todo binding on remote workspace (plan files live on the remote host): plan_file={}, todo_id={}",
            hook, plan_file, todo_id
        );
        return;
    }
    let Some(workspace_path) = workspace_path else {
        warn!(
            "{}: cannot resolve plan-todo binding without a workspace path: plan_file={}, todo_id={}",
            hook, plan_file, todo_id
        );
        return;
    };
    let result = async {
        let plan_path = resolve_plan_path_for_backend(&plan_file, Some(Path::new(workspace_path))).await?;
        apply_todo_status_update(&plan_path, &todo_id, status).await?;
        Ok::<_, BitFunError>(())
    }
    .await;
    match result {
        Ok(()) => info!(
            "{}: plan todo marked {}: plan_file={}, todo_id={}",
            hook, status, plan_file, todo_id
        ),
        Err(error) => warn!(
            "{}: failed to update bound plan todo (best-effort, turn continues): plan_file={}, todo_id={}, error={}",
            hook, plan_file, todo_id, error
        ),
    }
}

/// Resolve the plan file argument to a concrete filesystem path WITHOUT a
/// `ToolUseContext` (backend scheduler use, e.g. plan-todo binding). Bare file
/// names are resolved against the plans directory derived from the given
/// workspace root (`~/.bitfun/projects/<workspace-slug>/plans`), converging on
/// the same plans-dir that `CreatePlan` writes to. Remote workspaces must be
/// filtered by the caller: their plan files live on the remote host.
async fn resolve_plan_path_for_backend(
    plan_file: &str,
    workspace_path: Option<&Path>,
) -> BitFunResult<std::path::PathBuf> {
    let workspace_path = workspace_path.ok_or_else(|| {
        BitFunError::tool(
            "A workspace path is required to resolve a plan file in the plans directory"
                .to_string(),
        )
    })?;
    let plans_dir = get_path_manager_arc().project_plans_dir(workspace_path);
    let candidate = Path::new(plan_file);
    if candidate.is_absolute() {
        return Ok(candidate.to_path_buf());
    }
    // Bare file names are resolved inside the plans directory; the file name
    // itself carries the `.plan.md` suffix that `CreatePlan` produces.
    Ok(plans_dir.join(plan_file))
}

/// Apply a single todo status update to a plan file at the given path
/// (backend scheduler use, e.g. plan-todo binding). Reads, parses the YAML
/// frontmatter, locates the todo by id, rewrites its `status`, and writes the
/// file back atomically. Errors are surfaced to the caller, which owns the
/// failure policy (the scheduler treats them as best-effort).
async fn apply_todo_status_update(
    plan_path: &Path,
    todo_id: &str,
    status: &str,
) -> BitFunResult<()> {
    let content = fs::read_to_string(plan_path)
        .await
        .map_err(|error| BitFunError::tool(format!("Failed to read plan file: {}", error)))?;
    let (frontmatter, body) = parse_plan_file(&content)?;

    let todos = frontmatter
        .get("todos")
        .and_then(YamlValue::as_sequence)
        .ok_or_else(|| BitFunError::tool("Plan file has no todos".to_string()))?;
    if !todos.iter().any(|todo| todo.get("id").and_then(YamlValue::as_str) == Some(todo_id)) {
        return Err(BitFunError::tool(format!(
            "Todo id not found in plan: {}",
            todo_id
        )));
    }

    // Re-serialize the frontmatter with the updated status, preserving key
    // order and the markdown body unchanged (same write path shape as
    // `CreatePlan`). `serde_yaml::Value` keeps mapping insertion order (unlike
    // `serde_json::Value`, whose `Map` alphabetizes keys), so a round trip
    // leaves the frontmatter layout stable.
    let mut frontmatter_value = frontmatter;
    let mut updated_todos = frontmatter_value
        .get("todos")
        .and_then(YamlValue::as_sequence)
        .cloned()
        .unwrap_or_default();
    for todo in updated_todos.iter_mut() {
        if let Some(todo_obj) = todo.as_mapping_mut() {
            if todo_obj.get("id").and_then(YamlValue::as_str) == Some(todo_id) {
                todo_obj.insert(
                    YamlValue::String("status".to_string()),
                    YamlValue::String(status.to_string()),
                );
                break;
            }
        }
    }
    if let Some(map) = frontmatter_value.as_mapping_mut() {
        map.insert(
            YamlValue::String("todos".to_string()),
            YamlValue::Sequence(updated_todos),
        );
    }

    let yaml = serde_yaml::to_string(&frontmatter_value).map_err(|error| {
        BitFunError::tool(format!("Failed to serialize plan frontmatter: {}", error))
    })?;
    let new_content = format!("---\n{}---\n\n{}", yaml, body);

    atomic_write_plan_file(plan_path, new_content.as_bytes()).await
}

/// Split a plan file into its YAML frontmatter and markdown body. Mirrors the
/// frontmatter shape that `CreatePlan` produces (`---\n<yaml>---\n\n<body>`).
fn parse_plan_file(content: &str) -> BitFunResult<(YamlValue, String)> {
    let trimmed = content.trim_start();
    let after_open = trimmed.strip_prefix("---").ok_or_else(|| {
        BitFunError::tool("Plan file is missing the YAML frontmatter opener '---'")
    })?;
    let end = after_open.find("\n---").ok_or_else(|| {
        BitFunError::tool("Plan file is missing the YAML frontmatter closer '---'")
    })?;
    // Strip a trailing CR in case the file uses CRLF line endings.
    let yaml_part = after_open[..end].trim_end_matches('\r');
    let body_start = end + "\n---".len();
    let body = after_open[body_start..]
        .trim_start_matches(['\n', '\r'])
        .to_string();

    let frontmatter: YamlValue = serde_yaml::from_str(yaml_part).map_err(|error| {
        BitFunError::tool(format!("Failed to parse plan YAML frontmatter: {}", error))
    })?;
    Ok((frontmatter, body))
}

/// Write a plan file atomically: write to a sibling temp file, then rename over
/// the target. Best-effort callers never observe a partially written plan.
async fn atomic_write_plan_file(plan_path: &Path, content: &[u8]) -> BitFunResult<()> {
    let parent = plan_path
        .parent()
        .ok_or_else(|| BitFunError::tool("Plan file path has no parent directory".to_string()))?;
    let file_name = plan_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| BitFunError::tool("Plan file path has no file name".to_string()))?;
    let temp_path = parent.join(format!(".{file_name}.tmp"));
    fs::write(&temp_path, content)
        .await
        .map_err(|error| BitFunError::tool(format!("Failed to write plan file: {}", error)))?;
    fs::rename(&temp_path, plan_path)
        .await
        .map_err(|error| BitFunError::tool(format!("Failed to replace plan file: {}", error)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn completed_outcome(turn_id: &str) -> TurnOutcome {
        TurnOutcome::Completed {
            turn_id: turn_id.to_string(),
            final_response: "done".to_string(),
        }
    }

    #[test]
    fn read_todo_binding_returns_none_without_metadata() {
        assert_eq!(read_todo_binding(None), None);
    }

    #[test]
    fn read_todo_binding_returns_none_without_binding_keys() {
        let metadata = json!({ "senderSessionId": "source-1" });
        assert_eq!(read_todo_binding(Some(&metadata)), None);
    }

    #[test]
    fn read_todo_binding_requires_both_keys() {
        let metadata = json!({ "planFile": "my_plan_1234.plan.md" });
        assert_eq!(read_todo_binding(Some(&metadata)), None);
        let metadata = json!({ "todoId": "setup-auth" });
        assert_eq!(read_todo_binding(Some(&metadata)), None);
    }

    #[test]
    fn read_todo_binding_returns_binding_when_both_present() {
        let metadata = json!({
            "planFile": "my_plan_1234.plan.md",
            "todoId": "setup-auth",
        });
        assert_eq!(
            read_todo_binding(Some(&metadata)),
            Some(("my_plan_1234.plan.md".to_string(), "setup-auth".to_string()))
        );
    }

    #[test]
    fn read_todo_binding_rejects_empty_values() {
        let metadata = json!({ "planFile": "  ", "todoId": "setup-auth" });
        assert_eq!(read_todo_binding(Some(&metadata)), None);
        let metadata = json!({ "planFile": "my_plan.plan.md", "todoId": "" });
        assert_eq!(read_todo_binding(Some(&metadata)), None);
    }

    #[test]
    fn should_auto_complete_todo_only_for_completed_outcomes() {
        assert!(should_auto_complete_todo(&completed_outcome("turn-1")));
        assert!(!should_auto_complete_todo(&TurnOutcome::Cancelled {
            turn_id: "turn-2".to_string()
        }));
        assert!(!should_auto_complete_todo(&TurnOutcome::Failed {
            turn_id: "turn-3".to_string(),
            error: "boom".to_string()
        }));
        assert!(!should_auto_complete_todo(&TurnOutcome::Interrupted {
            turn_id: "turn-4".to_string(),
            execution_generation: 0,
        }));
    }

    #[tokio::test]
    async fn mark_todo_status_updates_frontmatter_in_progress() {
        let dir = std::env::temp_dir().join(format!(
            "g8-plan-todo-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let plan_file = dir.join("my_plan_1234.plan.md");
        let content = "---\nname: my_plan\noverview: plan\ntodos:\n  - id: setup-auth\n    content: setup auth\n    status: pending\n    dependencies: []\n---\n\nBody here\n";
        std::fs::write(&plan_file, content).expect("write plan");

        apply_todo_status_update(&plan_file, "setup-auth", "in_progress")
            .await
            .expect("apply update");

        let updated = std::fs::read_to_string(&plan_file).expect("read plan");
        assert!(updated.contains("status: in_progress"), "updated={updated}");
        assert!(updated.contains("Body here"), "body preserved");
        assert!(
            updated.contains("content: setup auth"),
            "todo content preserved, updated={updated}"
        );
        // Round-trip must preserve the insertion order of each todo item's
        // keys (id, content, status, dependencies) — serde_yaml::Value keeps
        // the mapping order rather than alphabetizing like serde_json::Value
        // would (which reorders to content/dependencies/id/status). Assert the
        // relative position of the todo keys inside the todo block, ignoring
        // the YAML sequence indentation that serde_yaml may reformat.
        let todo_block = updated
            .strip_prefix("---\nname: my_plan\noverview: plan\ntodos:\n")
            .expect("frontmatter opener preserved");
        let id_pos = todo_block.find("id: setup-auth").expect("id key present");
        let content_pos = todo_block.find("content: setup auth").expect("content key present");
        let status_pos = todo_block.find("status: in_progress").expect("status key present");
        let deps_pos = todo_block.find("dependencies: []").expect("dependencies key present");
        assert!(
            id_pos < content_pos && content_pos < status_pos && status_pos < deps_pos,
            "todo key order must be id/content/status/dependencies (not alphabetized), updated={updated}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn mark_todo_status_errors_on_missing_todo() {
        let dir = std::env::temp_dir().join(format!(
            "g8-plan-todo-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let plan_file = dir.join("my_plan_1234.plan.md");
        let content = "---\nname: my_plan\ntodos:\n  - id: setup-auth\n    content: setup auth\n    status: pending\n    dependencies: []\n---\n\nBody\n";
        std::fs::write(&plan_file, content).expect("write plan");

        let result = apply_todo_status_update(&plan_file, "does-not-exist", "in_progress").await;
        assert!(result.is_err(), "missing todo must error");
        std::fs::remove_dir_all(&dir).ok();
    }
}
