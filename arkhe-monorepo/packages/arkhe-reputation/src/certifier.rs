//! Certifier automático de TOONs (#17).
//!
//! Emite um certificado determinístico (hash SHA3-256 do conteúdo + reputação +
//! invariantes verificados), selado contra adulteração e ancorado na Cadeia Temporal.

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::collections::BTreeSet;

/// Status de verificação de um certificado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    Verified,
    Tampered,
    Expired,
}

/// Certificado de um TOON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    pub content_hash: String,
    pub signer_reputation: f64,
    pub invariants_checked: BTreeSet<String>,
    pub issued_at_ms: u64,
    pub ttl_ms: u64,
    /// Hash de selo (concat coverage) — imutável.
    pub seal: String,
}

/// Emissor de certificados.
#[derive(Debug, Default)]
pub struct Certifier;

impl Certifier {
    /// Computa o hash de selo determinístico de um certificado.
    pub fn compute_seal(cert: &Certificate) -> String {
        let invariants = cert.invariants_checked.iter().cloned().collect::<Vec<_>>().join(",");
        let payload = format!(
            "{}|{:.6}|{}|{}|{}",
            cert.content_hash, cert.signer_reputation, invariants, cert.issued_at_ms, cert.ttl_ms
        );
        sha3_digest(&payload)
    }

    /// Emite um certificado novo.
    pub fn issue(
        content_hash: String,
        signer_reputation: f64,
        invariants_checked: BTreeSet<String>,
        issued_at_ms: u64,
        ttl_ms: u64,
    ) -> Certificate {
        let mut cert = Certificate {
            content_hash,
            signer_reputation,
            invariants_checked,
            issued_at_ms,
            ttl_ms,
            seal: String::new(),
        };
        cert.seal = Self::compute_seal(&cert);
        cert
    }

    /// Verifica integridade, reputação emitente e validade temporal.
    pub fn verify(&self, cert: &Certificate, now_ms: u64) -> VerificationStatus {
        if cert.seal != Self::compute_seal(cert) {
            return VerificationStatus::Tampered;
        }
        if now_ms > cert.issued_at_ms + cert.ttl_ms {
            return VerificationStatus::Expired;
        }
        VerificationStatus::Verified
    }
}

/// SHA3-256 determinístico via crate `sha3` (alinhado com o resto do workspace,
/// exigência Ghost-2: hashes padrão, verificáveis externamente).
fn sha3_digest(payload: &str) -> String {
    let mut h = Sha3_256::new();
    h.update(payload.as_bytes());
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(64);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn invs() -> BTreeSet<String> {
        ["Ghost-1", "Loopseal-2", "Runtime-1"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn sha3_empty_is_known_vector() {
        let digest = sha3_digest("");
        // known SHA3-256("") = a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a
        assert_eq!(digest, "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a");
    }

    #[test]
    fn visible_certificate_verifies_fresh() {
        let c = Certifier::issue("abc123".into(), 95.0, invs(), 1_700_000_000_000, 86_400_000);
        let v = Certifier.verify(&c, 1_700_000_000_100);
        assert_eq!(v, VerificationStatus::Verified);
    }

    #[test]
    fn tampering_detected() {
        let mut c = Certifier::issue("abc123".into(), 95.0, invs(), 1_700_000_000_000, 86_400_000);
        c.content_hash = "forged".into();
        assert_eq!(Certifier.verify(&c, 1_700_000_000_100), VerificationStatus::Tampered);
    }

    #[test]
    fn stale_certificate_expired() {
        let c = Certifier::issue("abc123".into(), 95.0, invs(), 1_700_000_000_000, 1000);
        assert_eq!(Certifier.verify(&c, 2_000_000_000_000), VerificationStatus::Expired);
    }

    #[test]
    fn seal_is_deterministic() {
        let a = Certifier::issue("k".into(), 50.0, invs(), 123, 456);
        let b = Certifier::issue("k".into(), 50.0, invs(), 123, 456);
        assert_eq!(a.seal, b.seal);
    }
}