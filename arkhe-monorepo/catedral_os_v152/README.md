# Catedral OS v152.0 — Materialização Física

Pacote **Python** do v152.0 (a quarta onda da materialização). O núcleo Rust
vive em `packages/arkhe-catedral-v152`; este diretório contém o integrador da
Tríade + Undulator e os módulos de apoio.

## Escopo

| Módulo | Camada | Invariantes | Status |
|--------|--------|-------------|--------|
| `python/core/spectral.py` | Estrutura espectral | I157–I160 | ✅ testado |
| `python/core/entropy.py` | Entropia construtiva | I167 | ✅ testado |
| `python/core/temporal_chain.py` | Cadeia temporal | I190–I192, I162 | ✅ testado |
| `python/hardware/undulator.py` | Nó Undulator | I194–I198 | ✅ testado |
| `python/hardware/z1t_bridge.py` | Bridge Z1T (G1/G8/G11) | framing + fallback | ✅ testado |
| `python/hardware/tcamera_pipeline.py` | Percepção (T-Camera) | — | ✅ testado (sim) |
| `python/adaptive/shell_selector.py` | Cascas diádicas adaptativas | I199 | ✅ testado |
| `python/adaptive/governance_adaptive.py` | Governança adaptativa | — | ✅ testado |
| `python/verification/lean_runtime.py` | Verificação Lean embarcada | I200 | ✅ testado |
| `python/dashboard/visualizer.py` | Dashboard ASCII da malha | G14 | ✅ testado |
| `python/vision/pipeline.py` | Pipeline de visão (referência desktop) | I157–I160 | ✅ testado |
| `python/triad_integrator.py` | Tríade + Undulator (E2E) | I190–I198 | ✅ testado |

## Executando

```bash
python -m pytest tests -v
```

## Honestidade (auditoria)

- **Z1T físico**: `pyserial` é opcional. Sem porta/hardware, a bridge degrada
  para **simulação determinística** (`simulate_z1t_response`), marcada como
  `is_simulating=True`. Nenhuma comunicação serial real é fingida.
- **T-Camera S3**: `TCameraPipeline` simula espectros com seed fixo; o
  pipeline embarcado real (MicroPython/ESP32) é o `vision/pipeline.py`, cujo
  modelo de referência é verificado no desktop.
- **IPFS / Undulator físico**: o comportamento distribuído e o decaimento em
  hardware são verificados no crate Rust (`packages/arkhe-catedral-v152`).
- **Visualização**: ASCII puro, sem dependências.

## Formais

As provas Lean 4 dos invariantes I157–I201 vivem em
`packages/arkhe-catedral-v152/formal/lean/Invariants.lean`.