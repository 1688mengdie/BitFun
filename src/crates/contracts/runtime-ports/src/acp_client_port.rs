//! ACP client runtime port.
//!
//! Core-defined boundary so the session tools (`SessionControl`, `SessionMessage`)
//! can reach the desktop-hosted ACP client without depending on the ACP crate.
//! The desktop host injects a concrete implementation (backed by `AcpClientService`)
//! through the coordinator; core tools only call the trait methods.
//!
//! This is a deliberately thin subset of the ACP surface: only the methods the
//! session tools need (create a persisted ACP flow session, forward a message,
//! stream a forwarded response). Every request/result is `Serialize + Deserialize`
//! so the boundary can cross process and workspace boundaries.

use super::{PortError, PortErrorKind, PortResult, RuntimeServicePort};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// `SessionControl` create request for a real external ACP flow session.
///
/// Starts an external ACP client process bound to a persisted session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientCreateRequest {
    /// Registered ACP client id (for example `codex` or `claude-code`).
    pub client_id: String,
    /// Workspace path the external ACP process runs in.
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
}

/// Result of [`AcpClientPort::create_session`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientCreateResult {
    pub session_id: String,
    pub session_name: String,
    pub agent_type: String,
}

/// `SessionMessage` ACP flow-session forward request (target is a flow session
/// id of the shape `acp_<client_id>_<uuid>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientMessageRequest {
    pub session_id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

/// Result of [`AcpClientPort::send_message_stream`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientMessageResult {
    pub session_id: String,
    /// Full response text produced by the external ACP agent.
    pub response: String,
}

/// One registered ACP client entry from [`AcpClientPort::list_clients`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientSummary {
    pub client_id: String,
    pub name: String,
    pub status: String,
    pub session_count: usize,
    pub readonly: bool,
}

/// Result of [`AcpClientPort::list_clients`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientListResult {
    pub clients: Vec<AcpClientSummary>,
}

/// `SessionMessage` ACP direct-path request: forward one message to the
/// external ACP agent bound to an internal BitFun session
/// (`acp__<client_id>` session).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientBitfunMessageRequest {
    /// Registered ACP client id (for example `codex` or `claude-code`).
    pub client_id: String,
    /// Internal BitFun session id the external ACP process is bound to.
    pub bitfun_session_id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

/// One incrementally streamed output chunk of an ACP direct message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AcpClientStreamChunk {
    /// One incremental text chunk of the external agent's response.
    Text { text: String },
    /// One incremental thought chunk from the external agent.
    Thought { text: String },
    /// The external agent completed its turn.
    Completed,
    /// The external turn was cancelled.
    Cancelled,
}

/// Sink receiving [`AcpClientStreamChunk`] items while a streamed ACP message
/// runs. Unbounded so the producer never drops a chunk when the consumer is
/// temporarily slower.
pub type AcpClientStreamChunkSink = mpsc::UnboundedSender<AcpClientStreamChunk>;

/// ACP client runtime port.
///
/// Implementations live on the product host (desktop) and forward every call to
/// the real `AcpClientService`; core tools never touch the ACP crate.
#[async_trait]
pub trait AcpClientPort: RuntimeServicePort {
    /// Create a persisted ACP flow session and start the external client
    /// process for it. Implementations must roll the record back when the
    /// process start fails so no orphan record is left behind.
    async fn create_session(
        &self,
        request: AcpClientCreateRequest,
    ) -> PortResult<AcpClientCreateResult>;

    /// List registered ACP clients with their current runtime facts. Used by the
    /// `SessionMessage` ACP direct path to verify the target client is actually
    /// registered before routing (COORD-03: the `acp__` prefix is only a clue,
    /// the ACP client registry is authoritative).
    async fn list_clients(&self) -> PortResult<AcpClientListResult>;

    /// Forward one message through the real channel to an external ACP agent
    /// addressed by a flow session id (`acp_<client_id>_<uuid>`) and stream the
    /// response incrementally. Text chunks are pushed into `chunk_sink` as they
    /// arrive; the returned result still carries the full response text.
    async fn send_message_stream(
        &self,
        request: AcpClientMessageRequest,
        chunk_sink: AcpClientStreamChunkSink,
    ) -> PortResult<AcpClientMessageResult>;

    /// Forward one message to the external ACP agent bound to an internal
    /// BitFun session (`acp__<client_id>` session) and stream the response.
    /// This is the `SessionMessage` direct path: no local model turn is
    /// involved, only the port call.
    async fn send_message_to_bitfun_session_stream(
        &self,
        request: AcpClientBitfunMessageRequest,
        chunk_sink: AcpClientStreamChunkSink,
    ) -> PortResult<AcpClientMessageResult>;
}

/// Error helper: wrap an implementation failure as a backend `PortError`.
pub fn acp_backend_error(message: impl Into<String>) -> PortError {
    PortError::new(PortErrorKind::Backend, message)
}

/// Dependency-free canonical uuid shape guard for flow-session ids.
///
/// ACP flow session ids have the shape `acp_<client_id>_<uuid>`; the trailing
/// segment must be a canonical uuid (length 36, dashed 8-4-4-4-12, hex) so an
/// internal session id that merely starts with `acp_` is never mistaken for a
/// flow session, and an empty client id (`acp__<uuid>`) is rejected.
///
/// Single authoritative implementation (d3-P2-2): the desktop `AcpClientPort`,
/// `SessionMessage` direct-path tool and the Task ACP flow branch all share
/// this guard so the flow-session classification can never drift between
/// layers.
pub fn looks_like_uuid(segment: &str) -> bool {
    segment.len() == 36
        && segment.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

/// Parse the ACP client id out of a flow session id of the shape
/// `acp_<client_id>_<uuid>`. Returns `None` for any other id shape (including
/// an empty client id). Single authoritative implementation (d3-P2-2).
pub fn acp_flow_client_id_from_session_id(session_id: &str) -> Option<String> {
    let rest = session_id.strip_prefix("acp_")?;
    let (client_id, uuid_segment) = rest.rsplit_once('_')?;
    if client_id.is_empty() || !looks_like_uuid(uuid_segment) {
        return None;
    }
    Some(client_id.to_string())
}

#[cfg(test)]
mod acp_flow_id_tests {
    use super::{acp_flow_client_id_from_session_id, looks_like_uuid};

    #[test]
    fn looks_like_uuid_accepts_only_canonical_shape() {
        assert!(looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"));
        assert!(!looks_like_uuid("7f0e1a2b3c4d4e5f8a9b0c1d2e3f4a5b"));
        assert!(!looks_like_uuid(
            "7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b-extra"
        ));
        assert!(!looks_like_uuid(""));
        assert!(!looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5"));
    }

    #[test]
    fn acp_flow_client_id_parses_from_flow_session_id() {
        assert_eq!(
            acp_flow_client_id_from_session_id("acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b")
                .as_deref(),
            Some("codex")
        );
        assert_eq!(
            acp_flow_client_id_from_session_id(
                "acp_claude-code_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"
            )
            .as_deref(),
            Some("claude-code")
        );
    }

    #[test]
    fn acp_flow_client_id_rejects_non_flow_shapes() {
        assert_eq!(acp_flow_client_id_from_session_id("session-123"), None);
        assert_eq!(acp_flow_client_id_from_session_id("acp_codebuddy"), None);
        assert_eq!(
            acp_flow_client_id_from_session_id("acp__7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"),
            None
        );
        assert_eq!(acp_flow_client_id_from_session_id(""), None);
    }
}
