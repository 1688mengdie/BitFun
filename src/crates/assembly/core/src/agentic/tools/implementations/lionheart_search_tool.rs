use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use std::{fs, path::PathBuf};

/// Root of the LionHeart knowledge base.
///
/// The library lives outside any workspace, so workspace-bound tools (Grep,
/// Glob) cannot reach it. The root is hard-coded by design: the commander's
/// private library is a fixed machine-local path and must never be deleted or
/// overwritten (LionHeart rule).
const LIONHEART_ROOT: &str = "E:/LionHeart library";

/// Files larger than this are skipped (in bytes).
const MAX_SCAN_FILE_SIZE: u64 = 2 * 1024 * 1024;

/// Deepest directory level the recursive scan descends to (LEGION-09).
///
/// Symlink cycles and pathological nested layouts cannot be expressed with a
/// finite depth cap: the walk stops descending past this level.
const MAX_SCAN_DEPTH: usize = 16;

/// Hard cap on the number of files scanned in one call (LEGION-10).
///
/// A single tool call must never scan an unbounded tree; once the cap is hit
/// the walk stops and reports `file_cap_reached` so the caller can narrow the
/// scope (keyword/scope/max_results) instead of silently truncating.
const MAX_SCANNED_FILES: usize = 100_000;

/// Default result cap.
const DEFAULT_MAX_RESULTS: usize = 50;

/// Hard cap for `max_results`.
const MAX_RESULTS_CAP: usize = 200;

/// LionHeartSearch tool - full-text search over the LionHeart knowledge base.
pub struct LionHeartSearchTool;

impl Default for LionHeartSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl LionHeartSearchTool {
    pub fn new() -> Self {
        Self
    }
}

/// A concrete LionHeart layer. The library has L0/L1/L3/L4 and deliberately no L2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LionHeartLayer {
    L0,
    L1,
    L3,
    L4,
}

impl LionHeartLayer {
    fn as_str(self) -> &'static str {
        match self {
            LionHeartLayer::L0 => "L0",
            LionHeartLayer::L1 => "L1",
            LionHeartLayer::L3 => "L3",
            LionHeartLayer::L4 => "L4",
        }
    }
}

/// Resolved search scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LionHeartScope {
    All,
    Layer(LionHeartLayer),
}

impl LionHeartScope {
    fn root_dir(self) -> PathBuf {
        match self {
            LionHeartScope::All => PathBuf::from(LIONHEART_ROOT),
            LionHeartScope::Layer(layer) => PathBuf::from(LIONHEART_ROOT).join(layer.as_str()),
        }
    }
}

/// Parses the user-facing `scope` string into a concrete search scope.
fn parse_scope(scope: &str) -> Result<LionHeartScope, String> {
    let scope = scope.trim();
    if scope.is_empty() || scope.eq_ignore_ascii_case("all") {
        return Ok(LionHeartScope::All);
    }
    match scope.to_ascii_uppercase().as_str() {
        "L0" => Ok(LionHeartScope::Layer(LionHeartLayer::L0)),
        "L1" => Ok(LionHeartScope::Layer(LionHeartLayer::L1)),
        "L3" => Ok(LionHeartScope::Layer(LionHeartLayer::L3)),
        "L4" => Ok(LionHeartScope::Layer(LionHeartLayer::L4)),
        other => Err(format!(
            "Unsupported scope '{}'. Expected one of: all, L0, L1, L3, L4 (the LionHeart library has no L2 layer)",
            other
        )),
    }
}

/// Tracks what the scan saw so the caller can tell skipped content apart.
#[derive(Debug, Default)]
struct ScanStats {
    scanned_files: usize,
    skipped_binary: usize,
    skipped_oversized: usize,
    skipped_symlinks: usize,
    /// Set when the walk stopped because it hit a hard cap (MAX_SCAN_DEPTH or
    /// MAX_SCANNED_FILES): the scan did not fully cover the requested scope.
    file_cap_reached: bool,
}

/// Recursively searches `dir` for `keyword_lower`, appending matches to `results`.
///
/// `depth` guards against unbounded descent (LEGION-09): each recursion level
/// past `MAX_SCAN_DEPTH` stops the walk. `fs::symlink_metadata` is used so
/// symlinks are never followed — a link pointing outside the library root can
/// never escape the scan scope.
fn search_dir(
    dir: &Path,
    keyword_lower: &str,
    max_results: usize,
    results: &mut Vec<Value>,
    stats: &mut ScanStats,
    depth: usize,
) {
    if results.len() >= max_results {
        return;
    }
    if depth > MAX_SCAN_DEPTH || stats.scanned_files >= MAX_SCANNED_FILES {
        stats.file_cap_reached = true;
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    let mut paths = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    // Deterministic order across runs.
    paths.sort();

    for path in paths {
        if results.len() >= max_results {
            break;
        }
        if stats.scanned_files >= MAX_SCANNED_FILES {
            stats.file_cap_reached = true;
            break;
        }
        let Some(file_name) = path.file_name().map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        // symlink_metadata does not follow links: a symlink to a directory is
        // reported as a symlink, never traversed (LEGION-09).
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let file_type = meta.file_type();
        if file_type.is_symlink() {
            stats.skipped_symlinks += 1;
            continue;
        }
        if file_type.is_dir() {
            if file_name.starts_with('.') {
                // Skip hidden directories (e.g. .git).
                continue;
            }
            search_dir(&path, keyword_lower, max_results, results, stats, depth + 1);
        } else if file_type.is_file() {
            scan_file(&path, keyword_lower, max_results, results, stats);
        }
        // Special files are skipped.
    }
}

/// Scans one text file for `keyword_lower`, appending matches to `results`.
fn scan_file(
    path: &Path,
    keyword_lower: &str,
    max_results: usize,
    results: &mut Vec<Value>,
    stats: &mut ScanStats,
) {
    if results.len() >= max_results {
        return;
    }
    if stats.scanned_files >= MAX_SCANNED_FILES {
        stats.file_cap_reached = true;
        return;
    }
    // symlink_metadata: callers already skip symlinks, but a file that became a
    // symlink between the directory read and this call must not be followed.
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(_) => return,
    };
    if meta.file_type().is_symlink() {
        stats.skipped_symlinks += 1;
        return;
    }
    if meta.len() > MAX_SCAN_FILE_SIZE {
        stats.skipped_oversized += 1;
        return;
    }
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };
    // Heuristic binary detection: a NUL byte in the head of the file.
    let head_len = bytes.len().min(8192);
    if bytes[..head_len].contains(&0) {
        stats.skipped_binary += 1;
        return;
    }
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            stats.skipped_binary += 1;
            return;
        }
    };
    stats.scanned_files += 1;
    for (index, line) in text.lines().enumerate() {
        if results.len() >= max_results {
            break;
        }
        if line.to_lowercase().contains(keyword_lower) {
            results.push(json!({
                "path": path.to_string_lossy(),
                "line": index + 1,
                "line_content": line,
            }));
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct LionHeartSearchInput {
    keyword: String,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    max_results: Option<usize>,
}

#[async_trait]
impl Tool for LionHeartSearchTool {
    fn name(&self) -> &str {
        "LionHeartSearch"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Use this tool when you need to search the LionHeart knowledge base (`E:/LionHeart library`) for skills, rules, and accumulated lessons.

This tool is strictly read-only: it never deletes, overwrites, or modifies anything under the library root (LionHeart rule). It recursively walks the requested scope, scans UTF-8 text files for the keyword (case-insensitive), and returns every matching line.

`keyword` (required): the text to search for, matched case-insensitively against file contents.

`scope` (defaults to "all"):
- "all": the whole library root
- "L0": the top-level soul layer (LionHeart.md, chronicles, identities, etc.)
- "L1": skills / rules / tooling library
- "L3": refined prompts and knowledge layers
- "L4": archived or supplementary layers
Note: the library has L0/L1/L3/L4 and deliberately no L2 layer.

`max_results` (defaults to 50, capped at 200): maximum number of matching lines to return.

Non-text files, binary files, files larger than 2MB, hidden directories (e.g. .git), and symlinks are skipped; the walk stops at 16 directory levels or after 100k scanned files. The result includes `scanned_files`, `skipped_binary`, `skipped_oversized`, `skipped_symlinks`, and `file_cap_reached` counters so you can tell what was and was not searched.

Each match has the shape {path, line, line_content}, where `line` is the 1-based line number.

Examples:
1. Search the whole library for "S-31": keyword="S-31"
2. Search only the skills layer for "from-zero": keyword="from-zero", scope="L1"
3. Search the top layer with a tight cap: keyword="LionHeart", scope="L0", max_results=20"#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Search the LionHeart knowledge base (L0/L1/L3/L4) by keyword. Strictly read-only."
            .to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        // Mirrors the plan tool family calibration: commander staples stay
        // Direct so no GetToolSpec unlock round-trip is needed.
        ToolExposure::Direct
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "keyword": {
                    "type": "string",
                    "description": "Keyword to search for, matched case-insensitively against file contents. Required."
                },
                "scope": {
                    "type": "string",
                    "description": "Search scope. One of: all (default), L0, L1, L3, L4."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matching lines to return. Defaults to 50, capped at 200."
                }
            },
            "required": ["keyword"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: LionHeartSearchInput = match serde_json::from_value(input.clone()) {
            Ok(value) => value,
            Err(err) => {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid input: {}", err)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        if parsed.keyword.trim().is_empty() {
            return ValidationResult {
                result: false,
                message: Some("keyword must be a non-empty string".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if let Some(scope) = parsed.scope.as_deref() {
            if let Err(message) = parse_scope(scope) {
                return ValidationResult {
                    result: false,
                    message: Some(message),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        if let Some(max_results) = parsed.max_results {
            if !(1..=MAX_RESULTS_CAP).contains(&max_results) {
                return ValidationResult {
                    result: false,
                    message: Some(format!("max_results must be between 1 and {}", MAX_RESULTS_CAP)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        ValidationResult::default()
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let keyword = input
            .get("keyword")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let scope = input
            .get("scope")
            .and_then(|value| value.as_str())
            .unwrap_or("all");
        format!("Search LionHeart library for '{}' (scope '{}')", keyword, scope)
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: LionHeartSearchInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;

        let keyword = params.keyword.trim();
        if keyword.is_empty() {
            return Err(BitFunError::tool("keyword must not be empty"));
        }
        let scope = params.scope.as_deref().unwrap_or("all");
        let resolved = parse_scope(scope)
            .map_err(|message| BitFunError::tool(format!("Invalid scope: {}", message)))?;
        let max_results = params
            .max_results
            .unwrap_or(DEFAULT_MAX_RESULTS)
            .clamp(1, MAX_RESULTS_CAP);

        let root = resolved.root_dir();
        if !root.is_dir() {
            return Err(BitFunError::tool(format!(
                "LionHeart library root does not exist: {}",
                root.to_string_lossy()
            )));
        }

        let keyword_lower = keyword.to_lowercase();
        // LEGION-10: the recursive scan is CPU/IO-bound and unbounded in the
        // worst case (the whole library). Run it on the blocking pool so a
        // large scan never stalls the async executor, and return the capped
        // results/stats instead of mutating shared state across the await.
        let (results, stats) = tokio::task::spawn_blocking(move || {
            let mut results = Vec::new();
            let mut stats = ScanStats::default();
            search_dir(&root, &keyword_lower, max_results, &mut results, &mut stats, 0);
            (results, stats)
        })
        .await
        .map_err(|e| BitFunError::tool(format!("LionHeart search worker failed: {}", e)))?;

        Ok(vec![ToolResult::Result {
            data: json!({
                "success": true,
                "scope": scope,
                "keyword": keyword,
                "count": results.len(),
                "scanned_files": stats.scanned_files,
                "skipped_binary": stats.skipped_binary,
                "skipped_oversized": stats.skipped_oversized,
                "skipped_symlinks": stats.skipped_symlinks,
                "file_cap_reached": stats.file_cap_reached,
                "matches": results,
            }),
            result_for_assistant: Some(format!(
                "Searched the LionHeart library with scope '{}': {} match(es) across {} scanned file(s){}.",
                scope,
                stats.scanned_files,
                results.len(),
                if stats.file_cap_reached {
                    " (file cap reached; narrow the scope or keyword to scan more)"
                } else {
                    ""
                }
            )),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scope_accepts_default_and_known_scopes() {
        assert_eq!(parse_scope(""), Ok(LionHeartScope::All));
        assert_eq!(parse_scope("all"), Ok(LionHeartScope::All));
        assert_eq!(parse_scope("ALL"), Ok(LionHeartScope::All));
        assert_eq!(
            parse_scope("L0"),
            Ok(LionHeartScope::Layer(LionHeartLayer::L0))
        );
        assert_eq!(
            parse_scope("l1"),
            Ok(LionHeartScope::Layer(LionHeartLayer::L1))
        );
        assert_eq!(
            parse_scope("L3"),
            Ok(LionHeartScope::Layer(LionHeartLayer::L3))
        );
        assert_eq!(
            parse_scope("L4"),
            Ok(LionHeartScope::Layer(LionHeartLayer::L4))
        );
    }

    #[test]
    fn parse_scope_rejects_unknown_scopes() {
        assert!(parse_scope("unknown").is_err());
        // The library has L0/L1/L3/L4 and deliberately no L2 layer.
        assert!(parse_scope("L2").is_err());
        assert!(parse_scope("l2").is_err());
        assert!(parse_scope("by_status:all").is_err());
    }

    #[tokio::test]
    async fn validate_rejects_missing_or_empty_keyword() {
        let tool = LionHeartSearchTool::new();

        let validation = tool.validate_input(&json!({}), None).await;
        assert!(!validation.result);
        assert_eq!(validation.error_code, Some(400));

        let validation = tool
            .validate_input(&json!({ "keyword": "  " }), None)
            .await;
        assert!(!validation.result);
        assert_eq!(validation.error_code, Some(400));
    }

    #[tokio::test]
    async fn validate_rejects_unknown_scope() {
        let tool = LionHeartSearchTool::new();

        let validation = tool
            .validate_input(&json!({ "keyword": "LionHeart", "scope": "L2" }), None)
            .await;

        assert!(!validation.result);
        assert_eq!(validation.error_code, Some(400));
    }

    #[tokio::test]
    async fn validate_rejects_excessive_max_results() {
        let tool = LionHeartSearchTool::new();

        let validation = tool
            .validate_input(&json!({ "keyword": "LionHeart", "max_results": 201 }), None)
            .await;

        assert!(!validation.result);
        assert_eq!(validation.error_code, Some(400));
    }

    #[tokio::test]
    async fn validate_accepts_valid_input() {
        let tool = LionHeartSearchTool::new();

        let validation = tool
            .validate_input(
                &json!({ "keyword": "LionHeart", "scope": "L0", "max_results": 10 }),
                None,
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[cfg(unix)]
    fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[test]
    fn search_dir_skips_symlinks_outside_root() {
        // LEGION-09: a symlink pointing outside the library root must never be
        // followed. Symlink creation needs privileges on Windows, so the
        // assertion is skipped when the OS refuses to create the link.
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(&root).expect("create root");
        std::fs::write(root.join("a.txt"), "lionheart keyword\n").expect("write file");

        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&outside).expect("create outside dir");
        std::fs::write(outside.join("secret.txt"), "lionheart secret\n").expect("write secret");
        let link = root.join("link-to-outside");
        if make_symlink(&outside, &link).is_ok() {
            let mut results = Vec::new();
            let mut stats = ScanStats::default();
            search_dir(&root, "lionheart", 50, &mut results, &mut stats, 0);
            assert!(
                results
                    .iter()
                    .all(|result| !result["path"].to_string().contains("secret")),
                "files reached through a symlink must not be searched"
            );
            assert_eq!(stats.skipped_symlinks, 1);
        }
    }

    #[test]
    fn search_dir_stops_at_depth_cap() {
        // LEGION-09: the walk must not descend past MAX_SCAN_DEPTH, so a deeply
        // nested layout cannot blow up the scan.
        let temp = tempfile::tempdir().expect("tempdir");
        let mut dir = temp.path().join("root");
        std::fs::create_dir_all(&dir).expect("create root");
        for _ in 0..MAX_SCAN_DEPTH + 1 {
            dir = dir.join("nested");
        }
        std::fs::create_dir_all(&dir).expect("create nested chain");
        std::fs::write(dir.join("deep.txt"), "lionheart deep keyword\n").expect("write deep file");

        let mut results = Vec::new();
        let mut stats = ScanStats::default();
        search_dir(
            &temp.path().join("root"),
            "lionheart",
            50,
            &mut results,
            &mut stats,
            0,
        );
        assert_eq!(stats.file_cap_reached, true);
        assert!(
            results.iter().all(|result| !result["path"].to_string().contains("deep")),
            "files deeper than MAX_SCAN_DEPTH must not be searched"
        );
    }
}
