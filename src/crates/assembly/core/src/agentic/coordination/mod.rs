//! Coordination layer
//!
//! Top-level component that integrates all subsystems

mod background_outcomes;
mod coordination_store;
pub mod coordinator;
pub(crate) mod plan_todo_binding;
pub mod scheduler;
pub mod state_manager;
pub mod turn_outcome;
mod turn_settlement;

pub use coordinator::*;
pub use scheduler::*;
pub use state_manager::*;
pub use turn_outcome::*;

pub(crate) use plan_todo_binding::{
    PLAN_FILE_METADATA_KEY, TODO_ID_METADATA_KEY, read_todo_binding, should_auto_complete_todo,
};

pub(crate) use background_outcomes::{
    BackgroundSubagentOutcome, BackgroundSubagentOutcomeStore, BackgroundSubagentWaitMode,
    BackgroundSubagentWaitResult,
};

pub use coordinator::get_global_coordinator;
pub use scheduler::get_global_scheduler;
