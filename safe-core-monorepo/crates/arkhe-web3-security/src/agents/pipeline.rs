//! Orchestrates Recon → Hunting → Validation → GapFilling over a single
//! [`AuditInput`], recording every verdict to an [`EvidenceBus`].
//!
//! [`AuditPipeline::run`] is `async fn` end-to-end. The original (fabricated)
//! version of this pipeline mixed a synchronous `?` with `.await` calls
//! inside a non-`async` function — that doesn't compile in Rust. There is
//! nothing subtle to fix here beyond making the function signature honest
//! about being async; the type checker enforces the rest.

use crate::agents::evidence_bus::{AuditEvidence, EvidenceBus};
use crate::agents::gap_filling::GapFillingAgent;
use crate::agents::hunting::HuntingAgent;
use crate::agents::recon::{FunctionSignature, ReconAgent};
use crate::agents::validation::ValidationAgent;
use crate::analyzers::reentrancy::CallStep;
use crate::contracts::reentrancy::Op;
use crate::{InvariantId, InvariantVerdict};

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("audit input has no functions to analyze")]
    EmptyInput,
}

/// Everything a single audit run needs. In a real deployment this would be
/// assembled from an upstream Solidity/bytecode parser and an on-chain (or
/// simulated) transaction trace — this pipeline consumes the already
/// structured result, it doesn't produce it.
#[derive(Debug, Clone, Default)]
pub struct AuditInput {
    pub functions: Vec<FunctionSignature>,
    pub call_trace: Vec<CallStep>,
    pub ops: Vec<Op>,
    pub nonce_events: Vec<(String, u64)>,
}

#[derive(Debug, Clone)]
pub struct AuditReport {
    pub verdicts: Vec<(InvariantId, InvariantVerdict)>,
    pub gaps: Vec<InvariantId>,
}

impl AuditReport {
    pub fn all_hold(&self) -> bool {
        self.verdicts.iter().all(|(_, v)| v.holds())
    }
}

#[derive(Default)]
pub struct AuditPipeline {
    evidence_bus: EvidenceBus,
}

impl AuditPipeline {
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs the full Recon → Hunting → Validation → GapFilling pipeline
    /// over `input`, recording every verdict to the evidence bus.
    pub async fn run(&self, input: &AuditInput) -> Result<AuditReport, AuditError> {
        if input.functions.is_empty() {
            return Err(AuditError::EmptyInput);
        }

        let _functions = ReconAgent::process(&input.functions);

        let mut verdicts: Vec<(InvariantId, InvariantVerdict)> = Vec::new();

        let reentrancy_verdict = HuntingAgent::process(&input.call_trace);
        self.record("FI-W04", reentrancy_verdict.clone()).await;
        verdicts.push(("FI-W04", reentrancy_verdict));

        let cei_verdict = ValidationAgent::check_cei(&input.ops);
        self.record("FI-W03", cei_verdict.clone()).await;
        verdicts.push(("FI-W03", cei_verdict));

        for verdict in ValidationAgent::check_nonces(&input.nonce_events) {
            self.record("FI-W07", verdict.clone()).await;
            verdicts.push(("FI-W07", verdict));
        }

        let checked_ids: Vec<InvariantId> = verdicts.iter().map(|(id, _)| *id).collect();
        let gaps = GapFillingAgent::find_gaps(&checked_ids);

        Ok(AuditReport { verdicts, gaps })
    }

    async fn record(&self, invariant_id: InvariantId, verdict: InvariantVerdict) {
        self.evidence_bus.store(AuditEvidence { invariant_id, verdict }).await;
    }

    pub async fn evidence(&self) -> Vec<AuditEvidence> {
        self.evidence_bus.all().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input() -> AuditInput {
        AuditInput {
            functions: vec![FunctionSignature {
                name: "withdraw".into(),
                mutates_state: true,
                makes_external_call: true,
            }],
            call_trace: vec![
                CallStep { caller: "user".into(), callee: "vault".into() },
                CallStep { caller: "vault".into(), callee: "token".into() },
            ],
            ops: vec![Op::Effect, Op::ExternalCall],
            nonce_events: vec![("alice".into(), 1), ("alice".into(), 2)],
        }
    }

    #[tokio::test]
    async fn empty_input_is_rejected() {
        let pipeline = AuditPipeline::new();
        let result = pipeline.run(&AuditInput::default()).await;
        assert!(matches!(result, Err(AuditError::EmptyInput)));
    }

    #[tokio::test]
    async fn clean_input_produces_holding_verdicts_and_records_evidence() {
        let pipeline = AuditPipeline::new();
        let report = pipeline.run(&sample_input()).await.unwrap();

        assert!(report.all_hold());
        assert_eq!(pipeline.evidence().await.len(), report.verdicts.len());
        assert!(report.gaps.contains(&"FI-W01"));
        assert!(!report.gaps.contains(&"FI-W03"));
        assert!(!report.gaps.contains(&"FI-W04"));
        assert!(!report.gaps.contains(&"FI-W07"));
    }

    #[tokio::test]
    async fn reentrant_trace_is_flagged_in_the_report() {
        let mut input = sample_input();
        input.call_trace = vec![
            CallStep { caller: "user".into(), callee: "vault".into() },
            CallStep { caller: "vault".into(), callee: "attacker".into() },
            CallStep { caller: "attacker".into(), callee: "vault".into() },
        ];

        let pipeline = AuditPipeline::new();
        let report = pipeline.run(&input).await.unwrap();
        assert!(!report.all_hold());
    }
}
