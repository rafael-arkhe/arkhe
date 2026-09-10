//! Gate mínimo real de orquestração — invariantes I534/I535 (bloco 1010, v390.1).
//!
//! Espelho Rust do núcleo [crate] `OrchestratorGateNucleus.lean` (core Lean
//! 4.33.1, sem Mathlib, sem `sorry`), sobre as **âncoras reais** do workspace:
//!
//! * `KernelVerdict::is_sound()` de `arkhe-lean-bridge` é o único caminho para
//!   o veredito `verified` (digest SHA3-256 da fonte + kernel exit 0 +
//!   `#check` elaborado);
//! * banda Gap-1 conservadora `(5774, 9999]` ×10⁴ — os mesmos vértices provados
//!   em `I534A..E`;
//! * `decay_scaled` de `arkhe-field-stability` (I530) para a composição da
//!   retenção (I534-E);
//! * `CoherenceLedger` (SHA3-256, Gravity-1) como trilha auditável: a entrada
//!   só é gravada quando o gate autoriza (I534 no nível do ledger).

use arkhe_field_stability::coherence::phi_default;
use arkhe_field_stability::decay::decay_scaled;
use arkhe_field_stability::ledger::{CoherenceEntry, CoherenceLedger};
use arkhe_field_stability::phi::gap1_satisfied;

/// Escala fixa ×10⁴ da quantização inteira (1.0) — idem núcleo do gate I534.
pub const SCALE_X1E4: u64 = 10_000;

/// Piso estrito Gap-1 ×10⁴ (0.5774 — arredondado para cima de 1/√3). Exclusivo.
pub const FLOOR_GAP1_X1E4: u64 = 5_774;

/// Teto inclusivo Gap-1 ×10⁴ (0.9999). Inclusivo.
pub const CEILING_GAP1_X1E4: u64 = 9_999;

/// Veredito do kernel Lean (espelho de `KernelVerdict::is_sound()` da ponte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateVerdict {
    /// Kernel digeriu a fonte íntegra com exit 0 e todas as `#check` elaboradas.
    Verified,
    /// Qualquer falha (digest alterado, exit ≠ 0, elaboração incompleta).
    Rejected,
}

impl GateVerdict {
    /// Traduz o veredito consolidado real da ponte [`KernelVerdict::is_sound`].
    #[must_use]
    pub fn from_kernel_sound(sound: bool) -> Self {
        if sound {
            Self::Verified
        } else {
            Self::Rejected
        }
    }
}

/// Causa de recusa tipada do gate (espelho de I534-A/B/C).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// Sem veredito `verified` — recompensa/avanço nunca (I534-A).
    NoVerifiedProof,
    /// Φ ≤ piso conservador — fora da banda por baixo (I534-B).
    BelowFloor,
    /// Φ > teto — fora da banda por cima (I534-C).
    AboveCeiling,
}

/// Decisão do gate para um par (veredito, Φ×10⁴).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateDecision {
    /// Passo autorizado: `verified` e Φ dentro de `(FLOOR, CEILING]`.
    Allow,
    /// Passo recusado, com a causa honesta.
    Reject(RejectReason),
}

impl GateDecision {
    /// `true` quando o passo é autorizado.
    #[must_use]
    pub fn allowed(self) -> bool {
        matches!(self, Self::Allow)
    }

    /// Causa da recusa (ou `None` quando autorizado).
    #[must_use]
    pub fn reject_reason(self) -> Option<RejectReason> {
        match self {
            Self::Allow => None,
            Self::Reject(r) => Some(r),
        }
    }
}

/// Escala Φ f64 (canônico quadrático) para ×10⁴ inteiro, por arredondamento.
///
/// Retorna `None` quando `phi` não é finito ou sai da banda unitária [0,1]
/// (pré-condição das provas I534/I530; o Φ canônico nunca sai).
#[must_use]
pub fn phi_to_x1e4(phi: f64) -> Option<u64> {
    if !phi.is_finite() || !(0.0..=1.0).contains(&phi) {
        return None;
    }
    Some((phi * SCALE_X1E4 as f64).round() as u64)
}

/// I534: autorização de passo = `verified` E `FLOOR < Φ ≤ CEILING`.
#[must_use]
pub fn allowed(verdict: GateVerdict, phi_x1e4: u64) -> bool {
    verdict == GateVerdict::Verified
        && FLOOR_GAP1_X1E4 < phi_x1e4
        && phi_x1e4 <= CEILING_GAP1_X1E4
}

/// Decisão tipada do gate (I534-A/B/C): nunca autoriza sem `verified`, abaixo
/// do piso ou acima do teto.
#[must_use]
pub fn evaluate(verdict: GateVerdict, phi_x1e4: u64) -> GateDecision {
    if verdict != GateVerdict::Verified {
        return GateDecision::Reject(RejectReason::NoVerifiedProof);
    }
    if phi_x1e4 <= FLOOR_GAP1_X1E4 {
        return GateDecision::Reject(RejectReason::BelowFloor);
    }
    if phi_x1e4 > CEILING_GAP1_X1E4 {
        return GateDecision::Reject(RejectReason::AboveCeiling);
    }
    GateDecision::Allow
}

/// I534-E: valor aceite, após retenção `k ≤ teto`, permanece ≤ teto.
///
/// Composição real com o decaimento I530 (`decay_scaled` de
/// `arkhe-field-stability`): `(Φ×k)/10⁴`. Sob `allowed(Verified, Φ)` e `k ≤
/// CEILING_GAP1_X1E4` o resultado fica ≤ teto (fecho da banda por cima).
#[must_use]
#[inline]
pub fn retained_after(phi_x1e4: u64, k_x1e4: u64) -> u64 {
    decay_scaled(phi_x1e4, k_x1e4)
}

/// Um passo do transcripto de orquestração (espelho I535): veredito × flag de
/// commit (recompensa/avanço efetivamente emitido).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptStep {
    /// Veredito do kernel no passo.
    pub verdict: GateVerdict,
    /// `true` quando o passo emite commit/recompensa.
    pub commit: bool,
}

/// N.º de commits no transcripto (passos que emitem recompensa/avanço).
#[must_use]
pub fn commitments(steps: &[TranscriptStep]) -> u64 {
    steps.iter().filter(|s| s.commit).count() as u64
}

/// N.º de provas verificadas (veredito `verified`, com ou sem commit).
#[must_use]
pub fn proofs(steps: &[TranscriptStep]) -> u64 {
    steps.iter().filter(|s| s.verdict == GateVerdict::Verified).count() as u64
}

/// Restrição do gate (espelho I535 `well_formed`): todo commit exige `verified`.
#[must_use]
pub fn is_well_formed(steps: &[TranscriptStep]) -> bool {
    steps
        .iter()
        .all(|s| !s.commit || s.verdict == GateVerdict::Verified)
}

/// I535-A: num transcripto bem-formado, commits nunca excedem provas.
///
/// A implicação é a propriedade inteira (`não-bem-formado` é vacuamente
/// verdade); a prova formal é `I535A_commits_le_proofs` no núcleo Lean.
#[must_use]
#[inline]
pub fn i535a_commits_le_proofs(steps: &[TranscriptStep]) -> bool {
    !is_well_formed(steps) || commitments(steps) <= proofs(steps)
}

/// Erro de registro no audit do gate — recusa (I534) ou violação do ledger real.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordError {
    /// Passo não autorizado pelo gate (I534-A/B/C), com a causa.
    Gate(RejectReason),
    /// O ledger real rejeitou (Gravity-1 monotonia, Loopseal-2 encadeamento).
    Ledger(String),
}

/// Gate sobre o ledger real: a trilha auditável (CoherenceLedger SHA3-256/
/// Gravity-1) só grava a entrada quando o gate autoriza (I534 no nível do
/// ledger — a recompensa não pode «esquecer» a prova que a funda).
#[derive(Debug, Clone, Default)]
pub struct GateAudit {
    ledger: CoherenceLedger,
}

impl GateAudit {
    /// Ledger vazio ancorado em `GENESIS`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ledger: CoherenceLedger::new(),
        }
    }

    /// Acesso de leitura ao ledger subjacente.
    #[must_use]
    pub fn ledger(&self) -> &CoherenceLedger {
        &self.ledger
    }

    /// Consome o audit e devolve o ledger (para inspeção/verificação final).
    #[must_use]
    pub fn into_ledger(self) -> CoherenceLedger {
        self.ledger
    }

    /// Regista um passo: computa o Φ canônico quadrático
    /// `phi_default(Ω,Σ,Λ)` (I511–I516), aplica o gate I534 e **só quando
    /// autorizado** grava a entrada no ledger (Loopseal-2 append-only).
    ///
    /// # Errors
    ///
    /// * [`RecordError::Gate`] — passo recusado (sem `verified`, fora da
    ///   banda); **nada é gravado** (a cadeia permanece intacta).
    /// * [`RecordError::Ledger`] — o ledger real recusou (Gravity-1/Loopseal-2).
    pub fn try_record(
        &mut self,
        window_id: u64,
        ts: u64,
        verdict: GateVerdict,
        stability: f64,
        success_rate: f64,
        latency_score: f64,
    ) -> Result<(), RecordError> {
        let phi = phi_default([stability, success_rate, latency_score]);
        debug_assert!(!gap1_satisfied(phi) || phi_to_x1e4(phi).is_some());
        let Some(phi_x1e4) = phi_to_x1e4(phi) else {
            debug_assert!(false, "Φ canônico fora de [0,1] — incoerência");
            return Ok(());
        };
        match evaluate(verdict, phi_x1e4) {
            GateDecision::Reject(reason) => Err(RecordError::Gate(reason)),
            GateDecision::Allow => {
                let entry = CoherenceEntry::from_parts(
                    window_id,
                    ts,
                    phi,
                    stability,
                    success_rate,
                    latency_score,
                    self.ledger.last_hash(),
                );
                self.ledger.push(entry).map_err(RecordError::Ledger)
            }
        }
    }
}

/// Mensagem honesta de recusa do gate (para logs/reports por humano).
#[must_use]
pub fn reject_message(reason: RejectReason, phi_x1e4: u64) -> String {
    match reason {
        RejectReason::NoVerifiedProof => {
            format!("I534-A: sem veredito verified do kernel — sem recompensa/avanço (Φ×10⁴={phi_x1e4})")
        }
        RejectReason::BelowFloor => {
            format!("I534-B: Φ×10⁴={phi_x1e4} ≤ piso {FLOOR_GAP1_X1E4} — fora da banda Gap-1")
        }
        RejectReason::AboveCeiling => {
            format!("I534-C: Φ×10⁴={phi_x1e4} > teto {CEILING_GAP1_X1E4} — fora da banda Gap-1")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use arkhe_field_stability::ledger::GRAVITY_1;

    fn steps(len: usize) -> Vec<Vec<TranscriptStep>> {
        if len == 0 {
            return vec![Vec::new()];
        }
        let options = [
            TranscriptStep { verdict: GateVerdict::Verified, commit: false },
            TranscriptStep { verdict: GateVerdict::Verified, commit: true },
            TranscriptStep { verdict: GateVerdict::Rejected, commit: false },
            TranscriptStep { verdict: GateVerdict::Rejected, commit: true },
        ];
        let mut out = Vec::new();
        for p in steps(len - 1) {
            for s in options {
                let mut q = p.clone();
                q.push(s);
                out.push(q);
            }
        }
        out
    }

    #[test]
    fn i534a_rejected_never_allowed() {
        for phi in [0, 1, 5774, 5775, 8500, 9999, 10_000] {
            assert!(!allowed(GateVerdict::Rejected, phi), "I534-A: Φ={phi}");
            assert_eq!(
                evaluate(GateVerdict::Rejected, phi),
                GateDecision::Reject(RejectReason::NoVerifiedProof)
            );
        }
    }

    #[test]
    fn i534b_below_floor_blocked() {
        for phi in [0, 1, 5773, 5774] {
            assert!(!allowed(GateVerdict::Verified, phi), "I534-B: Φ={phi}");
            assert_eq!(
                evaluate(GateVerdict::Verified, phi),
                GateDecision::Reject(RejectReason::BelowFloor)
            );
        }
    }

    #[test]
    fn i534c_above_ceiling_blocked() {
        for phi in [10_000, 99_999] {
            assert!(!allowed(GateVerdict::Verified, phi), "I534-C: Φ={phi}");
            assert_eq!(
                evaluate(GateVerdict::Verified, phi),
                GateDecision::Reject(RejectReason::AboveCeiling)
            );
        }
    }

    #[test]
    fn i534d_authorized_implies_verified() {
        for phi in [5775, 8500, 9999] {
            assert!(allowed(GateVerdict::Verified, phi));
            assert_eq!(evaluate(GateVerdict::Verified, phi), GateDecision::Allow);
        }
        assert!(!allowed(GateVerdict::Verified, 5774), "piso é exclusivo");
        assert!(allowed(GateVerdict::Verified, 9999), "teto é inclusivo");
    }

    #[test]
    fn i534e_band_closed_under_retention() {
        // Grelha: permitidos (5774, 9999] × retenções k ≤ teto.
        let allowed_phis = [5775, 7000, 8500, 9800, 9999];
        let kk = [0, 1, 5000, 9000, 9999];
        for &phi in &allowed_phis {
            for &k in &kk {
                let retained = retained_after(phi, k);
                assert!(
                    retained <= CEILING_GAP1_X1E4,
                    "I534-E: Φ={phi}, k={k} → {retained} > teto"
                );
            }
        }
        // Pior caso estrito do modelo I530-G: 9998 < 9999.
        assert_eq!(retained_after(9999, 9999), 9998);
    }

    #[test]
    fn int_band_strictly_inside_f64_gap1() {
        // A banda inteira conservadora (5774, 9999] ×10⁻⁴ está estritamente
        // dentro da faixa f64 real `gap1_satisfied` (0.577350…, 0.9999].
        for phi_x1e4 in 5_775..=9_999 {
            assert!(gap1_satisfied(phi_x1e4 as f64 / SCALE_X1E4 as f64), "Φ={phi_x1e4}");
        }
    }

    #[test]
    fn i535a_commits_never_exceed_proofs_when_wellformed() {
        // Enumeração exaustiva: todos os transcriptos de comprimento 0..=5
        // (4^0+…+4^5 = 1365). O predicado é a implicação material
        // `well_formed ⟹ commits ≤ provas` (bem-formados cumprem I535-A).
        for len in 0..=5 {
            for t in steps(len) {
                assert_eq!(
                    i535a_commits_le_proofs(&t),
                    !is_well_formed(&t) || commitments(&t) <= proofs(&t),
                    "predicado divergiu da implicação para {t:?}"
                );
                if is_well_formed(&t) {
                    assert!(
                        commitments(&t) <= proofs(&t),
                        "I535-A falhou para {t:?} (well-formed)"
                    );
                }
            }
        }
    }

    #[test]
    fn i535a_non_vacuity_rejected_commit_exposes_violation() {
        // Fora da restrição, a violação é alcançável (commit sem verified):
        // `well_formed` denuncia o passo — é a hipótese que faz o trabalho.
        // A implicação material `well_formed ⟹ commits ≤ provas` é vacuamente
        // verdade para mal-formados; a conta expõe a violação.
        let bad = [TranscriptStep { verdict: GateVerdict::Rejected, commit: true }];
        assert!(!is_well_formed(&bad), "commit sem verified tem de ser mal-formado");
        assert_eq!(commitments(&bad), 1);
        assert_eq!(proofs(&bad), 0);
        assert!(
            commitments(&bad) > proofs(&bad),
            "conta deve expor commits sem provas"
        );
        assert!(
            i535a_commits_le_proofs(&bad),
            "I535-A é a implicação: mal-formado ⟹ vacuamente satisfeita"
        );
    }

    #[test]
    fn verdict_from_kernel_sound_mapping() {
        assert_eq!(GateVerdict::from_kernel_sound(true), GateVerdict::Verified);
        assert_eq!(GateVerdict::from_kernel_sound(false), GateVerdict::Rejected);
    }

    #[test]
    fn phi_to_x1e4_rounding_and_rejects() {
        assert_eq!(phi_to_x1e4(0.9646), Some(9646));
        assert_eq!(phi_to_x1e4(0.9999), Some(9999));
        assert_eq!(phi_to_x1e4(1.0), Some(10_000));
        assert_eq!(phi_to_x1e4(0.5), Some(5000));
        assert_eq!(phi_to_x1e4(f64::NAN), None);
        assert_eq!(phi_to_x1e4(f64::INFINITY), None);
        assert_eq!(phi_to_x1e4(1.5), None);
        assert_eq!(phi_to_x1e4(-0.1), None);
    }

    #[test]
    fn audit_rejects_without_verified() {
        let mut audit = GateAudit::new();
        let err = audit.try_record(0, 1, GateVerdict::Rejected, 0.9517, 1.0, 0.96);
        assert!(matches!(err, Err(RecordError::Gate(RejectReason::NoVerifiedProof))));
        assert_eq!(audit.ledger().entries().len(), 0, "nada gravado sem verified");
    }

    #[test]
    fn audit_rejects_outside_band() {
        let mut audit = GateAudit::new();
        // Φ=0.5 < piso (V3, rejeitado por Gap-1).
        let low = audit.try_record(0, 1, GateVerdict::Verified, 0.5, 0.5, 0.5);
        assert!(matches!(low, Err(RecordError::Gate(RejectReason::BelowFloor))));
        // Φ=1.0 > teto.
        let high = audit.try_record(1, 2, GateVerdict::Verified, 1.0, 1.0, 1.0);
        assert!(matches!(high, Err(RecordError::Gate(RejectReason::AboveCeiling))));
        assert_eq!(audit.ledger().entries().len(), 0, "nada gravado fora da banda");
    }

    #[test]
    fn audit_records_allowed_and_integrity_ok() {
        let mut audit = GateAudit::new();
        // Ω=0.9517, Σ=1.0, Λ=0.96 → Φ=0.9646 (canônico, I511 banda).
        audit.try_record(0, 1, GateVerdict::Verified, 0.9517, 1.0, 0.96).unwrap();
        audit.try_record(1, 2, GateVerdict::Verified, 0.9517, 1.0, 0.96).unwrap();
        let ledger = audit.ledger();
        assert_eq!(ledger.entries().len(), 2);
        assert_eq!(ledger.verify_integrity(), arkhe_field_stability::IntegrityStatus::Ok);
        assert!((ledger.mean_phi() - 0.9646).abs() < 1e-4);
    }

    #[test]
    fn audit_gravity1_still_applies() {
        let mut audit = GateAudit::new();
        audit.try_record(0, 5, GateVerdict::Verified, 0.9517, 1.0, 0.96).unwrap();
        let err = audit.try_record(1, 5, GateVerdict::Verified, 0.9517, 1.0, 0.96);
        assert!(matches!(err, Err(RecordError::Ledger(msg)) if msg.contains(GRAVITY_1)));
        assert_eq!(audit.ledger().entries().len(), 1, "cadeia intacta (Loopseal-2)");
    }

    #[test]
    fn reject_message_honest_and_typed() {
        assert!(reject_message(RejectReason::NoVerifiedProof, 8500).contains("I534-A"));
        assert!(reject_message(RejectReason::BelowFloor, 5774).contains("I534-B"));
        assert!(reject_message(RejectReason::AboveCeiling, 10_000).contains("I534-C"));
    }
}