//! WASM-bridgeable JSON facade over the real `PortalGunHypergraph` primitives.
//!
//! # Honesty notes vs. the v0.8.0 proposal
//! - The proposal's `with_default_stations`, `add_agent(spec)`, `spawn_edge`,
//!   `merge_edges`, `split_edge`, `sign_edge(.., sig)` and `unsafe
//!   MutexGuard::new_unchecked` do **not** exist on the real hypergraph. This
//!   bridge only calls the primitives that exist: [`PortalGunHypergraph::register`],
//!   [`PortalGunHypergraph::propose`], [`PortalGunHypergraph::sign`],
//!   [`PortalGunHypergraph::check_consensus`], [`PortalGunHypergraph::transition`]
//!   and [`PortalGunHypergraph::get`].
//! - No `unsafe`. The store lives in a `thread_local!` `RefCell`, which is the
//!   single-threaded-safe choice for wasm; no global `static Mutex`.
//! - PQC signing is real: each agent owns a [`QuantumSigner`] (ML-DSA).
//! - Every entry point returns JSON as `String` so a `#[wasm_bindgen]` shim can
//!   re-export them verbatim without extra logic (wasm-bindgen cannot be a
//!   native dependency; it lives behind the optional `wasm` feature).

use std::cell::RefCell;
use std::collections::HashMap;

use arkhe_pqc::sign::QuantumSigner;
use serde::Serialize;
use serde_json::json;

use crate::{
    AgentId, HyperedgeState, HyperedgeType, PortalGunHypergraph, ScientificPayload,
};

// ---------------------------------------------------------------------------
// Identity mapping
// ---------------------------------------------------------------------------

/// Map a specialty string used by the sandbox UI to a real [`AgentId`].
pub fn agent_id_from_specialty(s: &str) -> Option<AgentId> {
    Some(match s {
        "QuantumMaterials" => AgentId::S01,
        "PhotonicTimeCrystals" => AgentId::S02,
        "CTCGeometry" => AgentId::S03,
        "Lean4Formalization" => AgentId::S04,
        "FPGASynthesis" => AgentId::S05,
        "MetacrystalsOptics" => AgentId::S06,
        "EpistemologyVeto" => AgentId::S07,
        "LiteratureReview" => AgentId::S08,
        "HardwareFabrication" => AgentId::S09,
        "OpticalSystems" => AgentId::S10,
        "VacuumChambers" => AgentId::S11,
        "Sensors" => AgentId::S12,
        "PowerSystems" => AgentId::S13,
        "AirGapSecurity" => AgentId::S14,
        "CertificateRegistry" => AgentId::S15,
        "Architect" => AgentId::Architect,
        _ => return None,
    })
}

/// Default 16-station roster.
pub const DEFAULT_SPECIALTIES: [&str; 16] = [
    "QuantumMaterials",
    "PhotonicTimeCrystals",
    "CTCGeometry",
    "Lean4Formalization",
    "FPGASynthesis",
    "MetacrystalsOptics",
    "EpistemologyVeto",
    "LiteratureReview",
    "HardwareFabrication",
    "OpticalSystems",
    "VacuumChambers",
    "Sensors",
    "PowerSystems",
    "AirGapSecurity",
    "CertificateRegistry",
    "Architect",
];

fn edge_type_from_str(s: &str) -> Option<HyperedgeType> {
    Some(match s {
        "Casimir" => HyperedgeType::Casimir,
        "Stabilization" => HyperedgeType::Stabilization,
        "Amplification" => HyperedgeType::Amplification,
        "Safety" => HyperedgeType::Safety,
        "Experiment" => HyperedgeType::Experiment,
        _ => return None,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<[u8; 32]> {
    if s.len() < 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, c) in s.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(c).ok()?, 16).ok()?;
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

struct Sandbox {
    hg: PortalGunHypergraph,
    signers: HashMap<AgentId, QuantumSigner>,
}

impl Sandbox {
    fn new() -> Self {
        Self { hg: PortalGunHypergraph::new(), signers: HashMap::new() }
    }

    fn with_default_stations() -> Self {
        let mut s = Self::new();
        for spec in DEFAULT_SPECIALTIES {
            if let Some(a) = agent_id_from_specialty(spec) {
                s.ensure_agent(a);
            }
        }
        s
    }

    fn ensure_agent(&mut self, a: AgentId) {
        self.hg.register(a);
        self.signers.entry(a).or_default();
    }
}

thread_local! {
    static STORE: RefCell<HashMap<u32, Sandbox>> = RefCell::new(HashMap::new());
}

fn with_store<T>(f: impl FnOnce(&mut HashMap<u32, Sandbox>) -> T) -> T {
    STORE.with(|s| f(&mut s.borrow_mut()))
}

// Handles are process-local (wasm is single-threaded).
fn next_handle() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Serialized views
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct OpResult {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl OpResult {
    fn ok(v: String) -> String {
        serde_json::to_string(&Self { success: true, value: Some(v), error: None })
            .unwrap_or_default()
    }
    fn err(e: impl std::fmt::Display) -> String {
        serde_json::to_string(&Self { success: false, value: None, error: Some(e.to_string()) })
            .unwrap_or_default()
    }
}

#[derive(Serialize)]
struct HgStats {
    agent_count: usize,
    edge_count: usize,
    certified_count: usize,
    vetoed_count: usize,
    consensus_ready: usize,
}

// ---------------------------------------------------------------------------
// Public bridge API (JSON strings, wasm/JS-consumable)
// ---------------------------------------------------------------------------

/// Create a fresh hypergraph; returns a numeric handle.
pub fn hg_new() -> u32 {
    let h = next_handle();
    with_store(|s| {
        s.insert(h, Sandbox::new());
    });
    h
}

/// Create the default 16-station roster; returns a handle.
pub fn hg_new_default() -> u32 {
    let h = next_handle();
    with_store(|s| {
        s.insert(h, Sandbox::with_default_stations());
    });
    h
}

/// Free a handle.
pub fn hg_free(handle: u32) {
    with_store(|s| {
        s.remove(&handle);
    });
}

/// Register an agent (by specialty string).
pub fn hg_add_agent(handle: u32, specialty: &str) -> String {
    let Some(a) = agent_id_from_specialty(specialty) else {
        return OpResult::err(format!("Unknown specialty: {specialty}"));
    };
    with_store(|s| {
        if let Some(sb) = s.get_mut(&handle) {
            sb.ensure_agent(a);
            OpResult::ok(format!("{:?}", a))
        } else {
            OpResult::err("Invalid handle")
        }
    })
}

/// List agents as JSON array of `{id, name}`.
pub fn hg_list_agents(handle: u32) -> String {
    with_store(|s| {
        let Some(sb) = s.get(&handle) else { return "[]".into() };
        let mut out = Vec::new();
        for a in sb.hg.agents.keys() {
            out.push(json!({
                "id": format!("{:?}", a),
                "name": a.short_name(),
            }));
        }
        serde_json::to_string(&out).unwrap_or_default()
    })
}

/// Propose a hyperedge; `edge_type` is one of the real HyperedgeType names.
/// Required agents are derived from the edge type by the engine (PoP topology).
pub fn hg_spawn_edge(handle: u32, name: &str, edge_type: &str) -> String {
    let Some(et) = edge_type_from_str(edge_type) else {
        return OpResult::err(format!("Unknown edge_type: {edge_type}"));
    };
    with_store(|s| {
        let Some(sb) = s.get_mut(&handle) else { return OpResult::err("Invalid handle") };
        let payload = ScientificPayload::Hypothesis {
            text: name.to_string(),
            proposer: AgentId::Architect,
        };
        let id = sb.hg.propose(et, payload);
        OpResult::ok(hex(&id))
    })
}

/// List edges as JSON; each entry: `{id, type, state, signed, focal, consensus}`.
pub fn hg_list_edges(handle: u32) -> String {
    with_store(|s| {
        let Some(sb) = s.get(&handle) else { return "[]".into() };
        let edges = &sb.hg.edges;
        let mut out = Vec::new();
        for (id, e) in edges {
            let signed: Vec<&str> = e.signatures.keys().map(|a| a.short_name()).collect();
            out.push(json!({
                "id": hex(id),
                "type": format!("{:?}", e.edge_type),
                "state": state_label(e),
                "signed": signed,
                "focal": e.focal_score,
                "consensus": sb.hg.check_consensus(id),
            }));
        }
        serde_json::to_string(&out).unwrap_or_default()
    })
}

fn state_label(e: &crate::Hyperedge) -> String {
    match &e.state {
        HyperedgeState::Vetoed { reason, .. } => format!("Vetoed: {reason}"),
        other => format!("{other:?}"),
    }
}

/// A member signs the edge (ML-DSA).
pub fn hg_sign(handle: u32, edge_id: &str, by_specialty: &str) -> String {
    let Some(id) = unhex(edge_id) else { return OpResult::err("Bad edge id") };
    let Some(by) = agent_id_from_specialty(by_specialty) else {
        return OpResult::err(format!("Unknown specialty: {by_specialty}"));
    };
    with_store(|s| {
        let Some(sb) = s.get_mut(&handle) else { return OpResult::err("Invalid handle") };
        if !sb.hg.agents.contains_key(&by) {
            sb.ensure_agent(by);
        }
        let signer = sb.signers.get_mut(&by).unwrap();
        if sb.hg.sign(&id, signer, by) {
            OpResult::ok("signed".into())
        } else {
            OpResult::err("Signer is not a required agent of this edge")
        }
    })
}

/// True if every required agent has signed.
pub fn hg_check_consensus(handle: u32, edge_id: &str) -> String {
    let Some(id) = unhex(edge_id) else { return OpResult::err("Bad edge id") };
    with_store(|s| {
        let Some(sb) = s.get(&handle) else { return OpResult::err("Invalid handle") };
        OpResult::ok(sb.hg.check_consensus(&id).to_string())
    })
}

/// Apply a state transition with the acting agent (respects engine guardrails).
pub fn hg_transition(handle: u32, edge_id: &str, new_state: &str, by_specialty: &str) -> String {
    let Some(id) = unhex(edge_id) else { return OpResult::err("Bad edge id") };
    let Some(by) = agent_id_from_specialty(by_specialty) else {
        return OpResult::err(format!("Unknown specialty: {by_specialty}"));
    };
    let st = match new_state {
        "Formalizing" => HyperedgeState::Formalizing,
        "Simulating" => HyperedgeState::Simulating,
        "Certified" => HyperedgeState::Certified,
        "Archived" => HyperedgeState::Archived,
        _ => return OpResult::err(format!("Unknown state: {new_state}")),
    };
    with_store(|s| {
        let Some(sb) = s.get_mut(&handle) else { return OpResult::err("Invalid handle") };
        match sb.hg.transition(&id, st, by) {
            Ok(()) => OpResult::ok("transitioned".into()),
            Err(e) => OpResult::err(format!("{e:?}")),
        }
    })
}

/// Veto (the engine restricts this to `EpistemologyVeto`/S07).
pub fn hg_veto_edge(handle: u32, edge_id: &str, reason: &str, by_specialty: &str) -> String {
    let Some(id) = unhex(edge_id) else { return OpResult::err("Bad edge id") };
    let Some(by) = agent_id_from_specialty(by_specialty) else {
        return OpResult::err(format!("Unknown specialty: {by_specialty}"));
    };
    with_store(|s| {
        let Some(sb) = s.get_mut(&handle) else { return OpResult::err("Invalid handle") };
        match sb.hg.transition(
            &id,
            HyperedgeState::Vetoed { reason: reason.into(), agent: by },
            by,
        ) {
            Ok(()) => OpResult::ok("vetoed".into()),
            Err(e) => OpResult::err(format!("{e:?}")),
        }
    })
}

/// Stats snapshot for the UI.
pub fn hg_stats(handle: u32) -> String {
    with_store(|s| {
        let Some(sb) = s.get(&handle) else { return "{}".into() };
        let edges = &sb.hg.edges;
        let stats = HgStats {
            agent_count: sb.hg.agents.len(),
            edge_count: edges.len(),
            certified_count: edges
                .values()
                .filter(|e| matches!(e.state, HyperedgeState::Certified))
                .count(),
            vetoed_count: edges
                .values()
                .filter(|e| matches!(e.state, HyperedgeState::Vetoed { .. }))
                .count(),
            consensus_ready: edges.values().filter(|e| !e.signatures.is_empty()).count(),
        };
        serde_json::to_string(&stats).unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_cycle_survives_veto_guardrail() {
        let h = hg_new_default();
        let id = hg_spawn_edge(h, "H-Casimir", "Casimir");
        assert!(id.contains("\"success\":true"), "propose failed: {id}");
        let id_hex = serde_json::from_str::<serde_json::Value>(&id)
            .unwrap()["value"]
            .as_str()
            .unwrap()
            .to_string();

        // S04 must sign before Formalize (guardrail would block if we tried first).
        assert!(hg_sign(h, &id_hex, "Lean4Formalization").contains("signed"));
        assert!(hg_transition(h, &id_hex, "Formalizing", "Lean4Formalization").contains("transitioned"));

        // Consensus is not yet met (Safety needs S07 + S15 too).
        assert!(hg_check_consensus(h, &id_hex).contains("false"));

        // Only S07 can veto.
        assert!(hg_veto_edge(h, &id_hex, "risk", "Lean4Formalization").contains("S07"));
    }

    #[test]
    fn stats_and_agents_are_serializable() {
        let h = hg_new_default();
        let agents = hg_list_agents(h);
        let stats = hg_stats(h);
        let a: serde_json::Value = serde_json::from_str(&agents).unwrap();
        let s: serde_json::Value = serde_json::from_str(&stats).unwrap();
        assert!(a.is_array());
        assert_eq!(s["agent_count"], 16);
        hg_free(h);
    }
}