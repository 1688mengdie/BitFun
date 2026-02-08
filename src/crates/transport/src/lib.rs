/// BitFun Transport Layer
///
/// Cross-platform communication abstraction layer, supports:
/// - CLI (tokio mpsc)
/// - Tauri (app.emit)
/// - WebSocket/SSE (web server)

pub mod traits;
pub mod event_bus;
pub mod adapters;
pub mod events;
pub mod emitter;

pub use emitter::TransportEmitter;
pub use traits::{TransportAdapter, TextChunk, ToolEventPayload, ToolEventType, StreamEvent};
pub use event_bus::{EventBus, EventPriority};
pub use events::{
    UnifiedEvent, AgenticEventPayload, LspEventPayload, FileWatchEventPayload,
    ProfileEventPayload, SnapshotEventPayload, BackendEventPayload,
};
pub use adapters::{CliEvent, CliTransportAdapter, WebSocketTransportAdapter};

#[cfg(feature = "tauri-adapter")]
pub use adapters::TauriTransportAdapter;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
