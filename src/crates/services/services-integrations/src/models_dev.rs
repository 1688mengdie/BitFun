//! models.dev source loading and last-valid snapshot persistence.
//!
//! This module deliberately exposes the source document as JSON text. The
//! provider-specific interpretation belongs to `bitfun-ai-adapters`, which is
//! above this integration layer in the repository dependency graph.

use log::{debug, warn};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::fs;
use tokio::sync::Mutex;

pub const DEFAULT_MODELS_DEV_ENDPOINT: &str = "https://models.dev/api.json";
const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const MIN_REFRESH_ATTEMPT_INTERVAL: Duration = Duration::from_secs(5 * 60);
const MAX_REFRESH_ATTEMPTS: usize = 3;
const BUNDLED_MODELS_DEV_SNAPSHOT: &str = include_str!("../assets/models-dev.json");

#[derive(Debug, Default)]
struct RefreshState {
    last_attempt: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelsDevSnapshotSource {
    Cache,
    Bundled,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelsDevSnapshot {
    pub body: String,
    pub source: ModelsDevSnapshotSource,
    pub version: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelsDevRefreshOutcome {
    NotNeeded,
    Throttled,
    Unchanged { version: u64 },
    Updated(ModelsDevSnapshot),
    Failed,
}

#[derive(Debug, Clone)]
pub struct ModelsDevCatalogService {
    cache_file: PathBuf,
    endpoint_url: String,
    bundled_snapshot: Arc<str>,
    cache_ttl: Duration,
    refresh_state: Arc<Mutex<RefreshState>>,
}

impl ModelsDevCatalogService {
    pub fn new(cache_file: impl Into<PathBuf>) -> Self {
        let endpoint_url = std::env::var("BITFUN_MODELS_DEV_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_MODELS_DEV_ENDPOINT.to_string());
        Self {
            cache_file: cache_file.into(),
            endpoint_url,
            bundled_snapshot: Arc::from(BUNDLED_MODELS_DEV_SNAPSHOT),
            cache_ttl: DEFAULT_CACHE_TTL,
            refresh_state: Arc::new(Mutex::new(RefreshState::default())),
        }
    }

    #[cfg(test)]
    fn with_bundled_snapshot(mut self, snapshot: impl Into<Arc<str>>) -> Self {
        self.bundled_snapshot = snapshot.into();
        self
    }

    #[cfg(test)]
    fn with_endpoint(mut self, endpoint_url: impl Into<String>) -> Self {
        self.endpoint_url = endpoint_url.into();
        self
    }

    #[cfg(test)]
    fn with_cache_ttl(mut self, cache_ttl: Duration) -> Self {
        self.cache_ttl = cache_ttl;
        self
    }

    /// Load the best immediately available source without contacting the network.
    pub async fn load_cached_or_bundled(&self) -> ModelsDevSnapshot {
        if let Ok(body) = fs::read_to_string(&self.cache_file).await {
            if is_valid_catalog_document(&body) {
                return snapshot(body, ModelsDevSnapshotSource::Cache);
            }
            debug!(
                "Ignoring invalid models.dev cache at {}",
                self.cache_file.display()
            );
        }

        if is_valid_catalog_document(&self.bundled_snapshot) {
            return snapshot(
                self.bundled_snapshot.to_string(),
                ModelsDevSnapshotSource::Bundled,
            );
        }

        snapshot("{}".to_string(), ModelsDevSnapshotSource::Empty)
    }

    /// Refresh the cache when stale. Failures leave the last valid cache intact.
    pub async fn refresh_if_stale(&self) -> ModelsDevRefreshOutcome {
        if self.endpoint_url.trim().is_empty() || self.is_cache_fresh().await {
            return ModelsDevRefreshOutcome::NotNeeded;
        }
        let Ok(mut refresh_state) = self.refresh_state.try_lock() else {
            return ModelsDevRefreshOutcome::Throttled;
        };
        let now = Instant::now();
        if refresh_state.last_attempt.is_some_and(|last_attempt| {
            now.duration_since(last_attempt) < MIN_REFRESH_ATTEMPT_INTERVAL
        }) {
            return ModelsDevRefreshOutcome::Throttled;
        }
        refresh_state.last_attempt = Some(now);
        drop(refresh_state);

        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
        {
            Ok(client) => client,
            Err(error) => {
                warn!("Failed to create models.dev HTTP client: {}", error);
                return ModelsDevRefreshOutcome::Failed;
            }
        };

        let body = {
            let mut body = None;
            for attempt in 0..MAX_REFRESH_ATTEMPTS {
                match fetch_catalog_body(&client, &self.endpoint_url).await {
                    Ok(value) => {
                        body = Some(value);
                        break;
                    }
                    Err(error) if error.is_retryable() && attempt + 1 < MAX_REFRESH_ATTEMPTS => {
                        warn!(
                            "models.dev catalog refresh attempt {}/{} failed: {}; retrying",
                            attempt + 1,
                            MAX_REFRESH_ATTEMPTS,
                            error
                        );
                        tokio::time::sleep(retry_backoff(attempt)).await;
                    }
                    Err(error) => {
                        warn!(
                            "models.dev catalog refresh failed after {} attempt(s): {}",
                            attempt + 1,
                            error
                        );
                        break;
                    }
                }
            }
            let Some(body) = body else {
                return ModelsDevRefreshOutcome::Failed;
            };
            body
        };

        let previous = self.load_cached_or_bundled().await;

        match self.write_cache_atomically(&body).await {
            Ok(()) if previous.sha256 == sha256_hex(body.as_bytes()) => {
                ModelsDevRefreshOutcome::Unchanged {
                    version: previous.version,
                }
            }
            Ok(()) => {
                ModelsDevRefreshOutcome::Updated(snapshot(body, ModelsDevSnapshotSource::Cache))
            }
            Err(error) => {
                warn!(
                    "Failed to persist models.dev catalog at {}: {}",
                    self.cache_file.display(),
                    error
                );
                ModelsDevRefreshOutcome::Failed
            }
        }
    }

    async fn is_cache_fresh(&self) -> bool {
        let Ok(metadata) = fs::metadata(&self.cache_file).await else {
            return false;
        };
        let Ok(modified) = metadata.modified() else {
            return false;
        };
        let age = SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default();
        age < self.cache_ttl
            && fs::read_to_string(&self.cache_file)
                .await
                .is_ok_and(|body| is_valid_catalog_document(&body))
    }

    async fn write_cache_atomically(&self, body: &str) -> std::io::Result<()> {
        if let Some(parent) = self.cache_file.parent() {
            fs::create_dir_all(parent).await?;
        }
        let temp_file = self.cache_file.with_file_name(format!(
            ".{}.{}.tmp",
            self.cache_file
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("models-dev"),
            std::process::id()
        ));
        fs::write(&temp_file, body).await?;
        #[cfg(not(windows))]
        {
            match fs::rename(&temp_file, &self.cache_file).await {
                Ok(()) => Ok(()),
                Err(error) => {
                    let _ = fs::remove_file(&temp_file).await;
                    Err(error)
                }
            }
        }

        #[cfg(windows)]
        {
            match fs::rename(&temp_file, &self.cache_file).await {
                Ok(()) => Ok(()),
                Err(_rename_error) if self.cache_file.exists() => {
                    let backup_file = self.cache_file.with_file_name(format!(
                        ".{}.{}.bak",
                        self.cache_file
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("models-dev"),
                        std::process::id()
                    ));
                    let _ = fs::remove_file(&backup_file).await;
                    if let Err(error) = fs::rename(&self.cache_file, &backup_file).await {
                        let _ = fs::remove_file(&temp_file).await;
                        return Err(error);
                    }
                    match fs::rename(&temp_file, &self.cache_file).await {
                        Ok(()) => {
                            let _ = fs::remove_file(&backup_file).await;
                            Ok(())
                        }
                        Err(error) => {
                            let _ = fs::rename(&backup_file, &self.cache_file).await;
                            let _ = fs::remove_file(&temp_file).await;
                            Err(error)
                        }
                    }
                }
                Err(rename_error) => {
                    let _ = fs::remove_file(&temp_file).await;
                    Err(rename_error)
                }
            }
        }
    }
}

#[derive(Debug)]
enum RefreshError {
    Request(reqwest::Error),
    Status(reqwest::StatusCode),
    InvalidDocument,
    ResponseBody(reqwest::Error),
}

impl RefreshError {
    fn is_retryable(&self) -> bool {
        match self {
            Self::Request(error) | Self::ResponseBody(error) => {
                error.is_connect() || error.is_timeout() || error.is_request() || error.is_body()
            }
            Self::Status(status) => {
                status.is_server_error() || matches!(status.as_u16(), 408 | 429)
            }
            Self::InvalidDocument => false,
        }
    }
}

impl std::fmt::Display for RefreshError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Request(error) => write!(formatter, "request error: {error}"),
            Self::Status(status) => write!(formatter, "HTTP {status}"),
            Self::InvalidDocument => write!(formatter, "response failed schema validation"),
            Self::ResponseBody(error) => write!(formatter, "response body error: {error}"),
        }
    }
}

async fn fetch_catalog_body(
    client: &reqwest::Client,
    endpoint_url: &str,
) -> Result<String, RefreshError> {
    let response = client
        .get(endpoint_url)
        .send()
        .await
        .map_err(RefreshError::Request)?;
    let status = response.status();
    if !status.is_success() {
        return Err(RefreshError::Status(status));
    }
    let body = response.text().await.map_err(RefreshError::ResponseBody)?;
    if !is_valid_catalog_document(&body) {
        return Err(RefreshError::InvalidDocument);
    }
    Ok(body)
}

fn retry_backoff(attempt: usize) -> Duration {
    match attempt {
        0 => Duration::from_millis(100),
        _ => Duration::from_millis(250),
    }
}

fn snapshot(body: String, source: ModelsDevSnapshotSource) -> ModelsDevSnapshot {
    let digest = sha256_hex(body.as_bytes());
    let version = digest
        .get(..16)
        .and_then(|prefix| u64::from_str_radix(prefix, 16).ok())
        .unwrap_or(0);
    ModelsDevSnapshot {
        body,
        source,
        version,
        sha256: digest,
    }
}

fn sha256_hex(body: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(body);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn is_valid_catalog_document(body: &str) -> bool {
    let Ok(serde_json::Value::Object(providers)) = serde_json::from_str(body) else {
        return false;
    };
    providers.iter().any(|(provider_id, provider)| {
        !provider_id.trim().is_empty()
            && provider
                .get("models")
                .and_then(serde_json::Value::as_object)
                .is_some_and(|models| {
                    models.iter().any(|(model_id, model)| {
                        !model_id.trim().is_empty()
                            && model
                                .get("id")
                                .and_then(serde_json::Value::as_str)
                                .is_some_and(|id| !id.trim().is_empty())
                    })
                })
    })
}

#[cfg(test)]
mod tests {
    use super::{ModelsDevCatalogService, ModelsDevRefreshOutcome, ModelsDevSnapshotSource};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const VALID: &str = r#"{"openai":{"id":"openai","name":"OpenAI","models":{"gpt-test":{"id":"gpt-test","name":"GPT Test"}}}}"#;

    #[tokio::test]
    async fn cache_is_preferred_over_bundled_snapshot() {
        let directory = tempfile::tempdir().expect("temp directory");
        let cache_file = directory.path().join("models.json");
        tokio::fs::write(&cache_file, VALID)
            .await
            .expect("cache write");
        let service = ModelsDevCatalogService::new(&cache_file)
            .with_bundled_snapshot(r#"{"anthropic":{"models":{"other":{"id":"other"}}}}"#);

        let snapshot = service.load_cached_or_bundled().await;

        assert_eq!(snapshot.source, ModelsDevSnapshotSource::Cache);
        assert!(snapshot.body.contains("gpt-test"));
    }

    #[tokio::test]
    async fn invalid_cache_falls_back_to_bundled_snapshot() {
        let directory = tempfile::tempdir().expect("temp directory");
        let cache_file = directory.path().join("models.json");
        tokio::fs::write(&cache_file, "not json")
            .await
            .expect("cache write");
        let service = ModelsDevCatalogService::new(&cache_file)
            .with_bundled_snapshot(VALID)
            .with_endpoint("")
            .with_cache_ttl(Duration::ZERO);

        let snapshot = service.load_cached_or_bundled().await;

        assert_eq!(snapshot.source, ModelsDevSnapshotSource::Bundled);
        assert!(snapshot.body.contains("gpt-test"));
        assert_eq!(
            service.refresh_if_stale().await,
            ModelsDevRefreshOutcome::NotNeeded
        );
    }

    #[tokio::test]
    async fn atomic_cache_write_leaves_a_valid_document() {
        let directory = tempfile::tempdir().expect("temp directory");
        let cache_file = directory.path().join("nested").join("models.json");
        let service = ModelsDevCatalogService::new(&cache_file).with_endpoint("");

        tokio::fs::create_dir_all(cache_file.parent().expect("cache parent"))
            .await
            .expect("cache parent creation");
        tokio::fs::write(
            &cache_file,
            r#"{"openai":{"models":{"gpt-old":{"id":"gpt-old"}}}}"#,
        )
        .await
        .expect("existing cache write");

        service
            .write_cache_atomically(VALID)
            .await
            .expect("atomic write");
        let snapshot = service.load_cached_or_bundled().await;

        assert_eq!(snapshot.source, ModelsDevSnapshotSource::Cache);
        assert_eq!(snapshot.body, VALID);
        assert!(!cache_file.with_extension("tmp").exists());
    }

    #[tokio::test]
    async fn transient_refresh_failures_are_retried_with_a_bounded_attempt_count() {
        let directory = tempfile::tempdir().expect("temp directory");
        let cache_file = directory.path().join("models.json");
        let (endpoint, attempts, server) =
            spawn_http_server(vec![(500, ""), (503, ""), (200, VALID)]).await;
        let service = ModelsDevCatalogService::new(&cache_file)
            .with_endpoint(endpoint)
            .with_cache_ttl(Duration::ZERO);

        let outcome = service.refresh_if_stale().await;

        assert!(matches!(outcome, ModelsDevRefreshOutcome::Updated(_)));
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        server.await.expect("test server should finish");
    }

    #[tokio::test]
    async fn unchanged_refresh_does_not_report_a_catalog_update() {
        let directory = tempfile::tempdir().expect("temp directory");
        let cache_file = directory.path().join("models.json");
        tokio::fs::write(&cache_file, VALID)
            .await
            .expect("existing cache write");
        let (endpoint, attempts, server) = spawn_http_server(vec![(200, VALID)]).await;
        let service = ModelsDevCatalogService::new(&cache_file)
            .with_endpoint(endpoint)
            .with_cache_ttl(Duration::ZERO);

        let outcome = service.refresh_if_stale().await;

        assert!(matches!(outcome, ModelsDevRefreshOutcome::Unchanged { .. }));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        server.await.expect("test server should finish");
    }

    async fn spawn_http_server(
        responses: Vec<(u16, &str)>,
    ) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server should bind");
        let address = listener.local_addr().expect("test server address");
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_task = attempts.clone();
        let responses = responses
            .into_iter()
            .map(|(status, body)| (status, body.to_string()))
            .collect::<Vec<_>>();
        let task = tokio::spawn(async move {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().await.expect("test request");
                attempts_for_task.fetch_add(1, Ordering::SeqCst);
                let mut request = [0_u8; 1024];
                let _ = stream.read(&mut request).await;
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("test response");
            }
        });
        (format!("http://{address}/models.json"), attempts, task)
    }
}
