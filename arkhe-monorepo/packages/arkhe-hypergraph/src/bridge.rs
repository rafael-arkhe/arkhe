//! Bridge hypergraph ↔ kernel/forge (#16).
//!
//! Sincroniza um TOON (registro de topologia validada) para dentro do hypergraph
//! como um hyperedge de experimento; valida o portão de consenso kernel; e, ao
//! certificar, emite uma ação de forge ancorada pela Cadeia Temporal (Loopseal-1)
//! e assinada em PQC (ML-DSA).

use crate::{
    AgentId, Hyperedge, HyperedgeState, HyperedgeType, PortalGunHypergraph, ScientificPayload,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Registro de TOON trazido pelo kernel (topologia validada linha-inteira).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToonRecord {
    /// Hash do conteúdo do TOON (SHA3-256).
    pub content_hash: [u8; 32],
    /// Fator Φ_C do registro (Gap-1).
    pub phi_c: f64,
    /// Charge topológica (ex.: Nsk) se o kernel a forneceu.
    pub topological_charge: Option<f64>,
    /// Assinaturas do kernel (verificação externa).
    pub kernel_sig_ok: bool,
}

/// Ação emitida para o forge após certificação.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeAction {
    pub edge_id: [u8; 32],
    pub action_type: String,
    pub payload_hash: [u8; 32],
    pub timestamp_ms: i64,
    pub committed: bool,
}

/// Bridge que conecta kernel → hypergraph → forge.
#[derive(Debug, Default)]
pub struct ToonBridge {
    pub hypergraph: PortalGunHypergraph,
    pub falling_log: Vec<ForgeAction>,
}

impl ToonBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sincroniza o TOON para um hyperedge de experimento com o payload topológico.
    pub fn sync_toon(&mut self, record: &ToonRecord) -> Result<[u8; 32], BridgeError> {
        if !record.kernel_sig_ok {
            return Err(BridgeError::KernelSignatureInvalid);
        }
        if !(0.577350 < record.phi_c && record.phi_c <= 0.999900) {
            return Err(BridgeError::PhiOutOfBounds(record.phi_c));
        }
        let payload = match record.topological_charge {
            Some(nsk) => ScientificPayload::SimulationResult { fidelity: nsk, data_hash: record.content_hash },
            None => ScientificPayload::ExperimentalReadout { telemetry: vec![record.phi_c], timestamp: Utc::now().timestamp() },
        };
        let id = self.hypergraph.propose(HyperedgeType::Experiment, payload);
        Ok(id)
    }

    /// Certifica o hyperedge via S15 após assinatura dos membros (consenso).
    pub fn certify(&mut self, edge_id: &[u8; 32], signer: &arkhe_pqc::sign::QuantumSigner) -> Result<(), BridgeError> {
        for agent in [AgentId::S05, AgentId::S06, AgentId::S09, AgentId::S14] {
            if !self.hypergraph.sign(edge_id, signer, agent) {
                return Err(BridgeError::SignRejected(agent));
            }
        }
        if !self.hypergraph.check_consensus(edge_id) {
            return Err(BridgeError::ConsensusUnmet);
        }
        self.hypergraph
            .transition(edge_id, HyperedgeState::Certified, AgentId::S15)
            .map_err(|e| BridgeError::Transition(e.to_string()))?;
        Ok(())
    }

    /// Emite a ação de forge ancorada (uma vez só por edge).
    pub fn forge_action(&mut self, edge_id: &[u8; 32]) -> Result<ForgeAction, BridgeError> {
        if self.falling_log.iter().any(|a| a.edge_id == *edge_id) {
            return Err(BridgeError::AlreadyForged);
        }
        let edge: &Hyperedge = self
            .hypergraph
            .get(edge_id)
            .ok_or(BridgeError::EdgeNotFound)?;
        if !matches!(edge.state, HyperedgeState::Certified) {
            return Err(BridgeError::NotCertified);
        }
        let action = ForgeAction {
            edge_id: *edge_id,
            action_type: "forge.structure.commit".into(),
            payload_hash: *edge_id,
            timestamp_ms: Utc::now().timestamp_millis(),
            committed: true,
        };
        self.falling_log.push(action.clone());
        Ok(action)
    }

    /// Camada chamada para carregar TOONs: convite a checagem de consistência.
    pub fn root(&self) -> &PortalGunHypergraph {
        &self.hypergraph
    }
}

#[derive(Debug)]
pub enum BridgeError {
    KernelSignatureInvalid,
    PhiOutOfBounds(f64),
    SignRejected(AgentId),
    ConsensusUnmet,
    Transition(String),
    EdgeNotFound,
    NotCertified,
    AlreadyForged,
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for BridgeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_pqc::sign::QuantumSigner;

    fn toon(phi: f64, nsk: f64) -> ToonRecord {
        ToonRecord {
            content_hash: [0x11; 32],
            phi_c: phi,
            topological_charge: Some(nsk),
            kernel_sig_ok: true,
        }
    }

    #[test]
    fn sync_certify_forge_e2e() {
        let signer = QuantumSigner::new();
        let mut bridge = ToonBridge::new();
        let id = bridge.sync_toon(&toon(0.98, 1.0)).unwrap();
        assert!(bridge.hypergraph.get(&id).is_some());

        bridge.certify(&id, &signer).unwrap();
        let action = bridge.forge_action(&id).unwrap();
        assert_eq!(action.action_type, "forge.structure.commit");
        assert!(action.committed);

        // Idempotência de forge.
        assert!(matches!(bridge.forge_action(&id), Err(BridgeError::AlreadyForged)));
    }

    #[test]
    fn phi_out_of_bounds_rejected() {
        let mut bridge = ToonBridge::new();
        assert!(matches!(
            bridge.sync_toon(&toon(0.1, 1.0)),
            Err(BridgeError::PhiOutOfBounds(_))
        ));
    }

    #[test]
    fn bad_kernel_signature_rejected() {
        let mut bridge = ToonBridge::new();
        let mut r = toon(0.98, 1.0);
        r.kernel_sig_ok = false;
        assert!(matches!(bridge.sync_toon(&r), Err(BridgeError::KernelSignatureInvalid)));
    }

    #[test]
    fn forge_requires_certification() {
        let mut bridge = ToonBridge::new();
        let id = bridge.sync_toon(&toon(0.98, 1.0)).unwrap();
        assert!(matches!(bridge.forge_action(&id), Err(BridgeError::NotCertified)));
    }
}