//! State transition tracking for the SafeManifold.

use serde::{Deserialize, Serialize};
use crate::invariants::SystemState;

/// A single state transition record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionRecord {
    pub seq: u64,
    pub timestamp: String,
    pub from: SystemState,
    pub to: SystemState,
    pub reason: String,
    pub safe: bool,
}

/// Immutable history of state transitions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TransitionHistory {
    records: Vec<TransitionRecord>,
}

impl TransitionHistory {
    pub fn new() -> Self {
        Self { records: Vec::new() }
    }

    pub fn record(&mut self, from: SystemState, to: SystemState, reason: impl Into<String>) {
        let seq = self.records.len() as u64 + 1;
        let safe = to.check_all();
        self.records.push(TransitionRecord {
            seq,
            timestamp: String::new(),
            from,
            to,
            reason: reason.into(),
            safe,
        });
    }

    pub fn record_with_timestamp(
        &mut self,
        from: SystemState,
        to: SystemState,
        reason: impl Into<String>,
        timestamp: impl Into<String>,
    ) {
        let seq = self.records.len() as u64 + 1;
        let safe = to.check_all();
        self.records.push(TransitionRecord {
            seq,
            timestamp: timestamp.into(),
            from,
            to,
            reason: reason.into(),
            safe,
        });
    }

    pub fn records(&self) -> &[TransitionRecord] {
        &self.records
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn last(&self) -> Option<&TransitionRecord> {
        self.records.last()
    }

    pub fn unsafe_transitions(&self) -> Vec<&TransitionRecord> {
        self.records.iter().filter(|r| !r.safe).collect()
    }

    pub fn safe_count(&self) -> usize {
        self.records.iter().filter(|r| r.safe).count()
    }

    pub fn unsafe_count(&self) -> usize {
        self.records.iter().filter(|r| !r.safe).count()
    }
}
