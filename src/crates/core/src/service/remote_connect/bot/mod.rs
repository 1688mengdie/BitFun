//! Bot integration for Remote Connect.
//!
//! Supports Feishu and Telegram bots as relay channels.
//! Shared command logic lives in `command_router`; platform-specific
//! I/O is handled by `telegram` and `feishu`.

pub mod command_router;
pub mod feishu;
pub mod telegram;

use serde::{Deserialize, Serialize};

pub use command_router::{BotChatState, HandleResult, ForwardRequest, ForwardedTurnResult};

/// Configuration for a bot-based connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "bot_type", rename_all = "snake_case")]
pub enum BotConfig {
    Feishu {
        app_id: String,
        app_secret: String,
    },
    Telegram {
        bot_token: String,
    },
}

/// Pairing state for bot-based connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotPairingInfo {
    pub pairing_code: String,
    pub bot_type: String,
    pub bot_link: String,
    pub expires_at: i64,
}

/// Persisted bot connection — saved to disk so reconnect survives restarts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedBotConnection {
    pub bot_type: String,
    pub chat_id: String,
    pub config: BotConfig,
    pub chat_state: BotChatState,
    pub connected_at: i64,
}

/// All persisted bot connections (one per bot type at most).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BotPersistenceData {
    pub connections: Vec<SavedBotConnection>,
}

impl BotPersistenceData {
    pub fn upsert(&mut self, conn: SavedBotConnection) {
        self.connections.retain(|c| c.bot_type != conn.bot_type);
        self.connections.push(conn);
    }

    pub fn remove(&mut self, bot_type: &str) {
        self.connections.retain(|c| c.bot_type != bot_type);
    }

    pub fn get(&self, bot_type: &str) -> Option<&SavedBotConnection> {
        self.connections.iter().find(|c| c.bot_type == bot_type)
    }
}

// ── Shared workspace-file utilities ────────────────────────────────

/// File content read from the local workspace, ready to be sent over any channel.
pub struct WorkspaceFileContent {
    pub name: String,
    pub bytes: Vec<u8>,
    pub mime_type: &'static str,
    pub size: u64,
}

/// Resolve a raw path (with or without `computer://` prefix) to an absolute
/// `PathBuf`.  Relative paths are joined with the current workspace root.
/// Returns `None` when a relative path is given but no workspace is open.
pub fn resolve_workspace_path(raw: &str) -> Option<std::path::PathBuf> {
    use crate::infrastructure::get_workspace_path;

    let stripped = raw.strip_prefix("computer://").unwrap_or(raw);

    if stripped.starts_with('/')
        || (stripped.len() >= 3 && stripped.as_bytes()[1] == b':')
    {
        Some(std::path::PathBuf::from(stripped))
    } else if let Some(ws) = get_workspace_path() {
        Some(ws.join(stripped))
    } else {
        None
    }
}

/// Return the best-effort MIME type for a file based on its extension.
pub fn detect_mime_type(path: &std::path::Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" | "log" => "text/plain",
        "md" => "text/markdown",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "ts" | "tsx" | "jsx" | "rs" | "py" | "go" | "java" | "c" | "cpp" | "h" | "sh"
        | "toml" | "yaml" | "yml" => "text/plain",
        "json" => "application/json",
        "xml" => "application/xml",
        "csv" => "text/csv",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "zip" => "application/zip",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        }
        "mp4" => "video/mp4",
        "opus" => "audio/opus",
        _ => "application/octet-stream",
    }
}

/// Read a workspace file, resolving `computer://` prefixes and relative paths.
///
/// `max_size` is the caller-specific byte limit (e.g. 50 MB for Telegram,
/// 30 MB for Feishu, 10 MB for mobile relay).
///
/// Returns an error when the file is missing, is a directory, or exceeds
/// `max_size`.
pub async fn read_workspace_file(
    raw_path: &str,
    max_size: u64,
) -> anyhow::Result<WorkspaceFileContent> {
    let abs_path = resolve_workspace_path(raw_path)
        .ok_or_else(|| anyhow::anyhow!("No workspace open to resolve path: {raw_path}"))?;

    if !abs_path.exists() {
        return Err(anyhow::anyhow!("File not found: {}", abs_path.display()));
    }
    if !abs_path.is_file() {
        return Err(anyhow::anyhow!(
            "Path is not a regular file: {}",
            abs_path.display()
        ));
    }

    let metadata = tokio::fs::metadata(&abs_path).await.map_err(|e| {
        anyhow::anyhow!("Cannot read file metadata for {}: {e}", abs_path.display())
    })?;

    if metadata.len() > max_size {
        return Err(anyhow::anyhow!(
            "File too large ({} bytes, limit {max_size} bytes): {}",
            metadata.len(),
            abs_path.display()
        ));
    }

    let bytes = tokio::fs::read(&abs_path)
        .await
        .map_err(|e| anyhow::anyhow!("Cannot read file {}: {e}", abs_path.display()))?;

    let name = abs_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();

    let mime_type = detect_mime_type(&abs_path);

    Ok(WorkspaceFileContent {
        name,
        bytes,
        mime_type,
        size: metadata.len(),
    })
}

/// Get file metadata (name and size in bytes) without reading the full content.
/// Returns `None` if the path cannot be resolved, does not exist, or is not a
/// regular file.
pub fn get_file_metadata(raw_path: &str) -> Option<(String, u64)> {
    let abs = resolve_workspace_path(raw_path)?;
    if !abs.is_file() {
        return None;
    }
    let name = abs
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let size = std::fs::metadata(&abs).ok()?.len();
    Some((name, size))
}

/// Format a byte count as a human-readable string (e.g. "1.4 MB", "320 KB").
pub fn format_file_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{bytes} B")
    }
}

// ── computer:// link extraction ────────────────────────────────────

/// Extract local file paths referenced via `computer://` links in `text`.
///
/// Relative paths (e.g. `computer://artifacts/report.docx`) are resolved
/// against `workspace_path` when provided.  Only paths that exist as regular
/// files on disk are returned; directories and missing paths are skipped.
/// Duplicate paths are deduplicated before returning.
pub fn extract_computer_file_paths(text: &str) -> Vec<String> {
    const PREFIX: &str = "computer://";
    let mut paths: Vec<String> = Vec::new();
    let mut search = text;

    while let Some(idx) = search.find(PREFIX) {
        let rest = &search[idx + PREFIX.len()..];

        // Collect the path until whitespace or link-terminating punctuation.
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '(' | ')' | '"' | '\''))
            .unwrap_or(rest.len());

        // Strip trailing punctuation that is unlikely to be part of a path.
        let raw_suffix = rest[..end]
            .trim_end_matches(|c: char| matches!(c, '.' | ',' | ';' | ':' | ')' | ']'));

        if !raw_suffix.is_empty() {
            // Reconstruct the full computer:// URL for resolve_workspace_path
            let raw = format!("{PREFIX}{raw_suffix}");
            if let Some(abs) = resolve_workspace_path(&raw) {
                let abs_str = abs.to_string_lossy().into_owned();
                if abs.exists() && abs.is_file() && !paths.contains(&abs_str) {
                    paths.push(abs_str);
                }
            }
        }

        search = &rest[end..];
    }

    paths
}

const BOT_PERSISTENCE_FILENAME: &str = "bot_connections.json";

pub fn bot_persistence_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".bitfun").join(BOT_PERSISTENCE_FILENAME))
}

pub fn load_bot_persistence() -> BotPersistenceData {
    let Some(path) = bot_persistence_path() else {
        return BotPersistenceData::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => BotPersistenceData::default(),
    }
}

pub fn save_bot_persistence(data: &BotPersistenceData) {
    let Some(path) = bot_persistence_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(data) {
        if let Err(e) = std::fs::write(&path, json) {
            log::error!("Failed to save bot persistence: {e}");
        }
    }
}
