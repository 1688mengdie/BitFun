use crate::infrastructure::{FileSearchOutcome, FileSearchResult, SearchMatchType};
use crate::service::config::{types::WorkspaceConfig, ConfigService};
use crate::service::remote_ssh::workspace_state::{
    lookup_remote_connection, lookup_remote_connection_with_hint, RemoteWorkspaceEntry,
};
use crate::service::remote_ssh::{
    normalize_remote_workspace_path, RemoteFileService, SSHConnectionManager,
};
use crate::service::search::{
    ContentSearchOutputMode, ContentSearchRequest, ContentSearchResult, IndexTaskHandle,
    WorkspaceIndexStatus, WorkspaceSearchBackend, WorkspaceSearchContextLine,
    WorkspaceSearchDirtyFiles, WorkspaceSearchFileCount, WorkspaceSearchHit, WorkspaceSearchLine,
    WorkspaceSearchMatch, WorkspaceSearchMatchLocation, WorkspaceSearchOverlayStatus,
    WorkspaceSearchRepoPhase, WorkspaceSearchRepoStatus, WorkspaceSearchTaskKind,
    WorkspaceSearchTaskPhase, WorkspaceSearchTaskState, WorkspaceSearchTaskStatus,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

const REMOTE_FLASHGREP_INSTALL_DIR: &str = ".bitfun/bin";
const REMOTE_FLASHGREP_STATE_FILE_NAME: &str = "daemon-state.json";
const REMOTE_FLASHGREP_LOG_FILE_NAME: &str = "daemon.log";
const REMOTE_FLASHGREP_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const REMOTE_FLASHGREP_STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(200);
const REMOTE_OS_PROBES: &[&str] = &["uname -s", "sh -c 'uname -s 2>/dev/null'"];
const REMOTE_ARCHITECTURE_PROBES: &[&str] = &[
    "uname -m",
    "arch",
    "sh -c 'uname -m 2>/dev/null || arch 2>/dev/null'",
];
const LINUX_X86_64_FLASHGREP_BUNDLES: &[&str] = &[
    "flashgrep-x86_64-unknown-linux-musl",
    "flashgrep-x86_64-unknown-linux-gnu",
];
const LINUX_AARCH64_FLASHGREP_BUNDLES: &[&str] = &[
    "flashgrep-aarch64-unknown-linux-musl",
    "flashgrep-aarch64-unknown-linux-gnu",
];

#[derive(Clone)]
pub struct RemoteWorkspaceSearchService {
    ssh_manager: SSHConnectionManager,
    remote_file_service: RemoteFileService,
    config_service: Arc<ConfigService>,
    preferred_connection_id: Option<String>,
}

#[derive(Debug, Clone)]
struct RemoteSearchContext {
    connection: RemoteWorkspaceEntry,
    binary_path: String,
    repo_root: String,
    storage_root: String,
    state_file: String,
}

#[derive(Debug, Clone)]
struct OpenedRemoteRepo {
    daemon_addr: String,
    repo_id: String,
    repo_status: WorkspaceSearchRepoStatus,
}

#[derive(Debug, Deserialize)]
struct RemoteDaemonStateFile {
    addr: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RemoteDaemonResponse {
    RepoOpened {
        repo_id: String,
        status: RemoteRepoStatus,
    },
    RepoBaseSnapshotBuilt {
        indexed_docs: usize,
        status: RemoteRepoStatus,
    },
    RepoBaseSnapshotRebuilt {
        indexed_docs: usize,
        status: RemoteRepoStatus,
    },
    TaskStarted {
        task: RemoteTaskStatus,
    },
    SearchCompleted {
        backend: RemoteSearchBackend,
        status: RemoteRepoStatus,
        results: RemoteSearchResults,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RemoteSearchBackend {
    IndexedSnapshot,
    IndexedClean,
    IndexedWorkspaceView,
    RgFallback,
    ScanFallback,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RemoteRepoPhase {
    Opening,
    MissingBaseSnapshot,
    BuildingBaseSnapshot,
    ReadyClean,
    ReadyDirty,
    RebuildingBaseSnapshot,
    Degraded,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RemoteTaskKind {
    BuildBaseSnapshot,
    RebuildBaseSnapshot,
    RefreshWorkspace,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RemoteTaskState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RemoteTaskPhase {
    Scanning,
    Tokenizing,
    Writing,
    Finalizing,
    RefreshingOverlay,
}

#[derive(Debug, Clone, Deserialize)]
struct RemoteDirtyFileStats {
    modified: usize,
    deleted: usize,
    new: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct RemoteWorkspaceOverlayStatus {
    committed_seq_no: u64,
    last_seq_no: u64,
    uncommitted_ops: u64,
    pending_docs: usize,
    active_segments: usize,
    active_delete_segments: usize,
    merge_requested: bool,
    merge_running: bool,
    merge_attempts: u64,
    merge_completed: u64,
    merge_failed: u64,
    last_merge_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RemoteRepoStatus {
    repo_id: String,
    repo_path: String,
    storage_root: String,
    base_snapshot_root: String,
    workspace_overlay_root: String,
    phase: RemoteRepoPhase,
    snapshot_key: Option<String>,
    last_probe_unix_secs: Option<u64>,
    last_rebuild_unix_secs: Option<u64>,
    dirty_files: RemoteDirtyFileStats,
    rebuild_recommended: bool,
    active_task_id: Option<String>,
    probe_healthy: bool,
    last_error: Option<String>,
    overlay: Option<RemoteWorkspaceOverlayStatus>,
}

#[derive(Debug, Clone, Deserialize)]
struct RemoteTaskStatus {
    task_id: String,
    workspace_id: String,
    kind: RemoteTaskKind,
    state: RemoteTaskState,
    phase: Option<RemoteTaskPhase>,
    message: String,
    processed: usize,
    total: Option<usize>,
    started_unix_secs: u64,
    updated_unix_secs: u64,
    finished_unix_secs: Option<u64>,
    cancellable: bool,
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RemoteSearchResults {
    candidate_docs: usize,
    matched_lines: usize,
    matched_occurrences: usize,
    #[serde(default)]
    file_counts: Vec<RemoteFileCount>,
    #[serde(default)]
    hits: Vec<RemoteSearchHit>,
}

#[derive(Debug, Clone, Deserialize)]
struct RemoteFileCount {
    path: String,
    matched_lines: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct RemoteSearchHit {
    path: String,
    matches: Vec<RemoteFileMatch>,
    lines: Vec<RemoteSearchLine>,
}

#[derive(Debug, Clone, Deserialize)]
struct RemoteFileMatch {
    location: RemoteMatchLocation,
    snippet: String,
    matched_text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RemoteMatchLocation {
    line: usize,
    column: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RemoteSearchLine {
    Match { value: RemoteFileMatch },
    Context { line_number: usize, snippet: String },
    ContextBreak,
}

impl RemoteWorkspaceSearchService {
    pub fn new(
        ssh_manager: SSHConnectionManager,
        remote_file_service: RemoteFileService,
        config_service: Arc<ConfigService>,
    ) -> Self {
        Self {
            ssh_manager,
            remote_file_service,
            config_service,
            preferred_connection_id: None,
        }
    }

    pub fn with_preferred_connection_id(mut self, preferred_connection_id: Option<String>) -> Self {
        self.preferred_connection_id = preferred_connection_id;
        self
    }

    pub async fn get_index_status(&self, root_path: &str) -> Result<WorkspaceIndexStatus, String> {
        let context = self.ensure_remote_search_context(root_path).await?;
        let opened = self
            .open_remote_repo(&context, self.max_file_size().await)
            .await?;
        Ok(WorkspaceIndexStatus {
            active_task: synthesize_active_task(&opened.repo_status),
            repo_status: opened.repo_status,
        })
    }

    pub async fn build_index(&self, root_path: &str) -> Result<IndexTaskHandle, String> {
        let context = self.ensure_remote_search_context(root_path).await?;
        let opened = self
            .open_remote_repo(&context, self.max_file_size().await)
            .await?;
        let response = self
            .execute_flashgrep_json(
                &context.connection.connection_id,
                &format!(
                    "{} daemon build --addr {} --repo-id {}",
                    shell_escape(&context.binary_path),
                    shell_escape(&opened.daemon_addr),
                    shell_escape(&opened.repo_id),
                ),
            )
            .await?;
        match response {
            RemoteDaemonResponse::TaskStarted { task } => {
                let refreshed = self
                    .open_remote_repo(&context, self.max_file_size().await)
                    .await
                    .unwrap_or(opened);
                Ok(IndexTaskHandle {
                    task: task.into(),
                    repo_status: refreshed.repo_status,
                })
            }
            RemoteDaemonResponse::RepoBaseSnapshotBuilt {
                indexed_docs,
                status,
            } => {
                let repo_status: WorkspaceSearchRepoStatus = status.into();
                Ok(IndexTaskHandle {
                    task: completed_remote_index_task(
                        &repo_status,
                        WorkspaceSearchTaskKind::Build,
                        indexed_docs,
                    ),
                    repo_status,
                })
            }
            _ => Err("Unexpected flashgrep response while starting remote build".to_string()),
        }
    }

    pub async fn rebuild_index(&self, root_path: &str) -> Result<IndexTaskHandle, String> {
        let context = self.ensure_remote_search_context(root_path).await?;
        let opened = self
            .open_remote_repo(&context, self.max_file_size().await)
            .await?;
        let response = self
            .execute_flashgrep_json(
                &context.connection.connection_id,
                &format!(
                    "{} daemon rebuild --addr {} --repo-id {}",
                    shell_escape(&context.binary_path),
                    shell_escape(&opened.daemon_addr),
                    shell_escape(&opened.repo_id),
                ),
            )
            .await?;
        match response {
            RemoteDaemonResponse::TaskStarted { task } => {
                let refreshed = self
                    .open_remote_repo(&context, self.max_file_size().await)
                    .await
                    .unwrap_or(opened);
                Ok(IndexTaskHandle {
                    task: task.into(),
                    repo_status: refreshed.repo_status,
                })
            }
            RemoteDaemonResponse::RepoBaseSnapshotRebuilt {
                indexed_docs,
                status,
            } => {
                let repo_status: WorkspaceSearchRepoStatus = status.into();
                Ok(IndexTaskHandle {
                    task: completed_remote_index_task(
                        &repo_status,
                        WorkspaceSearchTaskKind::Rebuild,
                        indexed_docs,
                    ),
                    repo_status,
                })
            }
            _ => Err("Unexpected flashgrep response while starting remote rebuild".to_string()),
        }
    }

    pub async fn search_content(
        &self,
        request: ContentSearchRequest,
    ) -> Result<ContentSearchResult, String> {
        let repo_root = normalize_remote_workspace_path(&request.repo_root.to_string_lossy());
        let context = self.ensure_remote_search_context(&repo_root).await?;
        let opened = self
            .open_remote_repo(&context, self.max_file_size().await)
            .await?;

        let mut command = format!(
            "{} daemon search --addr {} --repo-id {}",
            shell_escape(&context.binary_path),
            shell_escape(&opened.daemon_addr),
            shell_escape(&opened.repo_id),
        );
        if !request.use_regex {
            command.push_str(" --fixed-strings");
        }
        if !request.case_sensitive {
            command.push_str(" --ignore-case");
        }
        if request.multiline {
            command.push_str(" --multiline --multiline-dotall");
        }
        if request.whole_word {
            command.push_str(" --word-regexp");
        }
        if request.before_context > 0 {
            command.push_str(&format!(" --before-context {}", request.before_context));
        }
        if request.after_context > 0 {
            command.push_str(&format!(" --after-context {}", request.after_context));
        }
        if matches!(request.output_mode, ContentSearchOutputMode::Count) {
            command.push_str(" --count");
        }
        if matches!(
            request.output_mode,
            ContentSearchOutputMode::FilesWithMatches
        ) {
            command.push_str(" --quiet");
        }
        command.push_str(" --allow-scan-fallback");
        for glob in &request.globs {
            command.push_str(&format!(" --glob {}", shell_escape(glob)));
        }
        for file_type in &request.file_types {
            command.push_str(&format!(" --type {}", shell_escape(file_type)));
        }
        for file_type in &request.exclude_file_types {
            command.push_str(&format!(" --type-not {}", shell_escape(file_type)));
        }
        command.push(' ');
        command.push_str(&shell_escape(&request.pattern));
        if let Some(search_path) = request.search_path.as_ref() {
            command.push(' ');
            command.push_str(&shell_escape(&search_path.to_string_lossy()));
        }

        let response = self
            .execute_flashgrep_json(&context.connection.connection_id, &command)
            .await?;
        let (backend, repo_status, raw_results) = match response {
            RemoteDaemonResponse::SearchCompleted {
                backend,
                status,
                results,
            } => (backend, status, results),
            _ => {
                return Err(
                    "Unexpected flashgrep response while searching remote workspace".to_string(),
                );
            }
        };

        let mut results = convert_search_results(&raw_results, request.output_mode);
        let truncated = request
            .max_results
            .is_some_and(|limit| limit > 0 && results.len() >= limit);
        if let Some(limit) = request.max_results.filter(|limit| *limit > 0) {
            results.truncate(limit);
        }

        Ok(ContentSearchResult {
            outcome: FileSearchOutcome { results, truncated },
            file_counts: raw_results
                .file_counts
                .into_iter()
                .map(Into::into)
                .collect(),
            hits: raw_results.hits.into_iter().map(Into::into).collect(),
            backend: backend.into(),
            repo_status: repo_status.into(),
            candidate_docs: raw_results.candidate_docs,
            matched_lines: raw_results.matched_lines,
            matched_occurrences: raw_results.matched_occurrences,
        })
    }

    pub async fn resolve_remote_workspace_entry(
        &self,
        root_path: &str,
    ) -> Result<RemoteWorkspaceEntry, String> {
        if let Some(entry) =
            lookup_remote_connection_with_hint(root_path, self.preferred_connection_id.as_deref())
                .await
        {
            return Ok(entry);
        }
        lookup_remote_connection(root_path)
            .await
            .ok_or_else(|| format!("Remote workspace is not registered for path: {root_path}"))
    }

    async fn ensure_remote_search_context(
        &self,
        root_path: &str,
    ) -> Result<RemoteSearchContext, String> {
        let repo_root = normalize_remote_workspace_path(root_path);
        let connection = self.resolve_remote_workspace_entry(&repo_root).await?;
        let cached_server_info = self
            .ssh_manager
            .get_server_info(&connection.connection_id)
            .await;
        let remote_os = if let Some(server_info) = cached_server_info {
            if server_info.os_type.eq_ignore_ascii_case("unknown") {
                self.detect_remote_os_type(&connection.connection_id)
                    .await
                    .unwrap_or_else(|| server_info.os_type.clone())
            } else {
                server_info.os_type
            }
        } else {
            self.detect_remote_os_type(&connection.connection_id)
                .await
                .unwrap_or_else(|| "unknown".to_string())
        };
        let inferred_linux = remote_os.eq_ignore_ascii_case("unknown")
            && looks_like_linux_workspace_root(&repo_root);
        if !remote_os.eq_ignore_ascii_case("linux") && !inferred_linux {
            return Err(format!(
                "Remote workspace search currently supports Linux only, but server OS is {}",
                remote_os
            ));
        }

        let remote_arch = self
            .detect_remote_architecture(&connection.connection_id)
            .await?;
        let binary_path = self
            .ensure_remote_flashgrep_binary(&connection.connection_id, &repo_root, &remote_arch)
            .await?;
        let storage_root = join_remote_path(&repo_root, ".bitfun/search/flashgrep-index");
        let state_file = join_remote_path(&storage_root, REMOTE_FLASHGREP_STATE_FILE_NAME);

        Ok(RemoteSearchContext {
            connection,
            binary_path,
            repo_root,
            storage_root,
            state_file,
        })
    }

    async fn open_remote_repo(
        &self,
        context: &RemoteSearchContext,
        max_file_size: u64,
    ) -> Result<OpenedRemoteRepo, String> {
        let mut daemon_addr = self
            .ensure_remote_daemon_addr(
                &context.connection.connection_id,
                &context.binary_path,
                &context.state_file,
            )
            .await?;
        match self
            .open_remote_repo_once(context, &daemon_addr, max_file_size)
            .await
        {
            Ok(opened) => Ok(opened),
            Err(_) => {
                daemon_addr = self
                    .restart_remote_daemon(
                        &context.connection.connection_id,
                        &context.binary_path,
                        &context.state_file,
                    )
                    .await?;
                self.open_remote_repo_once(context, &daemon_addr, max_file_size)
                    .await
            }
        }
    }

    async fn open_remote_repo_once(
        &self,
        context: &RemoteSearchContext,
        daemon_addr: &str,
        max_file_size: u64,
    ) -> Result<OpenedRemoteRepo, String> {
        let response = self
            .execute_flashgrep_json(
                &context.connection.connection_id,
                &format!(
                    "{} daemon open --addr {} --repo {} --storage-root {} --max-file-size {}",
                    shell_escape(&context.binary_path),
                    shell_escape(daemon_addr),
                    shell_escape(&context.repo_root),
                    shell_escape(&context.storage_root),
                    max_file_size,
                ),
            )
            .await?;
        match response {
            RemoteDaemonResponse::RepoOpened { repo_id, status } => Ok(OpenedRemoteRepo {
                daemon_addr: daemon_addr.to_string(),
                repo_id,
                repo_status: status.into(),
            }),
            _ => Err("Unexpected flashgrep response while opening remote repo".to_string()),
        }
    }

    async fn execute_flashgrep_json(
        &self,
        connection_id: &str,
        command: &str,
    ) -> Result<RemoteDaemonResponse, String> {
        let (stdout, stderr, exit_code) = self
            .ssh_manager
            .execute_command(connection_id, command)
            .await
            .map_err(|error| format!("Failed to execute remote flashgrep command: {error}"))?;
        if exit_code != 0 {
            let detail = stderr.trim();
            return if detail.is_empty() {
                Err(format!(
                    "Remote flashgrep command failed with exit code {exit_code}"
                ))
            } else {
                Err(format!(
                    "Remote flashgrep command failed with exit code {exit_code}: {detail}"
                ))
            };
        }
        serde_json::from_str(stdout.trim()).map_err(|error| {
            format!(
                "Failed to parse remote flashgrep response as JSON: {error}. Raw output: {}",
                stdout.trim()
            )
        })
    }

    async fn detect_remote_architecture(&self, connection_id: &str) -> Result<String, String> {
        let mut attempts = Vec::new();

        for probe in REMOTE_ARCHITECTURE_PROBES {
            match self.ssh_manager.execute_command(connection_id, probe).await {
                Ok((stdout, stderr, exit_code)) => {
                    if let Some(arch) = parse_remote_architecture_output(&stdout, &stderr) {
                        return Ok(arch);
                    }
                    attempts.push(format!(
                        "probe=`{probe}` exit_code={exit_code} stdout={:?} stderr={:?}",
                        stdout.trim(),
                        stderr.trim()
                    ));
                }
                Err(error) => {
                    attempts.push(format!("probe=`{probe}` error={error}"));
                }
            }
        }

        Err(format!(
            "Failed to detect remote architecture from SSH output. Attempts: {}",
            attempts.join("; ")
        ))
    }

    async fn detect_remote_os_type(&self, connection_id: &str) -> Option<String> {
        for probe in REMOTE_OS_PROBES {
            let Ok((stdout, stderr, _exit_code)) =
                self.ssh_manager.execute_command(connection_id, probe).await
            else {
                continue;
            };
            if let Some(os_type) = parse_remote_os_output(&stdout, &stderr) {
                return Some(os_type);
            }
        }
        None
    }

    async fn ensure_remote_flashgrep_binary(
        &self,
        connection_id: &str,
        repo_root: &str,
        remote_arch: &str,
    ) -> Result<String, String> {
        let bundled_binary_names = match remote_arch {
            "x86_64" | "amd64" => LINUX_X86_64_FLASHGREP_BUNDLES,
            "aarch64" | "arm64" => LINUX_AARCH64_FLASHGREP_BUNDLES,
            arch => {
                return Err(format!(
                    "Remote workspace search does not support Linux architecture: {arch}"
                ));
            }
        };

        let (stdout, _, exit_code) = self
            .ssh_manager
            .execute_command(
                connection_id,
                "command -v flashgrep >/dev/null 2>&1 && command -v flashgrep",
            )
            .await
            .map_err(|error| format!("Failed to probe remote flashgrep binary: {error}"))?;
        if exit_code == 0 {
            let path = stdout.trim();
            if !path.is_empty() {
                return Ok(path.to_string());
            }
        }

        let (bundled_binary_name, local_binary_path) = bundled_binary_names
            .iter()
            .find_map(|binary_name| {
                resolve_local_flashgrep_bundle(binary_name).map(|path| (*binary_name, path))
            })
            .ok_or_else(|| {
                format!(
                    "Bundled Linux flashgrep binary is missing. Expected one of: {}",
                    bundled_binary_names
                        .iter()
                        .map(|name| format!("resources/flashgrep/{name}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;
        let install_dir = remote_flashgrep_install_dir(repo_root);
        let remote_binary_path = join_remote_path(&install_dir, bundled_binary_name);

        self.remote_file_service
            .create_dir_all(connection_id, &install_dir)
            .await
            .map_err(|error| {
                format!("Failed to create remote flashgrep install directory: {error}")
            })?;
        if !self
            .remote_file_service
            .exists(connection_id, &remote_binary_path)
            .await
            .map_err(|error| format!("Failed to inspect remote flashgrep binary path: {error}"))?
        {
            let bytes = tokio::fs::read(&local_binary_path).await.map_err(|error| {
                format!(
                    "Failed to read bundled flashgrep binary {}: {error}",
                    local_binary_path.display()
                )
            })?;
            self.remote_file_service
                .write_file(connection_id, &remote_binary_path, &bytes)
                .await
                .map_err(|error| format!("Failed to upload flashgrep to remote host: {error}"))?;
        }
        self.ssh_manager
            .execute_command(
                connection_id,
                &format!("chmod 755 {}", shell_escape(&remote_binary_path)),
            )
            .await
            .map_err(|error| format!("Failed to mark remote flashgrep as executable: {error}"))?;

        Ok(remote_binary_path)
    }

    async fn ensure_remote_daemon_addr(
        &self,
        connection_id: &str,
        binary_path: &str,
        state_file: &str,
    ) -> Result<String, String> {
        if let Some(addr) = self
            .read_remote_daemon_addr(connection_id, state_file)
            .await?
        {
            return Ok(addr);
        }
        self.restart_remote_daemon(connection_id, binary_path, state_file)
            .await
    }

    async fn restart_remote_daemon(
        &self,
        connection_id: &str,
        binary_path: &str,
        state_file: &str,
    ) -> Result<String, String> {
        let state_dir = Path::new(state_file)
            .parent()
            .and_then(|parent| parent.to_str())
            .ok_or_else(|| format!("Invalid remote flashgrep state file path: {state_file}"))?;
        let log_file = remote_daemon_log_file_path(state_file)
            .ok_or_else(|| format!("Invalid remote flashgrep log file path: {state_file}"))?;
        self.remote_file_service
            .create_dir_all(connection_id, state_dir)
            .await
            .map_err(|error| {
                format!("Failed to create remote flashgrep storage directory: {error}")
            })?;
        let start_command = format!(
            "rm -f {state_file} {log_file} && nohup {binary} serve --bind 127.0.0.1:0 --state-file {state_file} >{log_file} 2>&1 < /dev/null &",
            state_file = shell_escape(state_file),
            log_file = shell_escape(&log_file),
            binary = shell_escape(binary_path),
        );
        let (_, stderr, exit_code) = self
            .ssh_manager
            .execute_command(connection_id, &start_command)
            .await
            .map_err(|error| format!("Failed to start remote flashgrep daemon: {error}"))?;
        if exit_code != 0 {
            return Err(format!(
                "Failed to start remote flashgrep daemon: {}",
                stderr.trim()
            ));
        }

        let deadline = tokio::time::Instant::now() + REMOTE_FLASHGREP_STARTUP_TIMEOUT;
        loop {
            if let Some(addr) = self
                .read_remote_daemon_addr(connection_id, state_file)
                .await?
            {
                return Ok(addr);
            }
            if tokio::time::Instant::now() >= deadline {
                let diagnostics = self
                    .read_remote_daemon_log_tail(connection_id, &log_file)
                    .await;
                if let Some(classified_error) = diagnostics
                    .as_deref()
                    .and_then(|text| classify_remote_flashgrep_start_failure(binary_path, text))
                {
                    return Err(classified_error);
                }
                let diagnostic_suffix = diagnostics
                    .as_deref()
                    .filter(|text| !text.trim().is_empty())
                    .map(|text| format!(" Daemon log tail: {text}"))
                    .unwrap_or_default();
                return Err(format!(
                    "Timed out while waiting for remote flashgrep daemon to write its state file.{diagnostic_suffix}"
                ));
            }
            sleep(REMOTE_FLASHGREP_STARTUP_POLL_INTERVAL).await;
        }
    }

    async fn read_remote_daemon_addr(
        &self,
        connection_id: &str,
        state_file: &str,
    ) -> Result<Option<String>, String> {
        if !self
            .remote_file_service
            .exists(connection_id, state_file)
            .await
            .map_err(|error| format!("Failed to inspect remote flashgrep state file: {error}"))?
        {
            return Ok(None);
        }
        let contents = self
            .remote_file_service
            .read_file(connection_id, state_file)
            .await
            .map_err(|error| format!("Failed to read remote flashgrep state file: {error}"))?;
        let state: RemoteDaemonStateFile = serde_json::from_slice(&contents).map_err(|error| {
            format!(
                "Failed to parse remote flashgrep state file {}: {error}",
                state_file
            )
        })?;
        if state.addr.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(state.addr))
    }

    async fn read_remote_daemon_log_tail(
        &self,
        connection_id: &str,
        log_file: &str,
    ) -> Option<String> {
        if !self
            .remote_file_service
            .exists(connection_id, log_file)
            .await
            .ok()?
        {
            return None;
        }

        let bytes = self
            .remote_file_service
            .read_file(connection_id, log_file)
            .await
            .ok()?;
        let text = String::from_utf8_lossy(&bytes);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }

        let mut lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() > 12 {
            lines = lines.split_off(lines.len() - 12);
        }
        Some(lines.join(" | "))
    }

    async fn max_file_size(&self) -> u64 {
        match self
            .config_service
            .get_config::<WorkspaceConfig>(Some("workspace"))
            .await
        {
            Ok(workspace_config) => workspace_config.max_file_size,
            Err(error) => {
                log::warn!(
                    "Failed to read workspace config for remote flashgrep repo open, using default max_file_size: {}",
                    error
                );
                WorkspaceConfig::default().max_file_size
            }
        }
    }
}

fn remote_flashgrep_install_dir(repo_root: &str) -> String {
    join_remote_path(
        &normalize_remote_workspace_path(repo_root),
        REMOTE_FLASHGREP_INSTALL_DIR,
    )
}

fn remote_daemon_log_file_path(state_file: &str) -> Option<String> {
    let state_path = Path::new(state_file);
    let state_dir = state_path.parent()?.to_str()?;
    Some(join_remote_path(state_dir, REMOTE_FLASHGREP_LOG_FILE_NAME))
}

fn looks_like_linux_workspace_root(path: &str) -> bool {
    path.starts_with('/') && !path.contains(':')
}

fn parse_remote_architecture_output(stdout: &str, stderr: &str) -> Option<String> {
    for stream in [stdout, stderr] {
        for line in stream.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let normalized = trimmed.to_ascii_lowercase();
            if normalized.contains("x86_64") || normalized.contains("amd64") {
                return Some("x86_64".to_string());
            }
            if normalized.contains("aarch64")
                || normalized.contains("arm64")
                || normalized.contains("armv8")
            {
                return Some("aarch64".to_string());
            }
        }
    }

    None
}

fn parse_remote_os_output(stdout: &str, stderr: &str) -> Option<String> {
    for stream in [stdout, stderr] {
        for line in stream.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let normalized = trimmed.to_ascii_lowercase();
            if normalized.contains("linux") {
                return Some("Linux".to_string());
            }
            if normalized.contains("darwin") || normalized.contains("macos") {
                return Some("Darwin".to_string());
            }
            if normalized.contains("windows")
                || normalized.contains("mingw")
                || normalized.contains("msys")
                || normalized.contains("cygwin")
            {
                return Some("Windows".to_string());
            }
        }
    }

    None
}

fn classify_remote_flashgrep_start_failure(binary_path: &str, log_tail: &str) -> Option<String> {
    let normalized = log_tail.to_ascii_lowercase();
    if normalized.contains("glibc_") && normalized.contains("not found") {
        let binary_name = Path::new(binary_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(binary_path);
        let suggested_bundle = if binary_name.contains("aarch64") || binary_name.contains("arm64")
        {
            "flashgrep-aarch64-unknown-linux-musl"
        } else {
            "flashgrep-x86_64-unknown-linux-musl"
        };
        return Some(format!(
            "Bundled remote flashgrep binary {binary_name} is incompatible with the remote Linux libc. The server is missing the GLIBC version required by that build. Install `flashgrep` on the remote PATH or bundle a musl build such as resources/flashgrep/{suggested_bundle}. Daemon log tail: {log_tail}"
        ));
    }

    None
}

fn resolve_local_flashgrep_bundle(binary_name: &str) -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.join("../../../..");
    let mut candidates = vec![workspace_root.join("resources/flashgrep").join(binary_name)];

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            candidates.push(parent.join("resources/flashgrep").join(binary_name));
            candidates.push(parent.join("flashgrep").join(binary_name));
            candidates.push(parent.join("../Resources/flashgrep").join(binary_name));
            candidates.push(parent.join("../share/bitfun/flashgrep").join(binary_name));
            candidates.push(
                parent
                    .join("../share/com.bitfun.desktop/flashgrep")
                    .join(binary_name),
            );
        }
    }

    candidates.into_iter().find(|candidate| candidate.exists())
}

fn convert_search_results(
    search_results: &RemoteSearchResults,
    output_mode: ContentSearchOutputMode,
) -> Vec<FileSearchResult> {
    match output_mode {
        ContentSearchOutputMode::Content => convert_hits_to_file_search_results(search_results),
        ContentSearchOutputMode::Count => convert_file_counts_to_search_results(search_results),
        ContentSearchOutputMode::FilesWithMatches => {
            convert_hits_to_file_only_results(search_results)
        }
    }
}

fn convert_file_counts_to_search_results(
    search_results: &RemoteSearchResults,
) -> Vec<FileSearchResult> {
    search_results
        .file_counts
        .iter()
        .map(|count| FileSearchResult {
            path: count.path.clone(),
            name: Path::new(&count.path)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or(&count.path)
                .to_string(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: None,
            matched_content: Some(count.matched_lines.to_string()),
            preview_before: None,
            preview_inside: None,
            preview_after: None,
        })
        .collect()
}

fn convert_hits_to_file_search_results(
    search_results: &RemoteSearchResults,
) -> Vec<FileSearchResult> {
    let mut file_results = Vec::new();
    for hit in &search_results.hits {
        let name = Path::new(&hit.path)
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or(&hit.path)
            .to_string();

        let mut lines = BTreeMap::new();
        for file_match in &hit.matches {
            lines
                .entry(file_match.location.line)
                .or_insert_with(|| file_match.clone());
        }

        for (_, file_match) in lines {
            let (preview_before, preview_inside, preview_after) =
                split_preview(&file_match.snippet, &file_match.matched_text);
            file_results.push(FileSearchResult {
                path: hit.path.clone(),
                name: name.clone(),
                is_directory: false,
                match_type: SearchMatchType::Content,
                line_number: Some(file_match.location.line),
                matched_content: Some(file_match.snippet),
                preview_before,
                preview_inside,
                preview_after,
            });
        }
    }
    file_results
}

fn convert_hits_to_file_only_results(
    search_results: &RemoteSearchResults,
) -> Vec<FileSearchResult> {
    search_results
        .hits
        .iter()
        .map(|hit| FileSearchResult {
            path: hit.path.clone(),
            name: Path::new(&hit.path)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or(&hit.path)
                .to_string(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: None,
            matched_content: None,
            preview_before: None,
            preview_inside: None,
            preview_after: None,
        })
        .collect()
}

fn split_preview(
    snippet: &str,
    matched_text: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    if matched_text.is_empty() {
        return (None, Some(snippet.to_string()), None);
    }

    if let Some(offset) = snippet.find(matched_text) {
        let before = snippet[..offset].to_string();
        let inside = matched_text.to_string();
        let after = snippet[offset + matched_text.len()..].to_string();
        return (
            (!before.is_empty()).then_some(before),
            Some(inside),
            (!after.is_empty()).then_some(after),
        );
    }

    (None, Some(snippet.to_string()), None)
}

fn synthesize_active_task(
    repo_status: &WorkspaceSearchRepoStatus,
) -> Option<WorkspaceSearchTaskStatus> {
    let task_id = repo_status.active_task_id.clone()?;
    let (kind, phase, message) = match repo_status.phase {
        WorkspaceSearchRepoPhase::Preparing | WorkspaceSearchRepoPhase::NeedsIndex => (
            WorkspaceSearchTaskKind::Build,
            Some(WorkspaceSearchTaskPhase::Discovering),
            "Preparing remote workspace index".to_string(),
        ),
        WorkspaceSearchRepoPhase::Building => (
            WorkspaceSearchTaskKind::Build,
            Some(WorkspaceSearchTaskPhase::Processing),
            "Building remote workspace index".to_string(),
        ),
        WorkspaceSearchRepoPhase::Refreshing => (
            WorkspaceSearchTaskKind::Rebuild,
            Some(WorkspaceSearchTaskPhase::Refreshing),
            "Refreshing remote workspace index".to_string(),
        ),
        WorkspaceSearchRepoPhase::TrackingChanges => (
            WorkspaceSearchTaskKind::Refresh,
            Some(WorkspaceSearchTaskPhase::Refreshing),
            "Refreshing remote workspace changes".to_string(),
        ),
        WorkspaceSearchRepoPhase::Ready | WorkspaceSearchRepoPhase::Limited => (
            WorkspaceSearchTaskKind::Refresh,
            None,
            "Remote workspace index task is active".to_string(),
        ),
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Some(WorkspaceSearchTaskStatus {
        task_id,
        workspace_id: repo_status.repo_id.clone(),
        kind,
        state: WorkspaceSearchTaskState::Running,
        phase,
        message,
        processed: 0,
        total: None,
        started_unix_secs: now,
        updated_unix_secs: now,
        finished_unix_secs: None,
        cancellable: false,
        error: None,
    })
}

fn completed_remote_index_task(
    repo_status: &WorkspaceSearchRepoStatus,
    kind: WorkspaceSearchTaskKind,
    indexed_docs: usize,
) -> WorkspaceSearchTaskStatus {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let verb = match kind {
        WorkspaceSearchTaskKind::Build => "Built",
        WorkspaceSearchTaskKind::Rebuild => "Rebuilt",
        WorkspaceSearchTaskKind::Refresh => "Refreshed",
    };
    WorkspaceSearchTaskStatus {
        task_id: format!("remote-{}-{now}", repo_status.repo_id),
        workspace_id: repo_status.repo_id.clone(),
        kind,
        state: WorkspaceSearchTaskState::Completed,
        phase: Some(WorkspaceSearchTaskPhase::Finalizing),
        message: format!("{verb} remote workspace index with {indexed_docs} documents"),
        processed: indexed_docs,
        total: Some(indexed_docs),
        started_unix_secs: now,
        updated_unix_secs: now,
        finished_unix_secs: Some(now),
        cancellable: false,
        error: None,
    }
}

fn join_remote_path(base: &str, child: &str) -> String {
    let base = normalize_remote_workspace_path(base);
    let child = child.trim_start_matches('/');
    if base == "/" {
        format!("/{child}")
    } else {
        format!("{base}/{child}")
    }
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '-' | '_' | ':' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

impl From<RemoteSearchBackend> for WorkspaceSearchBackend {
    fn from(value: RemoteSearchBackend) -> Self {
        match value {
            RemoteSearchBackend::IndexedSnapshot | RemoteSearchBackend::IndexedClean => {
                Self::Indexed
            }
            RemoteSearchBackend::IndexedWorkspaceView => Self::IndexedWorkspace,
            RemoteSearchBackend::RgFallback => Self::TextFallback,
            RemoteSearchBackend::ScanFallback => Self::ScanFallback,
        }
    }
}

impl From<RemoteRepoPhase> for WorkspaceSearchRepoPhase {
    fn from(value: RemoteRepoPhase) -> Self {
        match value {
            RemoteRepoPhase::Opening => Self::Preparing,
            RemoteRepoPhase::MissingBaseSnapshot => Self::NeedsIndex,
            RemoteRepoPhase::BuildingBaseSnapshot => Self::Building,
            RemoteRepoPhase::ReadyClean => Self::Ready,
            RemoteRepoPhase::ReadyDirty => Self::TrackingChanges,
            RemoteRepoPhase::RebuildingBaseSnapshot => Self::Refreshing,
            RemoteRepoPhase::Degraded => Self::Limited,
        }
    }
}

impl From<RemoteDirtyFileStats> for WorkspaceSearchDirtyFiles {
    fn from(value: RemoteDirtyFileStats) -> Self {
        Self {
            modified: value.modified,
            deleted: value.deleted,
            new: value.new,
        }
    }
}

impl From<RemoteWorkspaceOverlayStatus> for WorkspaceSearchOverlayStatus {
    fn from(value: RemoteWorkspaceOverlayStatus) -> Self {
        Self {
            committed_seq_no: value.committed_seq_no,
            last_seq_no: value.last_seq_no,
            uncommitted_ops: value.uncommitted_ops,
            pending_docs: value.pending_docs,
            active_segments: value.active_segments,
            active_delete_segments: value.active_delete_segments,
            merge_requested: value.merge_requested,
            merge_running: value.merge_running,
            merge_attempts: value.merge_attempts,
            merge_completed: value.merge_completed,
            merge_failed: value.merge_failed,
            last_merge_error: value.last_merge_error,
        }
    }
}

impl From<RemoteRepoStatus> for WorkspaceSearchRepoStatus {
    fn from(value: RemoteRepoStatus) -> Self {
        Self {
            repo_id: value.repo_id,
            repo_path: value.repo_path,
            storage_root: value.storage_root,
            base_snapshot_root: value.base_snapshot_root,
            workspace_overlay_root: value.workspace_overlay_root,
            phase: value.phase.into(),
            snapshot_key: value.snapshot_key,
            last_probe_unix_secs: value.last_probe_unix_secs,
            last_rebuild_unix_secs: value.last_rebuild_unix_secs,
            dirty_files: value.dirty_files.into(),
            rebuild_recommended: value.rebuild_recommended,
            active_task_id: value.active_task_id,
            probe_healthy: value.probe_healthy,
            last_error: value.last_error,
            overlay: value.overlay.map(Into::into),
        }
    }
}

impl From<RemoteTaskKind> for WorkspaceSearchTaskKind {
    fn from(value: RemoteTaskKind) -> Self {
        match value {
            RemoteTaskKind::BuildBaseSnapshot => Self::Build,
            RemoteTaskKind::RebuildBaseSnapshot => Self::Rebuild,
            RemoteTaskKind::RefreshWorkspace => Self::Refresh,
        }
    }
}

impl From<RemoteTaskState> for WorkspaceSearchTaskState {
    fn from(value: RemoteTaskState) -> Self {
        match value {
            RemoteTaskState::Queued => Self::Queued,
            RemoteTaskState::Running => Self::Running,
            RemoteTaskState::Completed => Self::Completed,
            RemoteTaskState::Failed => Self::Failed,
            RemoteTaskState::Cancelled => Self::Cancelled,
        }
    }
}

impl From<RemoteTaskPhase> for WorkspaceSearchTaskPhase {
    fn from(value: RemoteTaskPhase) -> Self {
        match value {
            RemoteTaskPhase::Scanning => Self::Discovering,
            RemoteTaskPhase::Tokenizing => Self::Processing,
            RemoteTaskPhase::Writing => Self::Persisting,
            RemoteTaskPhase::Finalizing => Self::Finalizing,
            RemoteTaskPhase::RefreshingOverlay => Self::Refreshing,
        }
    }
}

impl From<RemoteTaskStatus> for WorkspaceSearchTaskStatus {
    fn from(value: RemoteTaskStatus) -> Self {
        Self {
            task_id: value.task_id,
            workspace_id: value.workspace_id,
            kind: value.kind.into(),
            state: value.state.into(),
            phase: value.phase.map(Into::into),
            message: value.message,
            processed: value.processed,
            total: value.total,
            started_unix_secs: value.started_unix_secs,
            updated_unix_secs: value.updated_unix_secs,
            finished_unix_secs: value.finished_unix_secs,
            cancellable: value.cancellable,
            error: value.error,
        }
    }
}

impl From<RemoteFileCount> for WorkspaceSearchFileCount {
    fn from(value: RemoteFileCount) -> Self {
        Self {
            path: value.path,
            matched_lines: value.matched_lines,
        }
    }
}

impl From<RemoteMatchLocation> for WorkspaceSearchMatchLocation {
    fn from(value: RemoteMatchLocation) -> Self {
        Self {
            line: value.line,
            column: value.column,
        }
    }
}

impl From<RemoteFileMatch> for WorkspaceSearchMatch {
    fn from(value: RemoteFileMatch) -> Self {
        Self {
            location: value.location.into(),
            snippet: value.snippet,
            matched_text: value.matched_text,
        }
    }
}

impl From<RemoteSearchLine> for WorkspaceSearchLine {
    fn from(value: RemoteSearchLine) -> Self {
        match value {
            RemoteSearchLine::Match { value } => Self::Match {
                value: value.into(),
            },
            RemoteSearchLine::Context {
                line_number,
                snippet,
            } => Self::Context {
                value: WorkspaceSearchContextLine {
                    line_number,
                    snippet,
                },
            },
            RemoteSearchLine::ContextBreak => Self::ContextBreak,
        }
    }
}

impl From<RemoteSearchHit> for WorkspaceSearchHit {
    fn from(value: RemoteSearchHit) -> Self {
        Self {
            path: value.path,
            matches: value.matches.into_iter().map(Into::into).collect(),
            lines: value.lines.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_remote_flashgrep_start_failure, looks_like_linux_workspace_root,
        parse_remote_architecture_output, parse_remote_os_output, remote_daemon_log_file_path,
        remote_flashgrep_install_dir, RemoteDaemonResponse,
    };

    #[test]
    fn parses_plain_uname_architecture_output() {
        assert_eq!(
            parse_remote_architecture_output("x86_64\n", ""),
            Some("x86_64".to_string())
        );
        assert_eq!(
            parse_remote_architecture_output("aarch64\n", ""),
            Some("aarch64".to_string())
        );
    }

    #[test]
    fn parses_architecture_from_banner_prefixed_output() {
        let stdout = "Welcome to Ubuntu 24.04 LTS\nLast login: today\nArchitecture: amd64\n";
        assert_eq!(
            parse_remote_architecture_output(stdout, ""),
            Some("x86_64".to_string())
        );
    }

    #[test]
    fn parses_architecture_from_stderr_when_needed() {
        assert_eq!(
            parse_remote_architecture_output("", "machine: arm64\n"),
            Some("aarch64".to_string())
        );
    }

    #[test]
    fn installs_remote_flashgrep_under_workspace_root() {
        assert_eq!(
            remote_flashgrep_install_dir("/home/wgq/workspace/bot_detection"),
            "/home/wgq/workspace/bot_detection/.bitfun/bin"
        );
    }

    #[test]
    fn resolves_remote_daemon_log_as_sibling_of_state_file() {
        assert_eq!(
            remote_daemon_log_file_path(
                "/home/wgq/workspace/bot_detection/.bitfun/search/flashgrep-index/daemon-state.json"
            ),
            Some(
                "/home/wgq/workspace/bot_detection/.bitfun/search/flashgrep-index/daemon.log"
                    .to_string()
            )
        );
    }

    #[test]
    fn parses_remote_os_from_uname_output() {
        assert_eq!(
            parse_remote_os_output("Linux\n", ""),
            Some("Linux".to_string())
        );
        assert_eq!(
            parse_remote_os_output("Darwin Kernel Version\n", ""),
            Some("Darwin".to_string())
        );
    }

    #[test]
    fn parses_remote_os_from_banner_prefixed_output() {
        assert_eq!(
            parse_remote_os_output("Welcome\nOperating system: linux\n", ""),
            Some("Linux".to_string())
        );
    }

    #[test]
    fn infers_linux_from_posix_workspace_root() {
        assert!(looks_like_linux_workspace_root(
            "/home/wgq/workspace/bot_detection"
        ));
        assert!(!looks_like_linux_workspace_root(
            "C:/Users/wgq/workspace/bot_detection"
        ));
    }

    #[test]
    fn classifies_glibc_incompatibility_as_bundle_issue() {
        let error = classify_remote_flashgrep_start_failure(
            "/home/wgq/workspace/bot_detection/.bitfun/bin/flashgrep-x86_64-unknown-linux-gnu",
            "/home/wgq/workspace/bot_detection/.bitfun/bin/flashgrep-x86_64-unknown-linux-gnu: /lib64/libc.so.6: version `GLIBC_2.33' not found",
        )
        .expect("expected glibc classification");

        assert!(error.contains("incompatible with the remote Linux libc"));
        assert!(error.contains("flashgrep-x86_64-unknown-linux-musl"));
    }

    #[test]
    fn deserializes_sync_build_response_from_flashgrep() {
        let response = serde_json::from_str::<RemoteDaemonResponse>(
            r#"{
                "kind": "repo_base_snapshot_built",
                "indexed_docs": 7,
                "status": {
                    "repo_id": "/home/wgq/workspace/original_performance_takehome",
                    "repo_path": "/home/wgq/workspace/original_performance_takehome",
                    "storage_root": "/home/wgq/workspace/original_performance_takehome/.bitfun/search/flashgrep-index",
                    "base_snapshot_root": "/home/wgq/workspace/original_performance_takehome/.bitfun/search/flashgrep-index/base-snapshot",
                    "workspace_overlay_root": "/home/wgq/workspace/original_performance_takehome/.bitfun/search/flashgrep-index/workspace-overlay",
                    "phase": "ready_clean",
                    "snapshot_key": "base-git-demo",
                    "last_probe_unix_secs": 1778294098,
                    "last_rebuild_unix_secs": 1778294098,
                    "dirty_files": { "modified": 0, "deleted": 0, "new": 0 },
                    "rebuild_recommended": false,
                    "active_task_id": null,
                    "probe_healthy": true,
                    "last_error": null
                }
            }"#,
        )
        .expect("expected sync build response to deserialize");

        match response {
            RemoteDaemonResponse::RepoBaseSnapshotBuilt {
                indexed_docs,
                status,
            } => {
                assert_eq!(indexed_docs, 7);
                assert_eq!(status.repo_id, "/home/wgq/workspace/original_performance_takehome");
            }
            other => panic!("unexpected response variant: {:?}", other),
        }
    }
}
