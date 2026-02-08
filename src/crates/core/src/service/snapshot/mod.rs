pub mod events;
pub mod file_lock_manager;
pub mod isolation_manager;
pub mod manager;
pub mod service;
pub mod snapshot_core;
pub mod snapshot_system;
pub mod types;

pub use events::{
    emit_snapshot_event, emit_snapshot_session_event, initialize_snapshot_event_emitter,
    SnapshotEvent, SnapshotEventEmitter,
};
pub use manager::{
    ensure_global_snapshot_manager, get_global_snapshot_manager,
    initialize_global_snapshot_manager, SnapshotManager,
};
pub use service::{SnapshotService, SystemStats};
pub use snapshot_core::{FileChangeEntry, FileChangeQueue, SessionStats, SnapshotCore};
pub use types::*;
