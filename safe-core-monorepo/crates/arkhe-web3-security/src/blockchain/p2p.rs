//! FI-W16 / FI-W17 — Diversidade de clientes e mensagens P2P autenticadas.
//!
//! `client_count ≥ 3` · `∀ msg: authenticated(msg) → source_identity(msg) ∈ peers`

use crate::InvariantVerdict;
use std::collections::HashSet;

/// FI-W16: diversidade mínima de implementações de cliente na rede.
pub fn check_client_diversity(client_names: &[&str]) -> InvariantVerdict {
    let distinct: HashSet<&&str> = client_names.iter().collect();
    if distinct.len() >= 3 {
        InvariantVerdict::Holds
    } else {
        InvariantVerdict::violated(format!(
            "only {} distinct clients observed, need >= 3",
            distinct.len()
        ))
    }
}

/// Mensagem de rede P2P recebida de um peer.
#[derive(Debug, Clone)]
pub struct P2pMessage {
    pub source_peer_id: String,
    pub authenticated: bool,
}

/// FI-W17: mensagens autenticadas só podem ser aceitas de peers conhecidos.
/// Também é o invariante que teria bloqueado a exploração via
/// `CVE-2026-34219` (panic remoto no libp2p gossipsub) se a origem não
/// estivesse na lista de peers confiáveis.
pub fn check_message_authenticated(msg: &P2pMessage, known_peers: &[String]) -> InvariantVerdict {
    if !msg.authenticated {
        return InvariantVerdict::violated("message is not authenticated");
    }
    if !known_peers.iter().any(|p| p == &msg.source_peer_id) {
        return InvariantVerdict::violated(format!(
            "source peer '{}' not in known peer set",
            msg.source_peer_id
        ));
    }
    InvariantVerdict::Holds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_distinct_clients_hold() {
        assert!(check_client_diversity(&["geth", "nethermind", "erigon"]).holds());
    }

    #[test]
    fn single_client_violates() {
        assert!(!check_client_diversity(&["geth", "geth", "geth"]).holds());
    }

    #[test]
    fn authenticated_known_peer_holds() {
        let msg = P2pMessage { source_peer_id: "peer-1".into(), authenticated: true };
        let peers = vec!["peer-1".to_string()];
        assert!(check_message_authenticated(&msg, &peers).holds());
    }

    #[test]
    fn unauthenticated_message_violates() {
        let msg = P2pMessage { source_peer_id: "peer-1".into(), authenticated: false };
        let peers = vec!["peer-1".to_string()];
        assert!(!check_message_authenticated(&msg, &peers).holds());
    }

    #[test]
    fn unknown_peer_violates() {
        let msg = P2pMessage { source_peer_id: "peer-x".into(), authenticated: true };
        let peers = vec!["peer-1".to_string()];
        assert!(!check_message_authenticated(&msg, &peers).holds());
    }
}
