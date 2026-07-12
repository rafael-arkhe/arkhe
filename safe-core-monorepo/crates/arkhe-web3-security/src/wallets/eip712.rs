//! FI-W06 — Assinatura estruturada EIP-712 (domain separation).
//!
//! `valid(sig) → domain_hash(sig) = expected ∧ nonce(sig) = expected`
//!
//! Nota: usa BLAKE3 (via `arkhe_core::hash`) como stand-in determinístico para
//! o `keccak256` do EIP-712 real. O invariante de domain-separation e o
//! formato dos campos são fiéis à especificação; o hash primitivo não é.

use arkhe_core::hash::blake3_hash;
use arkhe_core::ArkheHash;

/// Domínio EIP-712: liga uma assinatura a um contrato/chain específico,
/// impedindo replay cross-contract e cross-chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Eip712Domain {
    pub name: String,
    pub version: String,
    pub chain_id: u64,
    pub verifying_contract: String,
}

impl Eip712Domain {
    /// Computa o hash de domínio determinístico.
    pub fn domain_hash(&self) -> ArkheHash {
        let payload = format!(
            "{}|{}|{}|{}",
            self.name, self.version, self.chain_id, self.verifying_contract
        );
        blake3_hash(payload.as_bytes())
    }
}

/// Uma assinatura estruturada com o hash de domínio no qual foi gerada.
#[derive(Debug, Clone)]
pub struct StructuredSignature {
    pub domain_hash: ArkheHash,
    pub nonce: u64,
}

/// FI-W06: a assinatura só é válida se o hash de domínio bate exatamente com
/// o domínio esperado (contrato/chain corretos) e o nonce é o esperado.
pub fn verify_domain_and_nonce(
    sig: &StructuredSignature,
    expected_domain: &Eip712Domain,
    expected_nonce: u64,
) -> bool {
    sig.domain_hash == expected_domain.domain_hash() && sig.nonce == expected_nonce
}

#[cfg(test)]
mod tests {
    use super::*;

    fn domain() -> Eip712Domain {
        Eip712Domain {
            name: "ArkheDApp".into(),
            version: "1".into(),
            chain_id: 1,
            verifying_contract: "0xArkhe".into(),
        }
    }

    #[test]
    fn matching_domain_and_nonce_is_valid() {
        let d = domain();
        let sig = StructuredSignature { domain_hash: d.domain_hash(), nonce: 5 };
        assert!(verify_domain_and_nonce(&sig, &d, 5));
    }

    #[test]
    fn cross_chain_replay_is_rejected() {
        let d = domain();
        let sig = StructuredSignature { domain_hash: d.domain_hash(), nonce: 5 };
        let other_chain = Eip712Domain { chain_id: 137, ..d };
        assert!(!verify_domain_and_nonce(&sig, &other_chain, 5));
    }

    #[test]
    fn wrong_nonce_is_rejected() {
        let d = domain();
        let sig = StructuredSignature { domain_hash: d.domain_hash(), nonce: 5 };
        assert!(!verify_domain_and_nonce(&sig, &d, 6));
    }
}
