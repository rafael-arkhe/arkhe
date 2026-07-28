//! # ARKHE-SATURON
//!
//! An event-driven orchestrator for physics-hypothesis verification.
//!
//! The design rule is strict: **Rust orchestrates, it never does the math.**
//! Equations discovered from arXiv papers are carried as opaque text and handed
//! to an external symbolic solver over the [`ScriptRunner`] bridge; the solver's
//! verdict is folded back into an immutable-reducer state.
//!
//! ## Flow
//! ```text
//!   SaturonEvent ──▶ SaturonOrchestrator ──▶ ScriptRunner (external solver)
//!        ▲                   │                        │
//!        │                   ▼                        ▼
//!   (arXiv miner)      SaturonState.reduce      VERDICT:PASS / FAIL
//!                            │
//!                            ▼
//!                      SaturonCommand (to UI)
//! ```
//!
//! The orchestrator is a single-consumer loop, so scripts run one at a time —
//! that is where the "mutual exclusion" fence comes from, structurally.
//!
//! ## Example
//! ```
//! use std::sync::{Arc, Mutex};
//! use arkhe_saturon::*;
//!
//! # async fn demo() {
//! let state = Arc::new(Mutex::new(SaturonState::new()));
//! let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(8);
//! let orch = SaturonOrchestrator::new(state.clone(), Arc::new(MockRunner::passing()), cmd_tx);
//!
//! orch.handle(SaturonEvent::ArxivDiscovery {
//!     hypothesis_id: "HYP-1".into(),
//!     arxiv_toon_id: "TOON-A".into(),
//!     hypothesis_text: "Saturons saturate the Bekenstein bound.".into(),
//!     math_syntax: "S = k_B*c^3*A/(4*G*hbar)".into(),
//!     variables: vec![],
//! }).await;
//!
//! assert_eq!(state.lock().unwrap().status_of("HYP-1"), Some(HypothesisStatus::Verified));
//! assert!(matches!(cmd_rx.recv().await, Some(SaturonCommand::HypothesisVerified { .. })));
//! # }
//! ```

pub mod bridge;
pub mod event;
pub mod orchestrator;
pub mod state;
pub mod types;
pub mod verify;

pub use bridge::{BridgeError, MockRunner, ScriptRunner, SubprocessRunner};
pub use event::{SaturonCommand, SaturonEvent};
pub use orchestrator::SaturonOrchestrator;
pub use state::{Action, SaturonState};
pub use types::{
    Hypothesis, HypothesisStatus, PhysicalVariable, VerificationResult, VerificationType,
};
pub use verify::{build_check_script, parse_verdict};
