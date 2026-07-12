//! Safe Core / Evidence Bus integration.
//!
//! Wraps a transaction's lifecycle in [`Web3Adapter::before_transaction`] /
//! [`Web3Adapter::after_execution`] hooks. `after_execution` runs the full
//! [`crate::agents::AuditPipeline`] (reentrancy guard + CEI ordering +
//! nonce monotonicity) plus the EIP-712 domain/nonce check, and emits a
//! single [`Web3Evidence`] record hashed with BLAKE3 — reusing
//! `arkhe_core::hash::blake3_hash`, the same helper `wallets::eip712`
//! already uses, rather than introducing a second hash primitive.

use arkhe_core::hash::blake3_hash;
use arkhe_core::ArkheHash;

use crate::agents::{AuditError, AuditInput, AuditPipeline, AuditReport};
use crate::wallets::eip712::{verify_domain_and_nonce, Eip712Domain, StructuredSignature};

/// BLAKE3 digest, 32 bytes — see the module doc comment on why this reuses
/// `arkhe_core`'s existing hash type rather than introducing a new one.
pub type Blake3Hash = ArkheHash;

#[derive(Debug, Clone)]
pub struct Web3Evidence {
    pub transaction_id: String,
    pub audit_report: AuditReport,
    pub domain_ok: bool,
    pub content_hash: Blake3Hash,
}

impl Web3Evidence {
    /// True only if every invariant checked by the audit pipeline holds
    /// *and* the EIP-712 domain/nonce check (if one was requested) passed.
    pub fn is_clean(&self) -> bool {
        self.audit_report.all_hold() && self.domain_ok
    }
}

#[derive(Default)]
pub struct Web3Adapter {
    pipeline: AuditPipeline,
}

impl Web3Adapter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs before a transaction is admitted. No pre-execution invariant is
    /// implemented in this crate yet (everything here is checked
    /// post-execution against a recorded trace) — kept as an explicit,
    /// named seam so a future pre-check has somewhere to attach instead of
    /// requiring a restructure, rather than silently doing nothing with no
    /// trace of the gap.
    pub async fn before_transaction(&self, _transaction_id: &str) {}

    /// Runs after a transaction executes: runs the audit pipeline over
    /// `input`, optionally checks an EIP-712 domain/nonce, and returns one
    /// [`Web3Evidence`] record for the transaction.
    pub async fn after_execution(
        &self,
        transaction_id: &str,
        input: &AuditInput,
        domain_check: Option<(&StructuredSignature, &Eip712Domain, u64)>,
    ) -> Result<Web3Evidence, AuditError> {
        let audit_report = self.pipeline.run(input).await?;

        let domain_ok = match domain_check {
            Some((sig, domain, expected_nonce)) => verify_domain_and_nonce(sig, domain, expected_nonce),
            None => true,
        };

        let mut payload = transaction_id.as_bytes().to_vec();
        payload.extend_from_slice(format!("{:?}", audit_report.verdicts).as_bytes());
        payload.push(domain_ok as u8);
        let content_hash = blake3_hash(&payload);

        Ok(Web3Evidence { transaction_id: transaction_id.to_string(), audit_report, domain_ok, content_hash })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::reentrancy::CallStep;
    use crate::contracts::reentrancy::Op;
    use crate::agents::FunctionSignature;

    fn clean_input() -> AuditInput {
        AuditInput {
            functions: vec![FunctionSignature {
                name: "withdraw".into(),
                mutates_state: true,
                makes_external_call: true,
            }],
            call_trace: vec![CallStep { caller: "user".into(), callee: "vault".into() }],
            ops: vec![Op::Effect, Op::ExternalCall],
            nonce_events: vec![("alice".into(), 1)],
        }
    }

    #[tokio::test]
    async fn clean_transaction_produces_clean_evidence() {
        let adapter = Web3Adapter::new();
        adapter.before_transaction("tx-1").await;
        let evidence = adapter.after_execution("tx-1", &clean_input(), None).await.unwrap();
        assert!(evidence.is_clean());
        assert_eq!(evidence.transaction_id, "tx-1");
    }

    #[tokio::test]
    async fn same_input_produces_the_same_content_hash() {
        let adapter = Web3Adapter::new();
        let a = adapter.after_execution("tx-1", &clean_input(), None).await.unwrap();
        let b = adapter.after_execution("tx-1", &clean_input(), None).await.unwrap();
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[tokio::test]
    async fn different_transaction_ids_produce_different_content_hashes() {
        let adapter = Web3Adapter::new();
        let a = adapter.after_execution("tx-1", &clean_input(), None).await.unwrap();
        let b = adapter.after_execution("tx-2", &clean_input(), None).await.unwrap();
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[tokio::test]
    async fn failed_domain_check_marks_evidence_unclean() {
        let adapter = Web3Adapter::new();
        let domain = Eip712Domain {
            name: "ArkheDApp".into(),
            version: "1".into(),
            chain_id: 1,
            verifying_contract: "0xArkhe".into(),
        };
        let sig = StructuredSignature { domain_hash: domain.domain_hash(), nonce: 5 };
        let evidence = adapter
            .after_execution("tx-1", &clean_input(), Some((&sig, &domain, 6 /* wrong nonce */)))
            .await
            .unwrap();
        assert!(!evidence.domain_ok);
        assert!(!evidence.is_clean());
    }
}
