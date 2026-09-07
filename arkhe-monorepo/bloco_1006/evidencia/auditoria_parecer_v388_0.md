# Auditoria de Fonte Primária — Parecer Executivo v388.0 (Fase 8)

**Bloco:** 1006 · **Data:** 2026-09-07 · **Tipo:** AUDITORIA_PARECER_FASE8
**Método:** verificação contra o monorepo rastreado (fonte primária), sem aceitar premissas por fé.

---

## 1. Resumo do parecer v388.0

O parecer propõe o bloco 1006 `UPGRADE_FASE_7` e a **Fase 8**:

| Tarefa | Invariante(s) | Proposta |
| :--- | :--- | :--- |
| F7-0 | I532 | Lean: quórum semântico `> 2/3` (limiar `floor(2n/3)+1`) |
| F7-1 | I533 | Lean: cadeia hash acíclica |
| F7-2 | I461–I465 | TensorNetwork (CollapseDetector, malha de tensores) |
| F7-3 | I531 | VisualSelfModel (auto-observação / OOD) |
| F7-4 | — | "Implementação do IterativeRefiner (Fase 2)" como módulo novo |
| — | — | Novo núcleo `/src/lean/FieldStability.lean (v388.0)` |

---

## 2. Verificação item a item

### 2.1 F7-4 — IterativeRefiner: PREMISSA DESATUALIZADA (verificado: já existe)

`src/refiner.rs` (314 linhas) contém, desde a **Fase 2**:

- `pub struct IterativeRefiner` com `Default`, `new()`, `refine()`, `refine_traced()`.
- **Φ como função objetivo** (doc da linha 1: *"iterative refinement with Φ as objective"*);
  gradiente numérico por diferença central sobre `(Ω, Σ, Λ)`, pesos canônicos `WEIGHTS_DEFAULT`.
- Critérios de parada: `ReachedTarget` (Φ > 0.95), `Converged` (variação < 1e-6), `MaxIterations`.
- Backtracking monotônico: Φ nunca decresce.
- Testes: `test_refiner_reaches_target_from_v2` (V2 Φ=0.6983 → Φ>0.95),
  `test_refiner_already_at_target`, `test_refiner_from_null_floor_climbs`,
  `test_refiner_converges_with_tiny_step`, `test_refiner_components_stay_in_domain`,
  `test_refiner_monotonic_increases_phi`, `test_refiner_traced_matches_result_and_is_monotone`,
  entre outros.
- Re-exportado via `lib.rs`.

**Conclusão:** a proposição "implementar o IterativeRefiner (Fase 2)" está
**satisfeita por premissa**; o parecer descreve o módulo como se ainda não
existisse. Nenhuma ação de implementação é necessária.

### 2.2 F7-2 — TensorNetwork (I461–I465): SEM SUBSTRATO

- `git grep -E "TensorNetwork|CollapseDetector|out-of-distribution|I530|I531|VisualSelfModel"` sobre arquivos rastreados → **0 ocorrências**.
- `grep "TensorNetwork|CollapseDetector|contract()|intrinsic_dim"` em `*.rs` → único match: `catedral_deps/nextgsim/nextgsim-she/src/sla.rs:384` (`test_sla_contract`) — teste de SLA, não relacionado.
- **I461/I462 já foram DEFERIDOS** por falta de substrato:
  - `docs/relatorio_final_fase4.md:70` — *"deferidos — sem substrato"*.
  - Precedente blocos 991/995/996 e memória arquitetural (Fases 5/7: TLA+/Apalache e Kani deferidas — 0 `.tla`, 0 substrato).

**Conclusão:** F7-2 **DEFERIDO**. O parecer trata I461–I465 como reais quando o
registro formal do monorepo os define como deferidos.

### 2.3 F7-3 — VisualSelfModel (I531): SEM SUBSTRATO

- `git grep` rastreado para `I530|I531|VisualSelfModel|out-of-distribution` → **0 ocorrências**.
- Não há evidência de I530/I531 como invariantes registrados no monorepo.

**Conclusão:** F7-3 **DEFERIDO**, mesmo critério do precedente.

### 2.4 F7-0/F7-1 — I532/I533 (Lean): CONDICIONADOS

**São propostas de novos invariantes**, não correções de existentes. Dois problemas honestos:

**a) O teorema `by rfl` é tautológico.** A semântica real do quórum (Rust):

```rust
// src/validators.rs:24, 60–77
pub const VALIDATOR_QUORUM: f64 = 2.0 / 3.0;   // 2/3 exato (estrito: >)
is_satisfied == total >= MIN_VALIDATORS && ratio > VALIDATOR_QUORUM
// ratio = approved as f64 / total as f64
```

O esboço do parecer define `quorum_threshold n := (2*n)/3 + 1` e declara o
teorema **`by rfl`** — identidade de definição, **não** uma prova de que o
limiar inteiro é equivalente a `approved/total > 2/3` para todo `n` (casos de
borda da discretização). Formalizar essa ponte é trabalho de demonstração real.

**b) Violação da convenção canônica dos núcleos.** O esboço importa
`Mathlib.Data.Real.Basic`; todos os núcleos aceitos usam **Lean 4 core, sem
Mathlib, sem `sorry`, kernel v4.33.1** (I511–I516 Fase 4; Fase 6). Além disso
referencia tipos não definidos (`Validator`, `Verdict`, `Hash`), usa
`Verdict.Accept` (Rust real: `Verdict::Approve`) e indexa
`results[i]`/`chain[i]` sem definição — **não compila**.

**Conclusão:** F7-0/F7-1 podem prosseguir **somente após decisão executiva** e
como novos artefatos Lean com prova honesta da ponte e convenção sem-Mathlib.

### 2.5 /src/lean/FieldStability.lean (v388.0): INEXISTENTE

- `Test-Path src/lean/FieldStability.lean` → **False**.
- Núcleos reais: `src/lean/SubstrateFieldStabilityCoherence.lean` (I511–I516) e
  `src/lean/nuclei/nuclei/FieldStabilityTLCSpec.lean`.

---

## 3. Veredito

| Item | Veredito | Motivo |
| :--- | :--- | :--- |
| F7-4 | 🟢 Satisfeito por premissa | IterativeRefiner existe e é testado desde a Fase 2 |
| F7-2 | 🔴 Deferido | Sem substrato; I461/I462 já deferidos |
| F7-3 | 🔴 Deferido | Sem substrato; I530/I531 inexistentes |
| F7-0/F7-1 | 🟡 Condicionado | Requer decisão + prova honesta de ponte + convenção sem-Mathlib |
| FieldStability.lean | 🔴 Inexistente | Núcleos reais são outros |

**Status:** `REGISTRO_DIVERGENCIAS_AGUARDA_DECISAO` — nenhuma implementação da
Fase 8 foi iniciada.

---

## 4. Métodos de verificação (reproduzíveis)

```text
git grep -nE "I530|I531|VisualSelfModel|CollapseDetector|TensorNetwork|out-of-distribution" -- arkhe-monorepo/   # exit 1 (0 ocorrências)
git grep -n -E "contract\(\)|intrinsic_dim" -- "*.rs"                                                              # só sla.rs:384 (não relacionado)
Test-Path src/lean/FieldStability.lean                                                                             # False
git ls-files packages/arkhe-field-stability/src/bin/                                                               # só e1.rs rastreado (e2–e4 on-disk + Cargo.toml)
```

**Selo:** `CATEDRAL-OS-AUDITORIA-PARECER-V388-0-FASE-8-2026-09-07`