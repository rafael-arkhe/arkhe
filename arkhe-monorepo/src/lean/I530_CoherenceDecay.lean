/- ============================================================================
   I530_CoherenceDecay — Invariante I530 (DECAIMENTO DE COERÊNCIA EM [0,1])
   Catedral OS — Extensão field-stability (bloco 1009, v390.0).
   Núcleo Lean 4 (v4.33.1), SEM Mathlib — convenção do repositório
   (blocos 966/971/972/996/1009: núcleos core com `omega`, `native_decide`,
   `simp`; proibido `sorry`; proibido `import Mathlib`).

   CORRECÇÃO DAS DEFEITOS DO PLANO (documento v509.1 proposto):
     1. o plano propunha f64 + `import Mathlib` (ℝ, `mul_nonneg`, `mul_le_one`)
        e um caminho `crates/arkhe-field-stability` — viola a convenção bloco 966
        e o caminho real do workspace (`packages/arkhe-field-stability`).
     2. este ficheiro é a realização CORE (sem Mathlib) em inteiros naturais,
        escala fixa ×10⁴, idêntica aos núcleos I511–I516 e I524–I529.
     3. registro do bloco real usa `handover_anterior` inteiro (1008), nunca
        um marcador "sha256:___"; o hash é calculado sobre o JSON canónico
        (evidencia/, bruto+gzip).

   ESCALA: ×10⁴ — os operandos de coerência e retenção (c, k) e o valor decaído
   são inteiros quantizados: c·k (f64) ⇄ `(C·K)/10_000` (Nat).
     escala   = 10000  (1.0)
     teto     = 9999   (0.9999 — teto inclusivo Gap-1)
   ============================================================================ -/

namespace ArkheCoherenceDecay

/-- Escala inteira ×10⁴ (1.0 da banda [0,1] em Quant). -/
def scale_x1e4 : Nat := 10000

/-- Teto inclusivo Gap-1 em escala ×10⁴ (0.9999). -/
def ceiling_x1e4 : Nat := 9999

/-- Coerência decaída em escala ×10⁴: `floor(C·K / 10⁴)`. -/
def decayed (c k : Nat) : Nat := c * k / scale_x1e4

/- ==========================================================================
   I530-A — NÃO NEGATIVIDADE (piso 0)
   A coerência decaída nunca é negativa: produtos/dividendos naturais são
   sempre ≥ 0. (A "banda [0,1]"-invariante tem aqui o piso 0 trivial.)
   ========================================================================== -/

/-- I530-A: o decaimento nunca produz valor negativo. -/
theorem I530A_decayed_nonneg (c k : Nat) : 0 ≤ decayed c k := by
  exact Nat.zero_le (decayed c k)

/- ==========================================================================
   I530-B — CLAUSURA DO PRODUTO (c,k ≤ 1.0 ⟹ C·K ≤ 1.0²)
   Em representação ×10⁴, o produto bruto C·K de dois operandos dentro da
   banda unitária não pode exceder 10000·10000 (a unidade ao quadrado).
   ========================================================================== -/

/-- I530-B: produto bruto dentro da banda unitária. -/
theorem I530B_product_within_unit (c k : Nat)
    (hc : c ≤ scale_x1e4) (hk : k ≤ scale_x1e4) :
    c * k ≤ scale_x1e4 * scale_x1e4 := by
  have h1 : c * k ≤ scale_x1e4 * k := Nat.mul_le_mul_right k hc
  have h2 : scale_x1e4 * k ≤ scale_x1e4 * scale_x1e4 := Nat.mul_le_mul_left scale_x1e4 hk
  exact Nat.le_trans h1 h2

/- ==========================================================================
   I530-C — VALOR DECAÍDO ≤ 1.0 (teto da banda unitária)
   `decayed c k = C·K/10⁴ ≤ 10⁴` quando c,k ≤ 10⁴: a divisão pela escala traz
   o produto de volta para o intervalo unitário.
   ========================================================================== -/

/-- I530-C: o valor decaído permanece ≤ 1.0 (escala ×10⁴). -/
theorem I530C_decayed_within_unit (c k : Nat)
    (hc : c ≤ scale_x1e4) (hk : k ≤ scale_x1e4) :
    decayed c k ≤ scale_x1e4 := by
  unfold decayed
  have hprod : c * k ≤ scale_x1e4 * scale_x1e4 := I530B_product_within_unit c k hc hk
  calc
    c * k / scale_x1e4 ≤ (scale_x1e4 * scale_x1e4) / scale_x1e4 := by
      exact Nat.div_le_div_right hprod
    _ = scale_x1e4 := by
      native_decide

/- ==========================================================================
   I530-D — PRESERVAÇÃO DO TETO GAP-1
   c,k ≤ 0.9999 (teto inclusivo) ⟹ decaído ≤ 0.9999: a retenção de dois valores
   in-band nunca empurra a coerência para fora da banda pelo topo.
   Conservador por construção (divisão trunca para baixo).
   ========================================================================== -/

/-- I530-D: operandos dentro do teto Gap-1 produzem decaído dentro do teto. -/
theorem I530D_decayed_within_ceiling (c k : Nat)
    (hc : c ≤ ceiling_x1e4) (hk : k ≤ ceiling_x1e4) :
    decayed c k ≤ ceiling_x1e4 := by
  unfold decayed
  have h1 : c * k ≤ ceiling_x1e4 * k := Nat.mul_le_mul_right k hc
  have h2 : ceiling_x1e4 * k ≤ ceiling_x1e4 * ceiling_x1e4 :=
    Nat.mul_le_mul_left ceiling_x1e4 hk
  have hprod : c * k ≤ ceiling_x1e4 * ceiling_x1e4 := Nat.le_trans h1 h2
  calc
    c * k / scale_x1e4 ≤ (ceiling_x1e4 * ceiling_x1e4) / scale_x1e4 := by
      exact Nat.div_le_div_right hprod
    _ = 9998 := by
      native_decide
    _ ≤ ceiling_x1e4 := by
      native_decide

/- ==========================================================================
   I530-G — DECAÍDO MÁXIMO É STRICTO ABAIXO DO TETO
   No pior caso (c = k = 0.9999), `(9999·9999)/10⁴ = 9998 < 9999`: a retenção
   de dois valores in-band nunca atinge o teto — margem constitucional viva.
   ========================================================================== -/

/-- I530-G: pior caso nunca toca o teto (9998 < 9999). -/
theorem I530G_worst_case_strictly_below_ceiling :
    (ceiling_x1e4 * ceiling_x1e4) / scale_x1e4 < ceiling_x1e4 := by
  native_decide

/- ==========================================================================
   I530-E — MONOTONIA NO C (coerência atual)
   Maior c (dada a mesma retenção) nunca produz decaído menor: a função de
   decaimento preserva a ordem por componente no retanto — análogo estrutural
   do clamp monotónico I524-C do núcleo da ponte Lean.
   ========================================================================== -/

/-- I530-E: monotonia no componente de coerência atual. -/
theorem I530E_decayed_monotone_c (c1 c2 k : Nat) (hc : c1 ≤ c2) :
    decayed c1 k ≤ decayed c2 k := by
  unfold decayed
  have h : c1 * k ≤ c2 * k := Nat.mul_le_mul_right k hc
  exact Nat.div_le_div_right h

/- ==========================================================================
   I530-F — MONOTONIA NO K (taxa de retenção)
   Maior retenção (dada a mesma coerência atual) nunca produz decaído menor.
   ========================================================================== -/

/-- I530-F: monotonia na taxa de retenção. -/
theorem I530F_decayed_monotone_k (c k1 k2 : Nat) (hk : k1 ≤ k2) :
    decayed c k1 ≤ decayed c k2 := by
  unfold decayed
  have h : c * k1 ≤ c * k2 := Nat.mul_le_mul_left c hk
  exact Nat.div_le_div_right h

/- ==========================================================================
   I530 — ENUNCIADO PRINCIPAL (conjunção da banda [0,1] + teto Gap-1)
   c,k ≤ 1.0 ⟹ decaído ∈ [0, 1.0]; com c,k ≤ teto o decaído permanece no teto.
   ========================================================================== -/

/-- I530 principal: banda [0,1] e preservação do teto Gap-1. -/
theorem I530_decay_bounded (c k : Nat)
    (hc : c ≤ scale_x1e4) (hk : k ≤ scale_x1e4)
    (hcb : c ≤ ceiling_x1e4) (hkb : k ≤ ceiling_x1e4) :
    0 ≤ decayed c k ∧ decayed c k ≤ scale_x1e4 ∧ decayed c k ≤ ceiling_x1e4 := by
  constructor
  · exact I530A_decayed_nonneg c k
  · constructor
    · exact I530C_decayed_within_unit c k hc hk
    · exact I530D_decayed_within_ceiling c k hcb hkb

/- ==========================================================================
   Verificação explícita das elaborações — consumida pelo crate Rust
   `arkhe-lean-bridge` (FFI ao kernel): nomes esperados no stdout do kernel.
   ========================================================================== -/

#check I530A_decayed_nonneg
#check I530B_product_within_unit
#check I530C_decayed_within_unit
#check I530D_decayed_within_ceiling
#check I530E_decayed_monotone_c
#check I530F_decayed_monotone_k
#check I530G_worst_case_strictly_below_ceiling
#check I530_decay_bounded

end ArkheCoherenceDecay