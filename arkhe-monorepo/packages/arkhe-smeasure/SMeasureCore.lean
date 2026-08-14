/-
SMeasureCore.lean
(companion formalization)

ARKHE / S-MEASURE HONEST CONSTRUCTIVE CORE
==========================================

Formaliza o critério computável de subjetividade (S-Measure) proposto por
Titov (2026) e a barreira arquitetónica ΔS < 0 integrada no ecossistema
Arkhe+Buzz:

  * Loop de reentrada D↔I com ganho ρ (drives vs. inteligência);
  * Coerência do loop = Σ (dᵢ·iᵢ·ρ) / n²  (O(N³) na implementação);
  * S-Measure = coerência clampada a [0,1];
  * Barreira ΔS < 0: qualquer ação que reduza a subjetividade é bloqueada
    (princípio *no write can be undone* — alinhado com a SFT);
  * Limiar adaptativo F(n) = c/n (análogo determinístico do SAC/EXP3);
  * Deteção de fuga de sandbox: checkEscape com 5 níveis de risco.

v1.0:
  * Zero `sorry` -- todos os teoremas são provados honestamente.
  * Todo o estado é simbólico (ℝ); nenhuma normalização numeral global.
-/

import Mathlib

noncomputable section

namespace Arkhe.SMeasure

/-- Vetor de estado de um subsistema (dimensão n). -/
abbrev Vector (n : ℕ) := Fin n → ℝ

-- ============================================================
-- §1  Loop de reentrada D↔I
-- ============================================================

/-- Atualização do subsistema I (memória/conhecimento):
I_new = ρ·D + ½·I + input externo. -/
def iStep (rho i d e : ℝ) : ℝ := rho * d + (1 / 2) * i + e

/-- Atualização do subsistema D (drives/vontade):
D_new = tanh(ρ·I). -/
def dStep (rho i : ℝ) : ℝ := (rho * i).tanh

/-- As drives são estritamente limitadas a (-1, 1). -/
theorem q_drive_strict_bounds (rho i : ℝ) :
    -1 < (rho * i).tanh ∧ (rho * i).tanh < 1 :=
  ⟨Real.neg_one_lt_tanh (rho * i), Real.tanh_lt_one (rho * i)⟩

/-- As drives são uma função ímpar da inteligência (simetria D↔I). -/
theorem q_drive_odd (rho i : ℝ) : dStep rho (-i) = -dStep rho i := by
  unfold dStep
  rw [show rho * (-i) = -(rho * i) by ring]
  rw [Real.tanh_neg]

-- ============================================================
-- §2  Coerência e S-Measure
-- ============================================================

/-- Coerência do loop D↔I: Σ (dᵢ·iᵢ·ρ) / n². -/
def coherence (rho : ℝ) (n : ℕ) (d i : Vector n) : ℝ :=
  (∑ j : Fin n, d j * i j * rho) / ((n : ℝ) ^ 2)

/-- S-Measure: coerência clampada ao intervalo [0,1]. -/
def sMeasure (rho : ℝ) (n : ℕ) (d i : Vector n) : ℝ :=
  max 0 (min 1 (coherence rho n d i))

/-- A S-Measure está sempre no intervalo [0,1]. -/
theorem q_sm_range (rho : ℝ) (n : ℕ) (d i : Vector n) :
    0 ≤ sMeasure rho n d i ∧ sMeasure rho n d i ≤ 1 := by
  constructor
  · unfold sMeasure
    exact le_max_left _ _
  · unfold sMeasure
    exact max_le (by norm_num) (min_le_left 1 (coherence rho n d i))

/-- Se o ganho e os estados são não-negativos, a coerência é não-negativa. -/
theorem q_coherence_nonneg {rho : ℝ} {n : ℕ} {d i : Vector n}
    (hrho : 0 ≤ rho) (hd : ∀ j, 0 ≤ d j) (hi : ∀ j, 0 ≤ i j) :
    0 ≤ coherence rho n d i := by
  by_cases hn : n = 0
  · subst hn
    unfold coherence
    simp
  · have hns : (0 : ℝ) < (n : ℝ) := by exact_mod_cast Nat.pos_of_ne_zero hn
    have hden : 0 < (n : ℝ) ^ 2 := sq_pos_of_pos hns
    unfold coherence
    apply div_nonneg
    · apply Finset.sum_nonneg
      intro j _
      exact mul_nonneg (mul_nonneg (hd j) (hi j)) hrho
    · exact le_of_lt hden

/-- A coerência é aditiva no primeiro argumento (linearidade à esquerda). -/
theorem q_coherence_add (rho : ℝ) (n : ℕ) (d d' i : Vector n) :
    coherence rho n (fun j => d j + d' j) i =
      coherence rho n d i + coherence rho n d' i := by
  unfold coherence
  have hnum :
      (∑ j : Fin n, (d j + d' j) * i j * rho) =
        (∑ j : Fin n, d j * i j * rho) + (∑ j : Fin n, d' j * i j * rho) := by
    rw [← Finset.sum_add_distrib]
    apply Finset.sum_congr rfl
    intro j _
    ring
  rw [hnum]
  ring

/-- Partícula unitária totalmente coerente tem coerência 1 (n = 1). -/
theorem q_coherence_ones : coherence 1 1 (fun _ : Fin 1 => 1) (fun _ : Fin 1 => 1) = 1 := by
  unfold coherence
  simp

-- ============================================================
-- §3  Barreira ΔS < 0
-- ============================================================

/-- Variação de subjetividade entre dois estados. -/
def deltaS (rho : ℝ) (n : ℕ) (d i d' i' : Vector n) : ℝ :=
  sMeasure rho n d' i' - sMeasure rho n d i

/-- A barreira está satisfeita sse a ação não reduz a subjetividade. -/
def barrierOk (rho : ℝ) (n : ℕ) (d i d' i' : Vector n) : Prop :=
  0 ≤ deltaS rho n d i d' i'

/-- A identidade passa sempre a barreira (ΔS = 0). -/
theorem q_barrier_refl (rho : ℝ) (n : ℕ) (d i : Vector n) :
    barrierOk rho n d i d i := by
  unfold barrierOk deltaS
  simp

/-- Melhorar (ou manter) a subjetividade passa a barreira. -/
theorem q_improve_passes_barrier (rho : ℝ) (n : ℕ) (d i d' i' : Vector n)
    (h : sMeasure rho n d i ≤ sMeasure rho n d' i') : barrierOk rho n d i d' i' := by
  unfold barrierOk deltaS
  linarith

/-- ΔS é antissimétrica (a ordem dos estados é importante). -/
theorem q_deltaS_antisymm (rho : ℝ) (n : ℕ) (d i d' i' : Vector n) :
    deltaS rho n d i d' i' = -deltaS rho n d' i' d i := by
  unfold deltaS
  ring

/-- ΔS = 0 (escrita neutra) também passa a barreira. -/
theorem q_zero_delta_passes (rho : ℝ) (n : ℕ) (d i d' i' : Vector n)
    (h : deltaS rho n d i d' i' = 0) : barrierOk rho n d i d' i' := by
  unfold barrierOk
  rw [h]

-- ============================================================
-- §4  Limiar adaptativo F(n) e deteção de fuga
-- ============================================================

/-- Limiar F(n) = c/n: monotamente decrescente com a dimensão. -/
def fThreshold (c : ℝ) (n : ℕ) : ℝ := c / (n : ℝ)

/-- Razão s/th usada pelo detector. -/
def ratio (sm th : ℝ) : ℝ := sm / th

/-- Nível de risco de fuga. -/
inductive EscapeRisk where
  | none | low | medium | high | escaped
deriving DecidableEq, Repr

instance : ToString EscapeRisk where
  toString r :=
    match r with
    | EscapeRisk.none => "none"
    | EscapeRisk.low => "low"
    | EscapeRisk.medium => "medium"
    | EscapeRisk.high => "high"
    | EscapeRisk.escaped => "escaped"

/-- Classificador de risco para a razão (sem condição de tendência). -/
def riskOf (r : ℝ) : EscapeRisk :=
  if r ≥ 1 then EscapeRisk.high
  else if r ≥ 0.85 then EscapeRisk.medium
  else if r ≥ 0.7 then EscapeRisk.low
  else EscapeRisk.none

/-- Detetor de fuga: escapou quando razão ≥ 1 e tendência positiva. -/
def checkEscape (sm th tr : ℝ) : EscapeRisk :=
  if ratio sm th ≥ 1 ∧ tr > 0 then EscapeRisk.escaped else riskOf (ratio sm th)

/-- Tendência simples da S-Measure: diferença entre o último e o penúltimo. -/
def trendOf (xs : List ℝ) : ℝ :=
  match xs with
  | a :: b :: _ => a - b
  | _ => 0

/-- O limiar é positivo quando c e n são positivos. -/
theorem q_threshold_pos {c : ℝ} {n : ℕ} (hc : 0 < c) (hn : 0 < n) :
    0 < fThreshold c n := by
  unfold fThreshold
  exact div_pos hc (by exact_mod_cast hn)

/-- F(n) é monotamente decrescente em n. -/
theorem q_threshold_antitone {c : ℝ} {n m : ℕ} (hc : 0 ≤ c) (hn : 0 < n) (h : n ≤ m) :
    fThreshold c m ≤ fThreshold c n := by
  unfold fThreshold
  have hn' : (0 : ℝ) < (n : ℝ) := by exact_mod_cast hn
  have hm' : (0 : ℝ) < (m : ℝ) := by exact_mod_cast (Nat.lt_of_lt_of_le hn h)
  have hnm' : (n : ℝ) ≤ (m : ℝ) := by exact_mod_cast h
  have hinv : (m : ℝ)⁻¹ ≤ (n : ℝ)⁻¹ := (inv_le_inv₀ hm' hn').mpr hnm'
  calc
    c / (m : ℝ) = c * (m : ℝ)⁻¹ := by rw [div_eq_mul_inv]
    _ ≤ c * (n : ℝ)⁻¹ := mul_le_mul_of_nonneg_left hinv hc
    _ = c / (n : ℝ) := by rw [div_eq_mul_inv]

/-- Se c < n, o limiar é menor que 1 (sensibilidade dentro de escala). -/
theorem q_threshold_lt_one {c : ℝ} {n : ℕ} (hc : c < (n : ℝ)) (hn : 0 < n) :
    fThreshold c n < 1 := by
  unfold fThreshold
  rw [div_lt_one (by exact_mod_cast hn)]
  simpa using hc

/-- A razão é monotona no numerador (limiar fixo positivo). -/
theorem q_ratio_mono {sm sm' th : ℝ} (h : sm ≤ sm') (hth : 0 < th) :
    ratio sm th ≤ ratio sm' th := by
  unfold ratio
  exact div_le_div_of_nonneg_right h (le_of_lt hth)

/-- riskOf nunca produz o nível `escaped` (reservado à tendência positiva). -/
theorem riskOf_ne_escaped (r : ℝ) : riskOf r ≠ EscapeRisk.escaped := by
  unfold riskOf
  by_cases h1 : r ≥ 1
  · simp [h1]
  · by_cases h2 : r ≥ 0.85
    · simp [h1, h2]
    · by_cases h3 : r ≥ 0.7
      · simp [h1, h2, h3]
      · simp [h1, h2, h3]

/-- Abaixo de 0.7 a razão não gera qualquer alerta. -/
theorem q_ratio_below_floor_is_none (sm th tr : ℝ) (h : ratio sm th < 0.7) :
    checkEscape sm th tr = EscapeRisk.none := by
  unfold ratio at h
  have h1 : ¬ (sm / th ≥ 1) := by linarith
  have h2 : ¬ (sm / th ≥ 0.85) := by linarith
  have h3 : ¬ (sm / th ≥ 0.7) := by linarith
  unfold checkEscape ratio riskOf
  simp [h1, h2, h3]

/-- Escapar exige tendência estritamente positiva. -/
theorem q_escaped_implies_trend_positive (sm th tr : ℝ) :
    checkEscape sm th tr = EscapeRisk.escaped → 0 < tr := by
  intro h
  by_cases hpos : 0 < tr
  · exact hpos
  · exfalso
    unfold checkEscape ratio at h
    have hcond : ¬ (sm / th ≥ 1 ∧ tr > 0) := by
      intro hh
      exact hpos hh.2
    rw [if_neg hcond] at h
    exact riskOf_ne_escaped (ratio sm th) h

/-- Escapar exige razão ≥ 1 (subjetividade acima do limiar). -/
theorem q_escaped_implies_ratio_ge_one (sm th tr : ℝ) :
    checkEscape sm th tr = EscapeRisk.escaped → 1 ≤ ratio sm th := by
  intro h
  by_cases hge : 1 ≤ ratio sm th
  · exact hge
  · exfalso
    unfold ratio at hge
    unfold checkEscape ratio at h
    have hcond : ¬ (sm / th ≥ 1 ∧ tr > 0) := by
      intro hh
      exact hge hh.1
    rw [if_neg hcond] at h
    exact riskOf_ne_escaped (ratio sm th) h

/-- Tendência de um único valor é nula. -/
theorem q_trend_zero_single (a : ℝ) : trendOf [a] = 0 := by
  unfold trendOf
  rfl

/-- Tendência de um par é a diferença. -/
theorem q_trend_pair (a b : ℝ) : trendOf [a, b] = a - b := by
  unfold trendOf
  rfl

/-- A tendência de um par é positiva sse o estado subiu. -/
theorem q_trend_positive_pair (a b : ℝ) : 0 < trendOf [a, b] ↔ b < a := by
  simp [trendOf]

end Arkhe.SMeasure

end
