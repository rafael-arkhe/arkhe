//! Hunting stage: runs the trace-based reentrancy analyzer over a recorded
//! call trace. Thin wrapper — the real logic lives in
//! `analyzers::reentrancy::detect_reentrant_cycle`, already unit tested.

use crate::analyzers::reentrancy::{detect_reentrant_cycle, CallStep};
use crate::InvariantVerdict;

pub struct HuntingAgent;

impl HuntingAgent {
    pub fn process(trace: &[CallStep]) -> InvariantVerdict {
        detect_reentrant_cycle(trace)
    }
}
