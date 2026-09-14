# AVALON ARCHITECTURE UPGRADE v2.1 -> v2.3

**Date:** 2026-08-16
**Status:** OPERATIONAL
**Classification:** Upgrade Manifest - Topology-General Ensemble + AST Linter Completion

## Executive Summary

The v2.3 upgrade has two goals inherited from the Arquiteto-\U0003a8 audit:

1. **Generalizar topologicamente** o núcleo Kuramoto acelerado (numba com fallback numpy) e responder, por varredura empírica, se a proximidade do `eps_sq_TUT` a `1/phi` é uma coincidência específica do honeycomb ou um fenômeno universal.
2. **Completar o linter AST**: a auditoria marcou `_check_ast_forbidden_patterns` como CRÍTICO/vazio; agora ele realiza validações estruturais reais (prova de formatação).

## 1. Varredura de Topologias (pergunta central)

### Protocolo usado

O observável `eps_sq_TUT = 0.429` da auditoria foi obtido com o **protocolo Floquet** (amortecimento `gamma` + força periódica), não com o ensemble de Kuramoto puro. Por isso a varredura decisiva reutiliza a semântica exata validada do v2.1 (`avalon_stochastic_tut_v2_1.FloquetHoneycombKuramoto`), trocando apenas o grafo.

Parâmetros de referência: `J=1.0, omega=0.5, amplitude=1.0, gamma=0.1, T=1.0, horizon=10s, dt=0.1s, trajetórias=1500-2000`.

### Resultado empírico (4 seeds)

| seed | chain | honeycomb | complete |
|------|-------|-----------|----------|
| 1    | 0.4711 | 0.3699 | 0.0000 |
| 7    | 0.4487 | 0.3592 | 0.0000 |
| 11   | 0.4349 | 0.4422 | 0.0000 |
| 21   | 0.5019 | 0.3895 | 0.0000 |

**Conclusão honesta (INV-OH):**

- O **honeycomb não é especial**. `chain` produz `eps_sq_TUT` no mesmo intervalo ruidoso (0.43–0.50), equivalente ao honeycomb (0.36–0.44).
- O **grafo completo colapsa o bound para 0** (vacuous), em todas as seeds.
- A proximidade a `1/phi` é um artefato observacional do **regime de baixo grau médio** (topologias esparsas), não um mecanismo de seleção por topologia. **Nenhuma causalidade é inferida** — apenas observação empírica.
- O valor de referência 0.429 da auditoria é dependente de parâmetros (ponto otimizado por DE), não uma constante topológica.

### Validação I6

Ensemble de equilíbrio (J=0, omega=0) deve dar Sigma ~ 0:

```
chain: max|Sigma| = 0.000e+00 (tol 0.001) -> OK
honeycomb: max|Sigma| = 0.000e+00 -> OK
complete: max|Sigma| = 0.000e+00 -> OK
I6 PASSED
```

### Aviso físico

O ensemble de Kuramoto sobredamped puro produz `Sigma` extensivo no tempo → `tanh(Sigma/2) -> 1` → bound vacuous (0) em todas as topologias. Esse regime **não discrimina topologias**; o regime informativo é o Floquet com confinamento, usado acima.

## 2. Correção Itô (importante)

O código proposto (e o rascunho inicial dos kernels v2.3) avaliava `divF` em `theta` **pós-update e pós-wrap**; o comentário dizia pré-update. Correção aplicada: `F = drift(theta)`, `divF = divergence(theta)`, depois `dtheta`, depois update — `F` e `divF` no **mesmo** `theta`, coerente com a semântica Itô validada do v2.1.

```python
# divF pre-update (coerência Itô)
divF = divergence_kuramoto(theta, adj, J)   # mesmo theta de F
dtheta = F * dt + noise
Sigma += (F * dtheta - 0.5 * divF * dt) / T_bath
theta = theta + dtheta
```

## 3. Linter AST Completo (antigo CRÍTICO)

`_check_ast_forbidden_patterns` agora implementa caminhadas AST reais:

| Invariante | Detecção AST | Forma bloqueada |
|-----------|-------------|-----------------|
| INV-OPT-01 | `BinOp(Pow, **2)` com referência a `INV_PHI`/`1/phi`/`0.618` na base | objetivo quadrático que usa a referência phi como alvo |
| INV-TUT-02 | `log/ln(3)` multiplicado por `k_B`, ou `log(3)` atribuído a símbolo de entropia | `S_opt = k_B * np.log(3)` |
| INV-SIGMA-01 | `AugAssign` em `Sigma`/`entropy` com `noise**2` acumulado | `Sigma += noise**2/(2 T dt)` |

Contextos negados/descritivos ignorados (`_in_negated_context`), e o próprio arquivo do linter não se auto-penaliza (`_is_self_match`).

### Verificação

- Positivo: `python avalon\tut_linter_unified.py --py avalon` → **ALL CLAIMS VALIDATED - Publication approved** (exit 0, sem falsos positivos).
- Negativo: fixture com as 3 formas → **4 violações detectadas, exit 1**.

## 4. Núcleo Acelerado v2.3

`avalon_core_v2_3.py`:

- `TopologyFactory` — honeycomb (rótulos inteiros + `honeycomb_radius_for_n`), chain, complete; `get_edges` retorna `(edges, n_graph)` com contagem real de nós.
- Kernels numba: `drift_kuramoto`, `divergence_kuramoto`, `integrate_ensemble` (seeding determinístico dentro do kernel → CRN-safe e reproduzível).
- Fallback numpy automático via stubs `jit`/`prange` (CI sem GPU/deps reproduzível).
- `run_ensemble`, `test_equilibrium_sigma` (I6), `sweep_topologies`, `sweep_floquet_topologies`, relatórios textuais honestos.
- CLI: `--topologies --n_nodes --n_traj --seed --skip_i6`.

## File Manifest

| File | Description | Status |
|------|-------------|--------|
| avalon_core_v2_3.py | Núcleo v2.3: topologias + kernels numba/fallback + varredura Floquet | NEW |
| tut_linter_unified.py | `_check_ast_forbidden_patterns` implementado (era vazio/CRÍTICO) | UPDATED |
| requirements.txt | numpy, networkx, scipy, numba | NEW |
| UPGRADE_v2_3.md | Este manifesto | NEW |

## Validation Checklist (v2.3)

- [x] I6 sanity: Sigma ~ 0 para chain/honeycomb/complete
- [x] Varredura Floquet reproduz o intervalo 0.429 observado e mostra a dependência topológica
- [x] 1/phi NÃO é honeycomb-específico (chain no mesmo intervalo; complete colapsa a 0)
- [x] Correção Itô: divF avaliado no mesmo theta pré-update de F
- [x] Linter AST real: INV-OPT-01, INV-TUT-02, INV-SIGMA-01 detectados estruturalmente
- [x] Linter positivo passa sem falsos positivos
- [x] Fallback numpy reproduz os mesmos resultados (CRN via seed no kernel)
- [x] Prosth: nenhuma claim causal sobre 1/phi em todo o módulo

## Verdict

| Componente | v2.1 | v2.3 |
|-----------|------|------|
| Núcleo Kuramoto | honeycomb-only | topology-general + numba/fallback |
| Linter AST | CRÍTICO (vazio) | Implementado + verificado positivo/negativo |
| Pergunta topológica | não respondida | Respondida: 1/phi é efeito de baixo grau, não honeycomb |
| Honestidade física (INV-*) | OK | OK (Itô corrigido no kernel) |

A física é honesta. A generalidade topológica foi testada, não assumida.

**Generated: 2026-08-16**