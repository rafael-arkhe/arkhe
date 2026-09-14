# 🔱 SOPHIA V5.0 — Auto-Hospedada

Agentes locais (C++/Python), visão em edge (Raspberry Pi / ESP32-CAM) e
cybersecurity/homelab com hardening e monitorização documentada. **Sem cloud,
sem SaaS** — tudo roda no seu homelab e é auditável.

```
agents/    # orquestrador, patches 19-22, temporal bridge, agente linguístico
  ├─ python/  sophia_local (orquestrador, patches, temporal chain, security)
  └─ cpp/     patches 19-22, orquestrador, hardening, vision pipeline (C++17)
edge_vision/ # firmware ESP32-CAM (captura de glifos em borda)
dashboard/   # Flask + Socket.IO (porta 8080)
security/    # hardening, integridade (SHA-256), regras Wazuh
deploy/      # deploy.sh, systemd units, auditoria
config/      # sophia.yaml (configuração única do sistema)
tests/       # pytest
```

## Fan-out rápido

```bash
cd agents/python
python -m sophia_local.orchestrator --cycles 5    # roda 5 ciclos
python -m sophia_local.security.hardening         # dry-run
python -m sophia_local.security.integrity --build-baseline
python ../dashboard/app.py                        # dashboard em :8080
```

## Testes

```bash
cd agents/python && python -m pytest ../../tests -q
```

## Deploy homelab (Linux)

```bash
sudo ./deploy/deploy.sh --all --environment=homelab --verbose
# inicia: sophia-orchestrator, sophia-dashboard (:8080), sophia-security
```

## Invariantes constitucionais preservados

- **Gap-1** — ψ_C limitado a `0.577350 < Φ_C ≤ 0.999900` (`patches.py`).
- **Loopseal-1/2** — cada ciclo é ancorado na TemporalChain local (append-only,
  SHA3-256, verificação de cadeia em `/api/health`).
- **Ghost-1** — monitor de integridade SHA-256 por baseline.
- **Runtime-3** — dashboard faz healthcheck em `/` e `/api/health`.