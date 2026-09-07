# Verificação de evidência — Fase 5 (ArkheKernel / TLA+ / Apalache)

> Bloco 999 — busca exaustiva no **working root do git** (todo o
> `sasc-v34.8-ω-__-real-implementation-engine`, não apenas `arkhe-monorepo`).
> Data: 2026-09-06.

## 1. Arquivos TLA+ (`*.tla`)

`glob **/*.tla` em todo o root → **2 arquivos**:

- `optimization/test/arkhe-modules/ArkheOS/spec/quantum_paxos.tla`
  - Cabeçalho: `----------------- MODULE QuantumPaxos -----------------`
  - Conteúdo: Paxos quântico com mensagens autenticadas (N=4, f=1),
    invariantes TypeInvariant, Consistency, Liveness, pred. Partition.
- `optimization/test/arkhe-modules/ArkheOS/spec/QuantumPBFT.tla`
  - Cabeçalho: `----------------- MODULE QuantumPBFT -----------------`
  - Conteúdo: PBFT para "Quantum Arkhe(n)".

Nenhum dos dois é módulo de **refinamento** `cr1-cr4`; ambos são de
**consenso quântico**.

## 2. Módulos de refinamento cr1-cr4

`rg -li "cr1|cr2|cr3|cr4" optimization/test/arkhe-modules/ArkheOS` → **0**.

O único artefato com "Refinement" no nome é Coq: `spec/ParallaxCore_Refinement.v`.

## 3. Apalache

`rg -li apalache .` (root inteiro, com `--hidden`) → **0 ocorrências em código**.

Ocorrências em `-g "*.yml" -g "*.yaml"` (CI) → **0**.

Workflows existentes (sem Apalache):
- `.github/workflows/agent-vm.yml`
- `.github/workflows/safe-core-monorepo.yml`
- `.github/workflows/web3-security.yml`

## 4. ArkheKernel

`rg -li arkhekernel .` → **0 ocorrências** em código/CI (apenas os registros
de auditoria dos blocos 995–998 e `docs/relatorio_final_fase4.md` apontam a
ausência).

## 5. Estado de versionamento

- `git ls-files -- optimization/` → **0 arquivos** (árvore 100% untracked).
- `git check-ignore -v ...quantum_paxos.tla` → sem regra de exclusão
  (confirmado não-tracked, não-ignored).

## 6. Síntese

| Reivindicação | Realidade |
| :--- | :--- |
| ArkheKernel com módulos TLA+ `cr1-cr4` | Não existe (2 `.tla` de consenso quântico existem, árvore não versionada) |
| Apalache no CI | Não existe (0 referências em qualquer workflow) |
| Refinamento TLA+ do field-stability | Não existe — seria criação (Opção B) ou deferimento (Opção C) |