//! Critérios de falsificação F1–F6 (plano v494.1) reancorados aos IDs reais
//! I524–I529 provados no núcleo Lean.
//!
//! Todos os valores estão em escala inteira **×10⁴** (mesma do núcleo
//! `LeanBridgeNucleus.lean`); a paridade é fechada por teste de integração
//! (FFI) que compara estas constantes com o texto aprovado pelo kernel.

/// Identificador canónico de cada critério de falsificação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriterionId {
    /// Entrada fora da banda Gap-1 (I524 ← I601).
    F1,
    /// Tick/handover não-regressa (I526 ← I602).
    F2,
    /// Ponto fixo da medição (I525 ← I603).
    F3,
    /// Alvo de coerência 0.85 (I527 ← I604).
    F4,
    /// Fronteira de robustez do E4 (I528 ← I605).
    F5,
    /// Canalização do teto (I529 ← I606).
    F6,
}

impl CriterionId {
    /// Correspondência crítica com o núcleo: (plano 'clareira', núcleo real).
    #[must_use]
    pub const fn mapped_invariants(self) -> (&'static str, &'static str) {
        match self {
            CriterionId::F1 => ("I601", "I524C_gclamp_monotone"),
            CriterionId::F2 => ("I602", "I526_strict_walk_advances"),
            CriterionId::F3 => ("I603", "I525_gclamp_fixed_point"),
            CriterionId::F4 => ("I604", "I527A_target_inside_gap1"),
            CriterionId::F5 => ("I605", "I528_collapse_preserves_band"),
            CriterionId::F6 => ("I606", "I529A_ceiling_exclusive"),
        }
    }
}

/// Espelho minimal de uma entrada do ledger coerente (escala ×10⁴).
///
/// Não carrega o crate `arkhe-field-stability` de propósito: a ponte deve
/// manter superfície de dependências mínima (Simplicity-2); a coerência de
/// tipos é coberta no registo do bloco pelos testes FFI + núcleo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeEntry {
    /// Índice da janela (monotónico).
    pub window: u64,
    /// Tick lógico de timestamp (Gravity-1).
    pub ts: u64,
    /// Φ da janela × 10⁴ (inteiro — banda Gap-1 em escala comum ao núcleo).
    pub phi_x1e4: u64,
}

/// Constantes canónicas da banda Gap-1 e dos alvos — paridade bit-a-bit com o
/// núcleo `LeanBridgeNucleus.lean` (fechada por teste FFI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FalsificationCriteria {
    /// Piso estrito Gap-1 ×10⁴ (`I527A` usa `5774 < 8500`).
    pub floor_gap1_x1e4: u64,
    /// Teto inclusivo Gap-1 ×10⁴ (`I529A` usa `≤ 9999`).
    pub ceiling_gap1_x1e4: u64,
    /// Alvo de coerência do E9 ×10⁴ (`I527A_target_inside_gap1`).
    pub target_phi_e9_x1e4: u64,
    /// Faixa quieta do E4 ×10⁴ (`I527B_quiet_inside_gap1`).
    pub quiet_phi_e9_x1e4: u64,
    /// Φ do colapso 20% do E4 ×10⁴ (`I528_collapse_preserves_band`).
    pub collapse_phi_e4_x1e4: u64,
}

impl FalsificationCriteria {
    /// Valores canónicos (vértices do núcleo — verificados por teste contra o
    /// texto aprovado pelo kernel).
    #[must_use]
    pub const fn canonical() -> Self {
        Self {
            floor_gap1_x1e4: 5774,
            ceiling_gap1_x1e4: 9999,
            target_phi_e9_x1e4: 8500,
            quiet_phi_e9_x1e4: 9800,
            collapse_phi_e4_x1e4: 7214,
        }
    }

    /// F1 (I524 ← I601): a entrada está DENTRO da banda aberta-à-direita
    /// `(floor, ceiling]`.
    #[must_use]
    pub fn f1_in_band(&self, e: &BridgeEntry) -> bool {
        self.floor_gap1_x1e4 < e.phi_x1e4 && e.phi_x1e4 <= self.ceiling_gap1_x1e4
    }

    /// F2 (I526 ← I602): o tick/handover avança estritamente (não-regressão de
    /// Gravity-1). Sem prévia (génese) → satisfeito.
    #[must_use]
    pub fn f2_strict_advance(&self, prev: Option<&BridgeEntry>, cur: &BridgeEntry) -> bool {
        match prev {
            None => true,
            Some(p) => p.ts < cur.ts,
        }
    }

    /// F3 (I525 ← I603): ponto fixo da medição — Φ medido é EXATAMENTE o
    /// avaliado (sem deriva de watermark). Na banda, medir não altera o valor.
    #[must_use]
    pub fn f3_fixed_point(&self, measured_x1e4: u64, evaluated_x1e4: u64) -> bool {
        measured_x1e4 == evaluated_x1e4
    }

    /// F4 (I527 ← I604): critério de recompensa — entrada na banda E Φ ≥ 0.85.
    #[must_use]
    pub fn f4_target_reward(&self, e: &BridgeEntry) -> bool {
        self.f1_in_band(e) && self.target_phi_e9_x1e4 <= e.phi_x1e4
    }

    /// F5 (I528 ← I605): fronteira de robustez do E4 — o colapso 20% preserva a
    /// banda `(piso, quieto]`.
    #[must_use]
    pub fn f5_collapse_preserves_band(&self) -> bool {
        self.floor_gap1_x1e4 < self.collapse_phi_e4_x1e4
            && self.collapse_phi_e4_x1e4 < self.quiet_phi_e9_x1e4
    }

    /// F6 (I529 ← I606): canalização — valor acima do teto NUNCA entra na banda.
    #[must_use]
    pub fn f6_ceiling_exclusive(&self, phi_x1e4: u64) -> bool {
        self.ceiling_gap1_x1e4 < phi_x1e4
    }
}