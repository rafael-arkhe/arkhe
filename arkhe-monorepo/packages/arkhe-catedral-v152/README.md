# arkhe-catedral-v152

Núcleo numérico da **Catedral OS v152.0** — materialização física dos quatro
pilares (sync distribuído, visão esp-config, ponte UART/USB, decaimento
adaptativo em Rust). Complemento do pacote `catedral_os_v152` (Python).

## Invariantes cobertos

| Faixa | Invariante | Onde |
|------|-----------|------|
| I157–I160 | Coerência irreduzível, limiar de Gödel, anti-flatness, cascas diádicas | `src/core/spectral.rs` |
| I161–I167 | Construção iterativa, entropia construtiva | `src/core/entropy.rs` |
| I190–I192 | Tríade | `src/core/entropy.rs`, `formal/lean` |
| I193 | Orçamento de energia (energy ≤ 0 → vacuously Verified) | `src/verification/lean_runtime.rs` |
| I194–I198 | Zeno Veto, ranging, decaimento adaptativo com clamp | `src/hardware/undulator.rs` |
| I199 | Seleção de casca por entropia | `src/core/spectral.rs` |
| I200 | Runtime Lean | `src/verification/lean_runtime.rs` |
| I201 | Sync distribuído (|Φ₁−Φ₂| < ε) | `src/network/ipfs_sync.rs` |

## Valores de coerência

```
Φ_C = clamp(1.5·log₂(ln N), 0, 1)      I158: Φ_crit = 1/log₂ N
Zeno Veto (I194): dt < 2·ranging       Decaimento: Φ(t) = Φ₀·exp(−t/τ), λ(t) clampado
```

## Verificação

```bash
cargo test -p arkhe-catedral-v152
python -m pytest tests -q          # em catedral_os_v152/

# Formalização Lean (I157–I201, zero `sorry`) — requer oleans do mathlib:
lean Invariants.lean                # cwd: packages/arkhe-catedral-v152/formal/lean
# com LEAN_PATH = ../lean-llm-agi/.lake/packages/*/.lake/build/lib/lean
```

Nota de auditoria (honestidade, G11): nenhum `unsafe`; UART e IPFS são
abstraídos por traits (`Z1TTransport`, `IpfsBackend`) com backends
determinísticos em memória — a integração com hardware real exige
implementar esses traits (ex.: `tokio-serial`).