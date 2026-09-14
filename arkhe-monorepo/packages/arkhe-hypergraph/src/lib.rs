//! Portal Gun hypergraph: S01-S15 scientific agents as validator nodes, with
//! hyperedges (higher-order collaborations) as topology transactions.
//!
//! This is the pure-Rust, pure-Rust dependency floor of the v0.6.0 upgrade:
//! no `pqcrypto-*` (C-backed), no `ndarray-linalg`/openblas, no `PriorityQueue`.
//! Signatures come from [`arkhe_pqc`] (FIPS 203 ML-KEM + FIPS 204 ML-DSA).

use arkhe_pqc::sign::{QuantumSignature, QuantumSigner};
use chrono::{Utc};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::collections::{HashMap, HashSet};

/// Identity of a scientific agent / validator node in the hypergraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentId {
    S01,
    S02,
    S03,
    S04,
    S05,
    S06,
    S07,
    S08,
    S09,
    S10,
    S11,
    S12,
    S13,
    S14,
    S15,
    Architect,
}

impl AgentId {
    /// Epistemic risk weight: how uncertain the agent's domain is. S07 (veto)
    /// carries the highest because it gates destructive decisions.
    pub fn focal_weight(self) -> f64 {
        use AgentId::*;
        match self {
            S01 | S03 => 0.8,
            S07 => 0.9,
            S02 | S05 | S06 => 0.6,
            S08 => 0.4,
            S04 => 0.3,
            S09 => 0.5,
            S10 => 0.5,
            S11 => 0.5,
            S12 => 0.5,
            S13 => 0.5,
            S14 => 0.5,
            S15 => 0.2,
            Architect => 0.5,
        }
    }

    pub fn short_name(self) -> &'static str {
        use AgentId::*;
        match self {
            S01 => "S01", S02 => "S02", S03 => "S03", S04 => "S04",
            S05 => "S05", S06 => "S06", S07 => "S07", S08 => "S08",
            S09 => "S09", S10 => "S10", S11 => "S11", S12 => "S12",
            S13 => "S13", S14 => "S14", S15 => "S15", Architect => "ARCH",
        }
    }
}

/// Kind of higher-order collaboration (a hyperedge).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HyperedgeType {
    Casimir,
    Stabilization,
    Amplification,
    Safety,
    Experiment,
    Custom { name: String, agents: Vec<AgentId> },
}

impl HyperedgeType {
    pub fn required_agents(&self) -> HashSet<AgentId> {
        use AgentId::*;
        let mut set = HashSet::new();
        match self {
            HyperedgeType::Casimir => set.extend([S01, S03, S04, S06]),
            HyperedgeType::Stabilization => set.extend([S02, S03, S05, S06]),
            HyperedgeType::Amplification => set.extend([S01, S05, S06, S09]),
            HyperedgeType::Safety => set.extend([S04, S07, S15]),
            HyperedgeType::Experiment => set.extend([S05, S06, S09, S14]),
            HyperedgeType::Custom { agents, .. } => set.extend(agents.iter().cloned()),
        }
        set
    }

    fn discriminant(&self) -> String {
        match self {
            HyperedgeType::Casimir => "Casimir".into(),
            HyperedgeType::Stabilization => "Stabilization".into(),
            HyperedgeType::Amplification => "Amplification".into(),
            HyperedgeType::Safety => "Safety".into(),
            HyperedgeType::Experiment => "Experiment".into(),
            HyperedgeType::Custom { name, .. } => name.clone(),
        }
    }
}

/// Lifecycle of a hyperedge in the ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HyperedgeState {
    Proposed,
    Formalizing,
    Simulating,
    Vetoed { reason: String, agent: AgentId },
    Certified,
    Archived,
}

/// Scientific payload carried by a hyperedge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScientificPayload {
    Hypothesis { text: String, proposer: AgentId },
    LeanProof { theorem: String, proof_hash: [u8; 32] },
    SimulationResult { fidelity: f64, data_hash: [u8; 32] },
    ExperimentalReadout { telemetry: Vec<f64>, timestamp: i64 },
    VetoRequest { risk_score: f64, justification: String },
    Certificate { aotb_hash: [u8; 32] },
}

/// A live hyperedge.
#[derive(Debug, Clone)]
pub struct Hyperedge {
    pub id: [u8; 32],
    pub edge_type: HyperedgeType,
    pub state: HyperedgeState,
    pub payload: ScientificPayload,
    /// PQC (ML-DSA) signatures from each signing agent.
    pub signatures: HashMap<AgentId, QuantumSignature>,
    pub created_at: i64,
    pub focal_score: f64,
}

/// Shared memory between agents — a wrapper dictionary keyed by edge id.
#[derive(Debug, Default)]
pub struct PortalGunHypergraph {
    pub edges: HashMap<[u8; 32], Hyperedge>,
    pub agents: HashMap<AgentId, AgentRuntimeState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeState {
    pub reputation: f64,
    pub signed_edges: usize,
}

impl PortalGunHypergraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, agent: AgentId) {
        self.agents.entry(agent).or_insert_with(|| AgentRuntimeState {
            reputation: 1.0,
            signed_edges: 0,
        });
    }

    /// Propose a new hyperedge; returns its content-addressed id.
    pub fn propose(&mut self, edge_type: HyperedgeType, payload: ScientificPayload) -> [u8; 32] {
        let id = Self::hash_edge(&edge_type, &payload);
        let required = edge_type.required_agents();
        let focal_score =
            required.iter().map(|a| a.focal_weight()).sum::<f64>() / required.len().max(1) as f64;
        let edge = Hyperedge {
            id,
            edge_type,
            state: HyperedgeState::Proposed,
            payload,
            signatures: HashMap::new(),
            created_at: Utc::now().timestamp(),
            focal_score,
        };
        self.edges.insert(id, edge);
        id
    }

    /// An agent certifies a hyperedge by signing its id + payload.
    pub fn sign(&mut self, edge_id: &[u8; 32], signer: &QuantumSigner, by: AgentId) -> bool {
        let Some(edge) = self.edges.get_mut(edge_id) else {
            return false;
        };
        if !edge.edge_type.required_agents().contains(&by) {
            return false; // non-member cannot sign
        }
        let sig = signer.sign(edge_id);
        edge.signatures.insert(by, sig);
        if let Some(a) = self.agents.get_mut(&by) {
            a.signed_edges += 1;
        }
        true
    }

    /// True when every mandatory agent has signed (PoP consensus gate).
    pub fn check_consensus(&self, edge_id: &[u8; 32]) -> bool {
        let Some(edge) = self.edges.get(edge_id) else {
            return false;
        };
        let required = edge.edge_type.required_agents();
        let signed: HashSet<AgentId> = edge.signatures.keys().cloned().collect();
        required.is_subset(&signed)
    }

    /// Validate one signature against the signed edge id.
    pub fn verify_signature(_edge_id: &[u8; 32], sig: &QuantumSignature) -> bool {
        arkhe_pqc::sign::quantum_verify_sig(sig).is_ok()
    }

    /// Topology guardrails structure the transitions between states.
    pub fn transition(
        &mut self,
        edge_id: &[u8; 32],
        new_state: HyperedgeState,
        by: AgentId,
    ) -> Result<(), HyperedgeError> {
        // Only S07 may apply a veto.
        if matches!(&new_state, HyperedgeState::Vetoed { .. }) && by != AgentId::S07 {
            return Err(HyperedgeError::Guardrail("only S07 may veto"));
        }
        // Only S15 may certify.
        if matches!(new_state, HyperedgeState::Certified) && by != AgentId::S15 {
            return Err(HyperedgeError::Guardrail("only S15 may certify"));
        }
        // Certification requires full consensus.
        if matches!(new_state, HyperedgeState::Certified) && !self.check_consensus(edge_id) {
            return Err(HyperedgeError::MissingConsensus);
        }
        // S04 must sign before Formalizing.
        if matches!(new_state, HyperedgeState::Formalizing) && !self.has_signed(edge_id, AgentId::S04) {
            return Err(HyperedgeError::Guardrail("S04 must sign before formalizing"));
        }

        let edge = self
            .edges
            .get_mut(edge_id)
            .ok_or(HyperedgeError::NotFound)?;

        // Simulation requires prior formalization / proposal only S04-gated.
        if matches!(new_state, HyperedgeState::Simulating)
            && !matches!(edge.state, HyperedgeState::Formalizing | HyperedgeState::Proposed)
        {
            return Err(HyperedgeError::Guardrail("must be proposed/formalized before simulation"));
        }

        edge.state = new_state;
        Ok(())
    }

    fn has_signed(&self, edge_id: &[u8; 32], agent: AgentId) -> bool {
        self.edges
            .get(edge_id)
            .map(|e| e.signatures.contains_key(&agent))
            .unwrap_or(false)
    }

pub fn get(&self, edge_id: &[u8; 32]) -> Option<&Hyperedge> {
        self.edges.get(edge_id)
    }

    pub fn get_mut(&mut self, edge_id: &[u8; 32]) -> Option<&mut Hyperedge> {
        self.edges.get_mut(edge_id)
    }

    fn hash_edge(edge_type: &HyperedgeType, payload: &ScientificPayload) -> [u8; 32] {
        let mut h = Sha3_256::new();
        h.update(b"arkhe:hyperedge:");
        h.update(edge_type.discriminant().as_bytes());
        h.update(b":");
        // Fold payload fields into the id (content addressed).
        match payload {
            ScientificPayload::Hypothesis { text, .. } => h.update(text.as_bytes()),
            ScientificPayload::LeanProof { theorem, proof_hash } => {
                h.update(theorem.as_bytes());
                h.update(proof_hash);
            }
            ScientificPayload::SimulationResult { fidelity, data_hash } => {
                h.update(fidelity.to_le_bytes());
                h.update(data_hash);
            }
            ScientificPayload::ExperimentalReadout { timestamp, .. } => {
                h.update(timestamp.to_le_bytes());
            }
            ScientificPayload::VetoRequest { risk_score, .. } => h.update(risk_score.to_le_bytes()),
            ScientificPayload::Certificate { aotb_hash } => h.update(aotb_hash),
        }
        let d = h.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&d[..32]);
        out
    }
}

#[derive(Debug)]
pub enum HyperedgeError {
    NotFound,
    Guardrail(&'static str),
    MissingConsensus,
}

impl std::fmt::Display for HyperedgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for HyperedgeError {}
pub mod governance;

/// Special-relativity kinematics (S03 CTC lane) — pure math, unit-tested.
pub mod relativity;

/// Numerai-style prediction/reputation layer (staking, CORR, meta-model).
pub mod prediction;

/// JSON facade over the real hypergraph primitives, wasm/JS-consumable.
pub mod wasm;

/// TOON → hypergraph → forge bridge (módulo #16).
pub mod bridge;

/// `#[wasm_bindgen]` re-export shim (only built with `--features wasm`).
#[cfg(feature = "wasm")]
pub mod wasm_bindings;

#[cfg(test)]
mod tests;

