//! Event system module

pub mod event_system;
pub mod emitter;

pub use event_system::BackendEventSystem as BackendEventManager;
pub use emitter::EventEmitter;
pub use bitfun_transport::TransportEmitter;
pub use event_system::{BackendEvent, BackendEventSystem, get_global_event_system, emit_global_event};
