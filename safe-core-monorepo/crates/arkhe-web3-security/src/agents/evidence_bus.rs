//! Async, in-memory evidence store. Every verdict the pipeline produces is
//! recorded here so a caller can inspect the full audit trail afterward,
//! not just the final report.

use crate::{InvariantId, InvariantVerdict};

#[derive(Debug, Clone)]
pub struct AuditEvidence {
    pub invariant_id: InvariantId,
    pub verdict: InvariantVerdict,
}

#[derive(Default)]
pub struct EvidenceBus {
    records: tokio::sync::Mutex<Vec<AuditEvidence>>,
}

impl EvidenceBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn store(&self, evidence: AuditEvidence) {
        self.records.lock().await.push(evidence);
    }

    pub async fn all(&self) -> Vec<AuditEvidence> {
        self.records.lock().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stores_and_returns_evidence_in_order() {
        let bus = EvidenceBus::new();
        bus.store(AuditEvidence { invariant_id: "FI-W03", verdict: InvariantVerdict::Holds }).await;
        bus.store(AuditEvidence {
            invariant_id: "FI-W04",
            verdict: InvariantVerdict::violated("reentrant call detected"),
        })
        .await;

        let all = bus.all().await;
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].invariant_id, "FI-W03");
        assert_eq!(all[1].invariant_id, "FI-W04");
    }
}
