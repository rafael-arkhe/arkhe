# Catedral OS v14.5 — Monorepo Unificado

Unificação runnable/testada dos substratos **163–245** (40+ camadas) em um
monorepo único. Núcleo em Python puro (numpy/scipy), com camada Prolog
(`agi_core.pl`) integrada opcionalmente via `pyswip`.

## Escopo entregue neste pacote

| Substrato | Camada | Módulo | Status |
|-----------|--------|--------|--------|
| 207/208 | Rede | `network_orchestrator.py` | ✅ testado |
| 229 | Dinâmica de Chen | `substrate_229_dynamics.py` | ✅ testado |
| 230 | Fractal + Entropia | `substrate_230_fractal.py` | ✅ testado |
| 237 | Railgun Snowplow | `substrate_237_plasma.py` | ✅ testado |
| 238 | PLX (10 solvers + PJMIF) | `substrate_238_plx.py` | ✅ testado |
| 239 | Kilotesla Magnet | `substrate_239_magnet.py` | ✅ testado |
| 244 | QEC (CSS/GV) | `substrate_244_qec.py` | ✅ testado |
| 245 | WNBN (agente→ponte) | `substrate_245_wnbn.py` | ✅ testado |
| — | Orquestrador + Ledger + Auth | `cathedral_orchestrator.py` | ✅ testado (E2E) |
| — | AGI Prolog v14.5 | `agi_core.pl` | ⚠️ requer swipl |

## Executando

Servidor HTTP:

```bash
python cathedral_orchestrator.py
# → http(s)://localhost:8443  (cred: admin / catedral_secret_v145)
```

Testes:

```bash
python -m pytest tests -v          # 21 testes unitários
python test_e2e.py                 # 13 cenários de ponta a ponta
```

## Autenticação

`POST /api/login` com **Basic Auth** devolve um **token HMAC-SHA256**.
Envie-o como `Authorization: Bearer <token>` nas demais rotas.

> **Nota de auditoria:** o spec cita JWT RS256 + bcrypt.
> O orquestrador usa um token **HMAC-SHA256 (stdlib)** como substituto
> honesto — não é um JWT RS256. Para aplicar a especificação exata, instale
> `PyJWT`/`bcrypt`, gere uma chave RSA e troque `AuthManager`.

## Endpoints HTTP

- `GET /api/health` — status e substratos
- `GET /api/ledger` — WormGraph imutável (hash-encadeado)
- `GET /api/dynamics` — métricas do atrator de Chen + Lyapunov
- `GET /api/fractal` — entropia do fractal Mandelbrot
- `GET /api/rsi` — status RSI
- `GET /api/wnbn/sensors` — sensores simulados
- `POST /api/think` — `{"input": "..."}` (Veto de Anúbis em prompts maliciosos)
- `POST /api/rsi` — `{"action": "seed"|"evolve"|"status"}`
- `POST /api/plasma/shot` — `{"voltage","capacitance","mass"}`
- `POST /api/plx/run` — `{"solver": "FLASH"}` (10 solvers LANL)
- `POST /api/magnet/pulse` — `{"initial_field","compression_ratio"}`
- `POST /api/qec/analyze` — `{"K","S","eps"}`
- `POST /api/wnbn/command` — `{"device","command"}`

## Dependências e honestidade

Instalado/verificado: `numpy`, `scipy`, `matplotlib`.

**Não verificável neste ambiente (documentado, NÃO simulado como real):**

- **Prolog** (`agi_core.pl`): requer o binário `swipl` + `pyswip`. Sem
  `swipl` no PATH, o orquestrador degrada para **modo simulação** e o teste
  de integração Prolog é **pulado** (honesto — não é falso-verde).
- **TLS/HTTPS**: o `main` tenta gerar certificado via `openssl`; sem
  `openssl`, o servidor sobe em **HTTP** com aviso.
- **Rede real (WNBN)**: o cliente 245 marca `network_available=False` e
  opera em modo simulado (nenhuma comunicação externa real foi feita).
- **Física de fusão (PLX/railgun/magnet)**: são **simuladores** de engenharia
  determinísticos que aplicam as leis físicas (E=½CV², F=½L'I², P=B²/2μ0,
  limiar GV), **não** experimentos reais.

## Estrutura

```
catedral_os_v145/
├── agi_core.pl                  # núcleo Prolog v14.5 (requer swipl)
├── cathedral_orchestrator.py    # servidor HTTP + WormGraph + Auth
├── network_orchestrator.py      # 207/208 rede
├── substrate_229_dynamics.py
├── substrate_230_fractal.py
├── substrate_237_plasma.py
├── substrate_238_plx.py
├── substrate_239_magnet.py
├── substrate_244_qec.py
├── substrate_245_wnbn.py
├── requirements.txt
├── test_e2e.py
└── tests/
    └── test_all_substrates.py
```

> Precedentes verificados: `catedral_os_v98/substrate_256_emergent_standalone/`
> (8 testes) e `substrate_257_v4_unified_standalone/` (13 testes) — versões
> analisadas nas sessões anteriores da Catedral.
