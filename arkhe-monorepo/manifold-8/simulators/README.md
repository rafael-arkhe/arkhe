# Simuladores do ecossistema Arkhe 8.1 (ROQUO, Glass5D, SQC)

Servidores HTTP mínimos que expõem a mesma API externa dos serviços reais,
permitindo desenvolvimento e testes locais sem infraestrutura.

- `roquo-sim`: POST /jobs, GET /jobs/{id}, GET /health
- `glass5d-sim`: POST /write, GET /read/{record_id}, GET /datasets, GET /health
- `sqc-sim`: GET /health (orquestração mapeada para os backends)

Cada servidor escuta na porta 8080 dentro do container.