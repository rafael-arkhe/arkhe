# 🌌 ARKHE MANIFOLD 8.1 — INTEGRAÇÃO COMPLETA COM O PIPELINE DE DADOS

Ecossistema integrado: **Verbal Chemistry → Event Processor → ROQUO (HPC) → Glass5D (WORM eterno)**, com validação topológica via **Skyrmions** e orquestração pela **SQC Interface**.

```
┌───────────────────────────────────────────────┐
│                ARKHE VERBAL CHEMISTRY          │
│         PALAVRAS → QUÍMICA → ESTADOS DE SCHMIDT│
└───────────────────────┬───────────────────────┘
                        │
                        ▼
┌───────────────────────────────────────────────┐
│                EVENT PROCESSOR (FastAPI)       │
│           /verbal/analyze · /events · /metrics │
└───────────────────────┬───────────────────────┘
                        │
                        ▼
┌───────────────────────────────────────────────┐
│                ARKHE BRIDGE (8.1)              │
│   SKYRMIONS → ROQUO HPC → GLASS5D (WORM)      │
└───────────────────────────────────────────────┘
```

## Estrutura

```
manifold-8/
├── src/                      # núcleo Python (pacote)
│   ├── arkhe_bridge.py       # ponte ROQUO/Glass5D/SQC/Skyrmions
│   ├── config.py             # settings (env + simulação)
│   ├── logger.py             # logger estruturado
│   ├── metrics.py            # métricas Prometheus-ready
│   └── models.py             # Event/ProcessedEvent/EngramRecord
├── core/verbal_chemistry.py  # biologia quântica das palavras
├── verbal_events_processor.py
├── event-processor/          # serviço FastAPI + Dockerfile
├── simulators/               # roquo-sim, glass5d-sim, sqc-sim
├── dashboard/                # Streamlit + Dockerfile
├── prometheus.yml
├── grafana/dashboards/
├── tests/                    # testes de integração
├── scripts/first_engram.py   # engrama genesis
└── docker-compose.integrated.yml
```

## Execução local (modo simulação, sem container)

```bash
pip install -r requirements.txt

# Testes
python tests/test_arkhe_bridge.py

# Primeiro engrama → genesis_result.json
python scripts/first_engram.py
```

Quando `ROQUO_API_URL`/`GLASS5D_API_URL` não respondem, `SIMULATION_MODE=true` faz o bridge degradar graciosamente com respostas sintéticas — o fluxo completa ponta a ponta.

## Execução com Docker

```bash
docker compose -f docker-compose.integrated.yml up --build
```

| Serviço       | Porta  |
|---------------|--------|
| Event Processor | 8000 |
| ROQUO sim     | 8081  |
| Glass5D sim   | 8082  |
| SQC sim       | 8083  |
| Prometheus    | 9090  |
| Grafana       | 3000  |
| Dashboard     | 8501  |

## Testes de integração

`tests/test_arkhe_bridge.py` cobre: inicialização, health check, validação topológica, submissão de job no ROQUO, escrita/leitura no Glass5D, processamento de engrama, integração com verbal chemistry, rastreamento de status e reset de sessão.