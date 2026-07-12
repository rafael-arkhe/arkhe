use serde::{Deserialize, Serialize};

/// Estado de um checkpoint após uma iteração do RSI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CheckpointStatus {
    Pending = 0,
    Validated = 1,
    RolledBack = 2,
}
