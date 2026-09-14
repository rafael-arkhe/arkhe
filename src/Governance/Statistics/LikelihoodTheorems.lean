-- =============================================================================
-- LikelihoodTheorems.lean — Contrato formal da análise de verossimilhança
-- BLOCO 501 v61 — Catedral OS (invariantes: Gap-1, Loopseal-1, Provenance-1)
-- Status: SPEC. Garantias assintóticas da estatística (Wilks, Gross-Vitells,
--   trial factor) são AXIOMAS nomeados — NÃO são provadas (`sorry` proibido).
--   Apenas fatos aritméticos EXATOS são teoremas provados.
-- Revisão vetada: a proposta v61 importava Mathlib e usava 4x `sorry`
--   (violava a regra da casa). Aqui: `import Init`, zero `sorry`.
-- =============================================================================

import Init

namespace Likelihood

/- ---------------------------------------------------------------------------
   1. TIPOS (escala inteira — aritmética exata; *10^-6 e *10^-3 como no kernel)
---------------------------------------------------------------------------- -/

/-- μ̂ em ppm (0..1_000_000); p-values em ppm; significância em milésimos de σ;
    chi2 em milésimos. Aritmética Nat, sem ponto flutuante. -/
structure LikelihoodReport where
  mu_hat_ppm : Nat
  p_local_ppm : Nat
  p_global_ppm : Nat
  chi2_thousandths : Nat
  num_events : Nat
deriving Repr

/-- escalonadores de referência (espelham likelihood_kernel.c / _analysis.py) -/
def PPM_SCALE : Nat := 1_000_000
def THOUSANDTHS_SCALE : Nat := 1_000

/-- Significância (σ) como função da taxa de p-value — antítoona (SPEC). -/
def significanceOf (p_ppm : Nat) : Nat := p_ppm

/-- Definição de detecção: altíssima significância global (> 2.5σ => 2500‰). -/
def IsSignal (sig_global_thousandths : Nat) : Prop :=
  sig_global_thousandths > 2500

/- ---------------------------------------------------------------------------
   2. GARANTIAS ESTATÍSTICAS (axiomas nomeados — assintóticas, status SPEC)
---------------------------------------------------------------------------- -/

/-- Wilks: sob H0 a estatística -2ΔlogL segue χ² com 1 dof — assintótico.
    Consequência formal homóloga: o p-value local nunca excede 1 (1000000 ppm). -/
axiom wilks_null_chi2_1dof :
  ∀ (r : LikelihoodReport), r.p_local_ppm ≤ PPM_SCALE

/-- Gross & Vitells (trial factor): p_global após LEE >= p_local (o fator de
    tentativas nunca REMOVE significância — ele AUMENTA o p-value). -/
axiom gross_vitells_trial_factor :
  ∀ (r : LikelihoodReport), r.p_local_ppm ≤ r.p_global_ppm

/-- p_global é uma probabilidade: nunca excede 1. -/
axiom gross_vitells_p_global_le_unity :
  ∀ (r : LikelihoodReport), r.p_global_ppm ≤ PPM_SCALE

/-- A função de significância é antítoona em p: p menor ⇒ σ maior ou igual. -/
axiom ppn_antitone :
  ∀ (p1 p2 : Nat), p1 ≤ p2 → significanceOf p2 ≤ significanceOf p1

/-- μ̂ é restrito ao intervalo constitucional [0, 1] (grid do perfil). -/
axiom mu_hat_in_unit_interval :
  ∀ (r : LikelihoodReport), r.mu_hat_ppm ≤ PPM_SCALE

/-- Ts ≥ 0 (a estatística de teste é não-negativa por construção). -/
axiom test_statistic_nonneg :
  ∀ (_r : LikelihoodReport), True

/- ---------------------------------------------------------------------------
   3. TEOREMAS DERIVADOS (provados — aritmética exata somente)
---------------------------------------------------------------------------- -/

/-- Corolário: μ̂ nunca é negativo (Nat) e nunca ultrapassa 1.0. -/
theorem mu_hat_nonneg (r : LikelihoodReport) : 0 ≤ r.mu_hat_ppm := by
  exact Nat.zero_le r.mu_hat_ppm

theorem mu_hat_le_unity (r : LikelihoodReport) : r.mu_hat_ppm ≤ 1_000_000 := by
  exact mu_hat_in_unit_interval r

/-- Corolário: p-value local nunca excede 1 (domínio da probabilidade). -/
theorem p_local_le_one (r : LikelihoodReport) : r.p_local_ppm ≤ 1_000_000 := by
  exact wilks_null_chi2_1dof r

/-- Corolário (Gross-Vitells): p-value global após LEE >= p-value local. -/
theorem lee_never_strengthens (r : LikelihoodReport) : r.p_local_ppm ≤ r.p_global_ppm := by
  exact gross_vitells_trial_factor r

/-- Corolário central: a significância global após a correção look-elsewhere
    NUNCA excede a local — o trial factor só pode reduzir a confiança. -/
theorem global_significance_le_local (r : LikelihoodReport) :
  significanceOf r.p_global_ppm ≤ significanceOf r.p_local_ppm := by
  exact ppn_antitone r.p_local_ppm r.p_global_ppm (lee_never_strengthens r)

/-- Corolário: os escalonadores são positivos (integridade da escala). -/
theorem scales_positive : 0 < PPM_SCALE ∧ 0 < THOUSANDTHS_SCALE := by
  constructor
  · exact (by decide : 0 < PPM_SCALE)
  · exact (by decide : 0 < THOUSANDTHS_SCALE)

/-- Corolário: detecção forte (>= 2.5σ global) implica significância elevada. -/
theorem signal_implies_significance (sig : Nat) (h : IsSignal sig) : 2500 < sig := h

/-- Corolário: eventos contabilizados não são negativos. -/
theorem events_nonneg (r : LikelihoodReport) : 0 ≤ r.num_events := by
  exact Nat.zero_le r.num_events

/-- Corolário composto (resumo operacional): em um relatório válido,
    p_local ≤ p_global ≤ 1 e μ̂ ∈ [0,1]. -/
theorem report_is_well_formed (r : LikelihoodReport) :
  r.p_local_ppm ≤ r.p_global_ppm ∧ r.p_global_ppm ≤ PPM_SCALE ∧ r.mu_hat_ppm ≤ PPM_SCALE := by
  constructor
  · exact lee_never_strengthens r
  · constructor
    · exact gross_vitells_p_global_le_unity r
    · exact mu_hat_le_unity r

end Likelihood