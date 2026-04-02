use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use globset::{GlobBuilder, GlobMatcher};
use ignore::WalkBuilder;
use log::warn;
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Eq, PartialEq)]
struct GlobCandidate {
    depth: usize,
    path: String,
}

impl Ord for GlobCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.depth
            .cmp(&other.depth)
            .then_with(|| self.path.cmp(&other.path))
    }
}

impl PartialOrd for GlobCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn extract_glob_base_directory(pattern: &str) -> (String, String) {
    let glob_start = pattern.find(['*', '?', '[', '{']);

    match glob_start {
        Some(index) => {
            let static_prefix = &pattern[..index];
            let last_separator = static_prefix
                .char_indices()
                .rev()
                .find(|(_, ch)| *ch == '/' || *ch == '\\')
                .map(|(idx, _)| idx);

            if let Some(separator_index) = last_separator {
                (
                    static_prefix[..separator_index].to_string(),
                    pattern[separator_index + 1..].to_string(),
                )
            } else {
                (String::new(), pattern.to_string())
            }
        }
        None => {
            let trimmed = pattern.trim_end_matches(['/', '\\']);
            let literal_path = Path::new(trimmed);
            let base_dir = literal_path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty() && *parent != Path::new("."))
                .map(|parent| parent.to_string_lossy().to_string())
                .unwrap_or_default();
            let file_name = literal_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| trimmed.to_string());

            let relative_pattern = if pattern.ends_with('/') || pattern.ends_with('\\') {
                format!("{}/", file_name)
            } else {
                file_name
            };

            (base_dir, relative_pattern)
        }
    }
}

fn normalize_path(path: &Path) -> String {
    dunce::simplified(path).to_string_lossy().replace('\\', "/")
}

fn is_safe_relative_subpath(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn derive_walk_root(search_path_abs: &Path, pattern: &str) -> (PathBuf, String) {
    let (base_dir, relative_pattern) = extract_glob_base_directory(pattern);
    let base_path = Path::new(&base_dir);

    if base_dir.is_empty() || !is_safe_relative_subpath(base_path) {
        return (search_path_abs.to_path_buf(), pattern.to_string());
    }

    let walk_root = search_path_abs.join(base_path);
    if walk_root.starts_with(search_path_abs) {
        (walk_root, relative_pattern)
    } else {
        (search_path_abs.to_path_buf(), pattern.to_string())
    }
}

fn match_relative_path(matcher: &GlobMatcher, relative_path: &str, is_dir: bool) -> bool {
    if is_dir {
        matcher.is_match(relative_path) || matcher.is_match(&format!("{}/", relative_path))
    } else {
        matcher.is_match(relative_path)
    }
}

pub fn glob_with_ignore(
    search_path: &str,
    pattern: &str,
    ignore: bool,
    ignore_hidden: bool,
    limit: usize,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let path = std::path::Path::new(search_path);
    if !path.exists() {
        return Err(format!("Search path '{}' does not exist", search_path).into());
    }
    if !path.is_dir() {
        return Err(format!("Search path '{}' is not a directory", search_path).into());
    }

    let search_path_abs = dunce::canonicalize(Path::new(search_path))?;
    let (walk_root, relative_pattern) = derive_walk_root(&search_path_abs, pattern);

    if !walk_root.exists() || !walk_root.is_dir() || limit == 0 {
        return Ok(Vec::new());
    }

    let glob = GlobBuilder::new(&relative_pattern)
        .literal_separator(true)
        .build()?
        .compile_matcher();

    let walker = WalkBuilder::new(&walk_root)
        .ignore(ignore)
        .git_ignore(ignore)
        .git_global(ignore)
        .git_exclude(ignore)
        .hidden(ignore_hidden)
        .build();

    let mut best_matches = BinaryHeap::with_capacity(limit.saturating_add(1));

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warn!("Glob walker entry error (skipped): {}", err);
                continue;
            }
        };
        let path = entry.path().to_path_buf();
        let relative_path = match path.strip_prefix(&walk_root) {
            Ok(relative) => relative,
            Err(_) => continue,
        };
        let relative_path = normalize_path(relative_path);

        if match_relative_path(
            &glob,
            &relative_path,
            entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false),
        ) {
            let normalized_path = normalize_path(&path);
            let candidate = GlobCandidate {
                depth: normalized_path.split('/').count(),
                path: normalized_path,
            };

            if best_matches.len() < limit {
                best_matches.push(candidate);
            } else if let Some(worst_match) = best_matches.peek() {
                if candidate < *worst_match {
                    best_matches.pop();
                    best_matches.push(candidate);
                }
            }
        }
    }

    let mut results = best_matches
        .into_sorted_vec()
        .into_iter()
        .map(|candidate| candidate.path)
        .collect::<Vec<_>>();
    results.sort();
    Ok(results)
}

fn limit_paths(paths: &[String], limit: usize) -> Vec<String> {
    let mut depth_and_paths = paths
        .iter()
        .map(|path| {
            let normalized_path = path.replace('\\', "/");
            let depth = normalized_path.split('/').count();
            (depth, normalized_path)
        })
        .collect::<Vec<_>>();
    depth_and_paths.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

    let mut result = depth_and_paths
        .into_iter()
        .take(limit)
        .map(|(_, path)| path)
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn call_glob(search_path: &str, pattern: &str, limit: usize) -> Result<Vec<String>, String> {
    let is_whitelisted = pattern.starts_with(".bitfun")
        || pattern.contains("/.bitfun")
        || pattern.contains("\\.bitfun");

    let apply_gitignore = !is_whitelisted;
    let ignore_hidden_files = !is_whitelisted;

    glob_with_ignore(
        search_path,
        pattern,
        apply_gitignore,
        ignore_hidden_files,
        limit,
    )
    .map_err(|e| e.to_string())
}

fn build_remote_find_command(search_dir: &str, pattern: &str, limit: usize) -> String {
    let search_dir_path = Path::new(search_dir);
    let (remote_walk_root, remote_pattern) = derive_walk_root(search_dir_path, pattern);

    let name_pattern = if remote_pattern.contains("**/") {
        remote_pattern.replacen("**/", "", 1)
    } else if remote_pattern.contains('/') || remote_pattern.contains('\\') {
        "*".to_string()
    } else {
        remote_pattern
    };

    let escaped_dir = remote_walk_root.to_string_lossy().replace('\'', "'\\''");
    let escaped_pattern = name_pattern.replace('\'', "'\\''");

    format!(
        "find '{}' -maxdepth 10 -name '{}' -not -path '*/.git/*' -not -path '*/node_modules/*' 2>/dev/null | head -n {}",
        escaped_dir, escaped_pattern, limit
    )
}

pub struct GlobTool;

impl GlobTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "Glob"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Fast file pattern matching tool support Standard Unix-style glob syntax
- Supports glob patterns like "**/*.js" or "src/**/*.ts"
- Returns matching file paths
- Use this tool when you need to find files by name patterns
- You can call multiple tools in a single response. It is always better to speculatively perform multiple searches in parallel if they are potentially useful.
<example>
- List files and directories in path: path = "/path/to/search", pattern = "*"
- Search all markdown files in path recursively: path = "/path/to/search", pattern = "**/*.md"
</example>
"#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match files against (relative to `path`)"
                },
                "path": {
                    "type": "string",
                    "description": "The directory to search in. If not specified, the current working directory will be used. IMPORTANT: Omit this field to use the default directory. DO NOT enter \"undefined\" or \"null\" - simply omit it for the default behavior. Must be a valid absolute path if provided."
                },
                "limit": {
                    "type": "number",
                    "description": "The maximum number of entries to return. Defaults to 100."
                }
            },
            "required": ["pattern"]
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

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("pattern is required".to_string()))?;

        let resolved_str = match input.get("path").and_then(|v| v.as_str()) {
            Some(user_path) => context.resolve_workspace_tool_path(user_path)?,
            None => context
                .workspace
                .as_ref()
                .map(|w| w.root_path_string())
                .ok_or_else(|| {
                    BitFunError::tool(
                        "workspace_path is required when Glob path is omitted".to_string(),
                    )
                })?,
        };

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(100);

        // Remote workspace: use `find` via the workspace shell
        if context.is_remote() {
            let ws_shell = context
                .ws_shell()
                .ok_or_else(|| BitFunError::tool("Workspace shell not available".to_string()))?;

            let search_dir = resolved_str.clone();
            let find_cmd = build_remote_find_command(&search_dir, pattern, limit);

            let (stdout, _stderr, _exit_code) = ws_shell
                .exec(&find_cmd, Some(30_000))
                .await
                .map_err(|e| BitFunError::tool(format!("Failed to glob on remote: {}", e)))?;

            let matches: Vec<String> = stdout
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect();
            let limited = limit_paths(&matches, limit);
            let result_text = if limited.is_empty() {
                format!("No files found matching pattern '{}'", pattern)
            } else {
                limited.join("\n")
            };

            return Ok(vec![ToolResult::Result {
                data: json!({
                    "pattern": pattern,
                    "path": search_dir,
                    "matches": limited,
                    "match_count": limited.len()
                }),
                result_for_assistant: Some(result_text),
                image_attachments: None,
            }]);
        }

        let matches = call_glob(&resolved_str, pattern, limit).map_err(|e| BitFunError::tool(e))?;

        let result_text = if matches.is_empty() {
            format!("No files found matching pattern '{}'", pattern)
        } else {
            matches.join("\n")
        };

        let result = ToolResult::Result {
            data: json!({
                "pattern": pattern,
                "path": resolved_str,
                "matches": matches,
                "match_count": matches.len()
            }),
            result_for_assistant: Some(result_text),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}

#[cfg(test)]
mod tests {
    use super::{call_glob, derive_walk_root, extract_glob_base_directory};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("bitfun-glob-tool-{name}-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn extracts_static_glob_prefix() {
        assert_eq!(
            extract_glob_base_directory("src/**/*.rs"),
            ("src".to_string(), "**/*.rs".to_string())
        );
        assert_eq!(
            extract_glob_base_directory("*.rs"),
            (String::new(), "*.rs".to_string())
        );
        assert_eq!(
            extract_glob_base_directory("src/lib.rs"),
            ("src".to_string(), "lib.rs".to_string())
        );
    }

    #[test]
    fn does_not_expand_walk_root_outside_search_path() {
        let root = std::env::temp_dir().join("bitfun-glob-root");
        let (walk_root, relative_pattern) = derive_walk_root(&root, "../*.rs");

        assert_eq!(walk_root, root);
        assert_eq!(relative_pattern, "../*.rs".to_string());
    }

    #[test]
    fn keeps_shallowest_matches_without_collecting_everything() {
        let root = make_temp_dir("limit");
        fs::create_dir_all(root.join("src/deep")).unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(root.join("Cargo.toml"), "").unwrap();
        fs::write(root.join("src/lib.rs"), "").unwrap();
        fs::write(root.join("src/deep/mod.rs"), "").unwrap();
        fs::write(root.join("tests/mod.rs"), "").unwrap();

        let matches = call_glob(root.to_string_lossy().as_ref(), "**/*.rs", 2).unwrap();

        assert_eq!(matches.len(), 2);
        assert!(matches.iter().any(|path| path.ends_with("/src/lib.rs")));
        assert!(matches.iter().any(|path| path.ends_with("/tests/mod.rs")));
        assert!(!matches
            .iter()
            .any(|path| path.ends_with("/src/deep/mod.rs")));

        let _ = fs::remove_dir_all(root);
    }
}
