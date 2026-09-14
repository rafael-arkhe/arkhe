# Substrato 257 v4 — BSTCM Unified Integration (USI, v53.0)

## Descrição

Integração unificada do BSTCM (Substrato 257) ao tecido cognitivo da Catedral OS
v53.0, conectando-o ao RSI (228), FSO (226), DI-Meter (255), WikiSkill (262) e ao
Veto de Anúbis (184).

## Pipeline de integração

```
SSVEP → Comando → STC → RSI → Híbrido (RF+FSO) → Coerência → Registro (WormGraph)
```

- **SSVEP/CCA** — classificador com filtros Chebyshev de banco e correlação canônica.
- **STC Metasurface** — matrizes espaço-temporais com 11 intervalos de tempo.
- **RSI (228)** — otimização hill-climbing adaptativa das STC matrices.
- **FSO (226)** — ponte híbrida RF-Óptica (4DPPM-DQPSK, QKD-BB84).
- **DI-Meter (255)** — medição da coerência de comunicação (RF + Óptico).
- **AES-GCM** — criptografia autenticada para o estado seguro.

## Requisitos

- Python 3.10+ (testado: 3.14)
- numpy, scipy, cryptography (opcional para testes: pytest)

## Instalação

```bash
pip install -r requirements.txt
```

## Execução (exemplo integrado)

```bash
python substrate_257_v4_unified.py
```

## Testes

```bash
pytest test_substrate_257_v4.py -v
```

## Uso programático

```python
from substrate_257_v4_unified import Substrate257Unified

substrate = Substrate257Unified(di_meter=my_di_meter)
result = substrate.process_and_evolve(raw_signal, validation_score=0.85)
print(result['classification']['command'], result['stc_angle'])
```

## AGI.prolog

O arquivo `agi_core_v53.pl` contém os predicados BSTCM (`assert_bstcm_state/3`,
`bstcm_fitness/1`, `hybrid_coherence/1`) sobre a infraestrutura de métricas
thread-safe (`nb_setval`) e o `think/3` com Veto de Anúbis.

### Nota de honestidade (auditoria Arquiteto-Ω)

O `agi_core_v53.pl` **depende de predicados-base** que não estão definidos neste
arquivo: `is_safe_prompt/1`, `compute_alpha_with_iccid/3`, `epistemic_escalation/2`
e `run_full_tests/0`. No monorepo eles estão definidos em `catedral_os_v98/agi_core.pl`
(base do AGI). O v53 assume esses predicados carregados no mesmo escopo; caso contrário
devem ser fornecidos/reexportados antes de carregar o módulo `cathedral_v53`.

## Correções aplicadas (v53.0)

- **Imports ausentes:** adicionados `numpy`, `logging` e `enum.Enum` (o código
  original fazia referência a eles sem importar, quebrando a importação do módulo).
- **`STCMatrix` indefinido:** a dataclass era referenciada mas nunca definida;
  adicionada a definição.
- **Bug `if validation_score else None`:** conseguiria tratar `0` como falta de score;
  corrigido para `if validation_score is not None`.

## Licença

MIT
