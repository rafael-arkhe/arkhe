#![deny(unsafe_code)]

//! ARKHE Fountain Protocol ↔ Buzz (Nostr) bridge.
//!
//! Translates ARKHE evidence between the Nostr relay (Z1) and the discrete
//!/continuous layers (Z2/Z3) under the epistemic firewall (Z0–Z3).

pub mod bridge;
pub mod evidence;
pub mod firewall;
pub mod fountain_core;
pub mod orch_or;

/// Fountain Protocol surface (OrchOR + LT codec).
pub mod fountain {
    pub use crate::fountain_core::{FountainDecoder, FountainEncoder, AFT_MAGIC};
    pub use crate::orch_or::{OrchORState, ORCH_OR_STATE_LEN};
}

pub use bridge::BuzzBridge;
pub use bridge::{KIND_AFT_FRAME, KIND_EVIDENCE_BUNDLE, KIND_FIREWALL_AUDIT, KIND_ORCH_OR_STATE};
pub use evidence::{CertificationStatus, DivergenceReport, EvidenceBundle};
pub use firewall::{EdgeType, Zone, validate_event_firewall, validate_hyperedge_firewall};
pub use fountain_core::{FountainDecoder, FountainEncoder, AFT_MAGIC};
pub use orch_or::{OrchORState, ORCH_OR_STATE_LEN};

pub const VERSION: &str = "0.1.0-ARKHE-BUZZ-2026-08-02";
