# Ledger — Reparação: honestidade sobre o plano v494.1 vs a cadeia real

**Data:** 2026-09-08 — **Contexto:** Parecer v492.0→v494.1 (Ponte Lean) — **Selo proposto:** `ARKHE-PONTE-LEAN-V494-APROVADO-2026-09-08` (plano paralelo 'clareira')

---

## Conclusão executiva

**O ledger real do monorepo não precisa de reparação.** A "Tarefa 0/1 —
reparação do ledger" do plano v494.1 diz respeito a um **ledger paralelo**
('clareira', blocos 7–982, handover `sha256:ac06c2…b84fc`, invariantes
I601–I606), que **nunca foi canonizado neste repositório**. Este documento
explica a descontinuidade e fixa o tratamento honesto da ponte.

---

## 1. O que existe de facto (auditável, não "para reparar")

A cadeia canónica real é registada por commits (branch `QC-0892`) e consiste
em blocos **1000→1008** (2026-09-06 → 2026-09-08, versão v389.1 no topo):

| Bloco | Conteúdo (resumo) | Registo |
|------|-------------------|---------|
| 1000 | Spec finita `ArkheCoherenceLedger.tla`, TLC PASS | commit (Fase 6) |
| 1001 | Decisões D1–D4 do Plano v377.0 | commit |
| 1002 | Execução Fase 6 (evidência mecânica) + §11 do relatório | commit `d6eefcc` |
| 1003–1004 | Auditorias honestas (Fase 7: divergências linear↔quadrático, crate inexistente) | commits |
| 1005 | Execução Fase 7 (v381.1): SemanticValidity/Loopseal/CoherenceReport | commit `4bdc2ee` |
| 1006 | Auditoria honesta Fase 8 (premissas desatualizadas/sem substrato) | commit `1e0262c` |
| 1007 | Execução firmware Fase 1 (v389.0, 57/57) | commit `0e7e0fb` |
| 1008 | Fase 2 — ponte dispositivo↔hospedeiro (v389.1, 167/167) | commit `5455ff3` |

Diretórios `bloco_470…998` existem no *working tree*, mas não são nós da
cadeia canónica (a maioria nunca foi registada em commit). Verificar por
`git log --oneline -- <dir>`; a cadeia auditada arranca em 1000.

## 2. A descontinuidade do plano v494.1 (e porque não se "repara")

O plano v494.1 reivindica blocos 1–6 e um "bloco 983" com handover
`sha256:ac06c21143a3aebbfbe53c04d39a4e0e1e7b9696db8cf5aac682276cb80b84fc`.
Este handover **não existe** em nenhum `bloco_*` do repo nem em `git
cat-file` (bloco 983 é fictício; a cadeia real nunca suportou blocos 7–982).

**Decisão de honestidade:** não se "completa" a lacuna com um bloco 7 nem se
ancora a ponte num hash imaginário — isso seria produzir um artefacto
sem substrato, exatamente o que as auditorias reais 1003/1004/1006
reprovaram. A ponte real ancora-se no **núcleo Lean e no crate
`arkhe-lean-bridge`**, provados e testados neste monorepo.

## 3. O que a reparação REAL é

- **Invariantes:** I601–I606 (plano) → reancoradas como **I524–I529** no
  núcleo `src/lean/LeanBridgeNucleus.lean`, com a numeração contínua da
  cadeia real (último usado: I523 em `nuclei/FieldStabilityTLCSpec.lean`).
  A escala é ×10⁴, comum ao núcleo e ao crate.
- **FFI:** o crate `arkhe-lean-bridge` chama o **binário do kernel Lean
  4.33.1** (PATH/`ARKHE_LEAN_BIN`) como único ponto de confiança — jamais
  interpreta provas; confirma `exit 0`, nomes `#check` elaborados e
  fingerprint SHA3-256 (Ghost-1).
- **Registo futuro:** quando decidido, a ponte entra como **bloco_1009**,
  com handover = hash real do conteúdo de bloco_1008 — nunca o handover
  fictício `ac06c2…`.

## 4. Paridade de constantes (F6)

O teste `criteria_constants_parity_with_kernel_accepted_text` compara as
constantes de `FalsificationCriteria::canonical()` (Rust) com o **texto que o
kernel aceitou** (defs de `LeanBridgeNucleus.lean`). Alterar um lado sem o
outro quebra a verificação — canalização da analogia, não identidade.

## 5. Ficheiros desta entrega

- `src/lean/LeanBridgeNucleus.lean` — I524–I529 (10 teoremas, kernel 4.33.1,
  sem Mathlib, sem `sorry`; `lean` exit 0)
- `packages/arkhe-lean-bridge/` — FFI forte + critérios F1–F6 (`cargo test`
  7/7; clippy `-D warnings` exit 0; `unsafe_code = deny`; deps: só `sha3`)
- `docs/e9-falsification-criteria.md` — mapeamento e falsificação