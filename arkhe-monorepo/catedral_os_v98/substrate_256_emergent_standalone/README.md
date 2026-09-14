# Substrato 256 v3.0 — Emergent Dynamics Analyzer Advanced (Standalone)

## Descrição

Pacote autossuficiente para análise de dinâmica emergente multiescala, baseado em
Milinkovic et al. (2026) *"Emergent Multiscale Organisation..."*. Detecta o **sweet
spot** entre emergência e estrutura em séries temporais multivariadas.

## Aprimoramentos v3.0 (vs. v2.0)

1. **Otimização de Grassmann** — gradiente descendente projetado (projeção no espaço
   tangente + retração QR), mais eficiente que Stiefel + retração.
2. **Cálculo de DD** — integração de Simpson com regularização e estabilização numérica.
3. **Threshold de SF adaptativo** — percentil 5 da distribuição de similaridades
   (antes, fixo em 1e-4).
4. **Bootstrap e intervalos de confiança** — inferência estatística para DD e diferenças.
5. **Integração com RSI** — fitness = sweet spot score.
6. **Aceleração GPU** — CuPy opcional (auto-detectado).
7. **Visualização avançada** — matriz de similaridade, distribuição de DD, top fontes.

## Requisitos

- Python 3.10+ (testado: 3.14)
- numpy, scipy, matplotlib (opcional: cupy para GPU)

## Instalação

```bash
pip install -r requirements.txt
```

## Execução (exemplo de uso)

```bash
python substrate_256.py
```

## Testes

```bash
pytest test_substrate_256.py -v
```

## Uso programático

```python
from substrate_256 import Substrate256EmergentDynamicsV3

substrate = Substrate256EmergentDynamicsV3()
results = substrate.analyze(wake_data, scales=[2, 3, 4, 5], n_runs=20, max_iter=300)
report = substrate.get_sweet_spot_report()
fitness = substrate.get_rsi_fitness(wake_data)
comp = substrate.compare_conditions(wake_data, anesth_data, scales=[2, 3, 4])
```

## Notas de honestidade (auditoria Arquiteto-Ω)

- O DD usa o proxy `F* = Σ_k ||M Q_k M^T||_F²` como direção de busca e reavalia a
  integral espectral exata a cada passo — o gradiente é da proxy, não da DD integral.
- CuPy é opcional: sem a biblioteca, o código funciona em CPU silenciosamente.
- A visualização bidimensional (DD vs SF por escala) é placeholder e requer múltiplas
  escalas previamente calculadas.

## Correções de bugs (v3.0 → v3.0.1, auditoria)

- **Inicialização/refinamento QR:** `qr(M.T)` devolve Q `(N, N)` (full) e o código
  retomava `Q.T` sem fatiar as primeiras `n_macro` colunas, produzindo matrizes
  `(N, N)` em vez de `(n_macro, N)` — quebrava a ortonormalidade e o shape do DD
  (`ValueError` de broadcast). Corrigido com `Q[:, :n_macro].T` (também no
  refinamento local).
- **Performance do DD:** o loop Python sobre as 128 frequências era o gargalo;
  vetorizou-se o cálculo em batched `(n_freq, r, r)`, mantendo o mesmo resultado
  numérico com aceleração de ordem de magnitude.


## Licença

MIT
