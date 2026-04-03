/// Event type definitions
///
/// Define cross-platform event data structures
use serde::{Deserialize, Serialize};

/// Event priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EventPriority {
    Low = 0,
    Normal = 1,
    High = 2,
}

impl Default for EventPriority {
    fn default() -> Self {
        Self::Normal
    }
}
