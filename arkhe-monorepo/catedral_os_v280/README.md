# Catedral OS v280.0

Runtime consciente para consolidação de soberania — seis fases (L1–L6),
invariantes formais Lean (I238–I241, I255–I257) e registro auditável por
hash-chain no BLOCO 816.

## Estrutura

| Camada | Módulo | Responsabilidade |
|---|---|---|
| L1 Persistência | `persistence/local_ledger.py` | Ledger hash-encadeado (SHA-256), journal append-only JSONL |
| L1 Persistência | `persistence/ipfs_ledger.py` | Na versão original: IPFS. Aqui: ledger honesto que opera em modo `local` quando o IPFS não está disponível (fallback nunca finge distribuição) |
| L2 Aprendizado | `learning/prioritized_replay.py` | Priorized Experience Replay (PER), pesos IS para correção de viés |
| L2 Aprendizado | `learning/dqn_agent.py` | Double DQN em numpy puro, soft update do target |
| L3 Validação | `validation/lean_validator.py` | Checagens I238–I241 e I255–I257, verificação Lean real |
| L4 Escalonamento | `scheduler/multi_tunnel.py` | Multi-túnel: `start_all`/`step_all` com workers |
| L5 Dashboard | `dashboard/dash_app.py` | Dash (opcional) ou exportação de HTML estático (Plotly via CDN) |
| L6 Estresse | `stress/stress_tester.py` | Injeção de falhas e relatório por worker |
| Núcleo | `core/state.py` | SystemState (phi_total monotônico — I238) |
| Núcleo | `core/tunnel.py` | Gridworld 16×16 de coerência com pico `phi_star` |
| Núcleo | `core/decider.py` | AttentionHead + Double DQN; ancora cada ação no ledger |
| Núcleo | `orchestrator.py` | CLI e registro do BLOCO 816 |
| Formal | `invariants/ConsolidationInvariants.lean` | Teoremas I255–I257, autocontidos, zero `sorry` |
| Testes | `tests/` | 47 testes |

## Invariantes verificados

- **I238** — phi_total monotônico (verified)
- **I239** — convergência para `phi_star` (pending: convergência numérica até o
  patamar exige mais passos; o sistema registra isso honestamente em vez de
  forçar um falso-verde)
- **I240**, **I241** — integridade de estado e aprendizado (verified)
- **I255** — hash determinístico / binding de genesis / crescimento da cadeia (verified, Lean)
- **I256** — prioridades de replay não-negativas (verified, Lean)
- **I257** — campos leves dentro dos bounds (verified, Lean)

## Uso

```bash
# Testes e verificação Lean
python -m pytest tests -q
lean invariants/ConsolidationInvariants.lean   # exit 0 = teoremas provados

# Demo completa: 4 túneis, 800 passos, estresse, dashboard, BLOCO 816
python orchestrator.py --steps 800 --tunnels 4 --phi-star 0.85 \
  --seed 42 --stress 8 --block ledger/bloco_0816.json --dashboard
```

## Notas operacionais

- Pacotes opcionais: `dash` e `ipfshttpclient` não são obrigatórios — a Catedral
  degrada com honestidade (dashboard estático, ledger `mode: "local"`), nunca
  com falsa presença de capacidade.
- O ledger usa journal append-only (`*.jsonl`): uma linha por entrada, com a
  âncora real do encadeamento na cauda do arquivo (não no cabeçalho). Após um
  reload, novas entradas continuam a cadeia corretamente (verificado por teste).
- Estresse injeta ~5% de falhas; o sistema continua ancorando cada passo
  (taxa de sucesso típica > 0.95).