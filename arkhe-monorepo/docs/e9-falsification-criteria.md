# Critérios de Falsificação E9 — Ponte Lean (F1–F6)

**Data:** 2026-09-08 — **Contexto:** Plano v494.1 aprovado (selo
`ARKHE-PONTE-LEAN-V494-APROVADO-2026-09-08`, ledger paralelo 'clareira').
Este documento reancora os critérios do plano aos IDs **reais** I524–I529 e
ao crate `arkhe-lean-bridge`.

**Escala única:** ×10⁴ (inteiros de Φ e alvos).
**Convenção Gap-1:** `floor < Φ ≤ floor` com floor = 5774 (0.5774 > 1/√3,
piso estrito) e teto = 9999 (0.9999, teto inclusivo).

| Plano (I6xx) | Critério | Núcleo real | Declaração formal | Falsificado quando | Rust (`criteria.rs`) |
|---|---|---|---|---|---|
| I601 | F1 | I524 | `gclamp` é monótono; banda fechada | Φ < 0.5774 *ou* Φ > 0.9999 numa janela | `f1_in_band` |
| I602 | F2 | I526 | tick estritamente crescente ⟹ avanço ≥ passos (Gravity-1) | handover/ts regressa ou não avança | `f2_strict_advance` |
| I603 | F3 | I525 | dentro da banda, medir ≡ avaliar (ponto fixo) | Φ medido ≠ Φ avaliado (deriva de watermark) | `f3_fixed_point` |
| I604 | F4 | I527 | alvo 0.85 e faixa quieta 0.98 dentro da banda | entrada na banda com Φ < 0.85 (recompensa negada) | `f4_target_reward` |
| I605 | F5 | I528 | colapso 20% (E4) preserva banda: 0.5774 < 0.7214 < 0.98 | Φ < 0.5774 ou ≥ 0.98 sob colapso ≤ 20% | `f5_collapse_preserves_band` |
| I606 | F6 | I529 | teto é exclusivo e transitivo (canalização) | valor acima de 0.9999 entra na banda, ou analogia tratada como identidade | `f6_ceiling_exclusive` |

## Prova de cada critério no núcleo (`src/lean/LeanBridgeNucleus.lean`)

Verificada pelo binário do kernel Lean 4.33.1 (`lean` exit 0), sem Mathlib e
sem `sorry` (10 declarações; `#check` no fim do ficheiro). Estrutura:

- **I524** (F1): `I524A_gclamp_ge_lo`, `I524B_gclamp_le_hi`,
  `I524C_gclamp_monotone` — casos + `omega` (estrutural).
- **I525** (F3): `gclamp` = identidade quando `lo ≤ x ≤ hi`.
- **I526** (F2): indução no kernel — `∀x, x < f x → n ≤ iterate f n 0`
  (não-regressão do tick Gravity-1). **A prova mais forte do ficheiro.**
- **I527** (F4): alvo (8500) e quieto (9800) na banda — `native_decide`.
- **I528** (F5): colapso E4 (7214) entre piso e quieto — `native_decide`.
- **I529** (F6): `I529A_ceiling_exclusive` (teto exclusivo) +
  `I529B_outside_after_ceiling_transitive` (transitividade do canal).

## Fechos de verificação (testes `cargo test -p arkhe-lean-bridge`)

1. `kernel_elaborates_bridge_nucleus_without_sorry` — FFI real ao kernel:
   exit 0, 10/10 nomes `#check` no stdout, stderr vazio, digest íntegro.
2. `fingerpint_ghost1_detects_tampering` — qualquer byte muda o SHA3-256.
3. `criteria_constants_parity_with_kernel_accepted_text` — constante do Rust
   == constante do texto aprovado pelo kernel (F6).
4. `criteria_f1_f4_band_and_reward` / `f2_gravity1_strict_advance` /
   `f3_f5_f6` — comportamento nominal e falsificação de cada critério.
5. `criterion_mapping_plan_to_nucleus` — tabela da página anterior.

## Honestidade e limitações

- **Classe das provas:** sem Mathlib não há `Real`/`sqrt`/Cauchy–Schwarz
  geral; as invariantes numéricas são "fecho-computáveis" exatas (mesma
  classe de I511–I516) nas escalas inteiras declaradas; as estruturais são
  provadas com `omega`/`simp`/`induction`.
- **Não é identidade, é analogia:** nenhum invariante do plano entra neste
  repo como "prova" de um facto que o código Rust não implementa. O único
  ponto de confiança é o **kernel Lean** elaborar as declarações reais sobre
  as constantes que o Rust usa.
- **Não destrutivo:** nenhum `sorry`, nenhum `by rfl` como conteúdo
  semântico vazio, nenhum `unsafe` (lints `unsafe_code = deny`), nenhuma
  dependência externa nova (só `sha3` do workspace).