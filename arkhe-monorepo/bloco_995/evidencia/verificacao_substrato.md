# Verificação de substrato — Fases 5–8 (proposta do Arquiteto)

> Bloco 995 — auditoria de evidência. Data: 2026-09-06.
> Critério: mesmo precedente I461/I462 (bloco 990) — recomendação sem
> substrato no monorepo é registrada e deferida, nunca adotada por fé.

## 1. Arquivos TLA+

Comando: glob `**/*.tla` (recursivo, todo o monorepo).

Resultado: **0 arquivos**. Nenhum módulo TLA+ existe.

## 2. Apalache

Grep `Apalache` (incluindo `*.{tla,toolbox,cfg,toml,md,tex}`).

Resultado: **0 ocorrências**. Não há integração Apalache no CI nem em docs.

## 3. Kani (Fase 7)

Grep `Kani` / `kani`.

Resultado: **0 ocorrências**. Kani não está presente em lugar algum do monorepo.

## 4. Referências "cr1-cr4" / refinamento

Grep `cr[1-4]` / `cr1_cr[r]?4`:

- Matches restritos a **artefatos de build bytecode**: arquivos `.o`/`.d`
  incrementais em `catedral-os-v304\rust\target\debug\incremental`.
- **Nenhum módulo TLA+ de refinamento** denominado `cr1`–`cr4`.

## 5. ArkheKernel / Leslie / Veil

Grep `ArkheKernel`, `Leslie`, `Veil`:

- **ArkheKernel:** nenhum match de módulo TLA+ de refinamento.
- **Leslie/Veil:** ocorrências triviais e não relacionadas (nomes próprios em
  man pages SWI-Prolog, bibliotecas Lean incompletas) — sem o projeto Leslie
  nem a peça arkhe-veil alegados na proposta.

## 6. Marcadores de linguagem TLA+

Grep `SPECIFICATION|Apalache|TLC|ktams` terminando o scope
`*.{tla,toolbox,cfg,toml,md,tex}`:

- Apenas "SPECIFICATIONS" em documentos não-técnicos:
  - `catedral_deps/nexus_agi/archive/MEGA_DEPLOYMENT_PROGRESS_REPORT.md`
  - `catedral_deps/nexus_agi/archive/LIGHTNING_NETWORK_GUIDE.md`
- Nenhum cabeçalho de módulo TLA+ (`---- MODULE`), nenhuma linha
  `SPECIFICATION` de prova.

## 7. O que EXISTE de fato (bases formais reais)

| Item | Local | Verificado |
| :--- | :--- | :--- |
| Núcleo Lean 4 (I500–I504) | `src/lean/Substrate924.lean` | ✅ provas sem Mathlib/sorry, bloco 971 |
| Núcleo Lean 4 (I505–I510) | `src/lean/SubstrateBitcoin972.lean` | ✅ provas sem Mathlib/sorry, bloco 972 |
| TemporalChain append-only + SHA3-256 | substrate 923, `sophia-v5`, `catedral_os_v152/core/temporal_chain.py` | ✅ ancoragem Loopseal-1 real (Python) |
| Cadeia de dados CoherenceLedger | `packages/arkhe-field-stability/src/ledger.rs` | ✅ Fase 4 (bloco 994), 44/44 |

## 8. Síntese

A premissa portante da **Fase 5** ("refinar módulos TLA+ existentes cr1-cr4
com Apalache no CI") **não tem substrato**. As bases formais reais são o
núcleo Lean 4 (Fundação para Fase 6) e o TemporalChain Python (ancoragem,
não TLA+). Kani (Fase 7) não existe no monorepo.

Decisão registrada em `bloco_995/README.md` com as opções A (TLA+ novo),
B (deferir), C (re-especificar) — aguardando o Arquiteto.

## Comandos executados (registro)

```
# glob **/*.tla                  -> 0
# grep 'Apalache'                 -> 0
# grep 'Kani'                     -> 0
# grep 'Specification|Apalache|TLC|ktams' -> 2 (docs nao-tecnicos)
# grep 'ArkheKernel|cr1_cr4|Leslie|Veil'  -> 114 (artefatos build + falsos positivos)