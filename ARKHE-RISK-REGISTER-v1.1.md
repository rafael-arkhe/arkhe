# ARKHE-RISK-REGISTER-v1.1

Anexo H atualizado — ARKHE OS v1.2 + Camada de Orquestração Externa (ADR-009)
Selo: ARKHE-RISK-REGISTER-v1.1-2026-08-11

| ID | Risco | Probabilidade | Impacto | Mitigação | Owner |
|---|---|---|---|---|---|
| R1 | Sandbox não fica pronto na Semana 18 | Alta | Alto | Design paralelo na Semana 4; prototipagem precoce | Domínio C |
| R2 | Verilator gate não é alcançável | Média | Alto | Buffer de 2 semanas (Semanas 15–16); golden model validado previamente | Domínio B |
| R3 | PolyglotVerifier não atinge 75/100 | Alta | Médio | Postergado para pós-Fase 3; não bloqueia integração | Domínio C |
| R4 | Qiskit API breaking change | Média | Médio | BOM pinado (qiskit==1.2.0, qiskit-aer==0.14.2) | Domínio A |
| R5 | VE2602 timing closure a 100 MHz | Média | Alto | Margem de DSP reservada (100 DSPs); fallback M=24,N=48 | Domínio B |
| R6 | Domínio A (Python) quebra workspace Rust | Alta | Médio | CI separado (rust-ci + python-ci); justfile unificado | DevOps |
| R7 | Readout error do IBEX Q1 > 2.0% | Baixa | Médio | Usar hipótese conservadora; validar em QPU real na Fase 5 | Domínio A |
| R8 | Detector de torção excede recursos VE2602 | Média | Alto | Reserva de 100 DSPs + 2Mb BRAM; migração para VE2802 como plano B | Domínio B |
| R9 | Adoção de workflow engine como runtime | Alta (se não documentado) | Alto | ADR-009 obrigatória; code review rejeita qualquer PR que injete Windmill/Activepieces no runtime dos domínios | Arquiteto |
| R10 | Latência p95 < 100ms violada por overhead de fila PostgreSQL | Alta | Médio | Medir no Integration Spike (Semana 18); fallback: bypass Windmill para EvidencePacket críticos, usar gRPC direto | DevOps |
| R11 | Windmill AGPLv3 + ARKHE MIT/Apache = incompatibilidade de licença em linking | Baixa | Médio | Windmill executa como processo separado (webhook/gRPC), nunca como crate linkada; ADR-009 documenta boundary; aprovação Legal obrigatória antes de deploy | Legal/DevOps |
| **R12** | **Privilege escalation via docker.sock no worker Windmill** | **Média** | **Alto** | **docker.sock removido do compose v1.1; se Docker-in-Docker for necessário no futuro, usar docker-socket-proxy (Tecnativa) com filtros de API read-only ou rootless Docker** | **DevOps** |
| **R13** | **Falha de reprodutibilidade por imagem Docker não pinada** | **Alta** | **Médio** | **Todas as imagens pinadas por digest SHA256 no docker-compose v1.1; BOM Docker versionado no repo; `cargo audit` análogo aplicado a imagens base via Trivy/Grype** | **DevOps** |

## Notas de Revisão v1.0 → v1.1

- **R11:** Probabilidade reduzida de "Média" para "Baixa" após adoção da cláusula de processo separado no ADR-009 v1.1 e remoção de qualquer possibilidade de linking.
- **R12:** Adicionado. O docker-compose v1.0 montava `/var/run/docker.sock` no worker Windmill, criando vetor de privilege escalation. Removido na v1.1.
- **R13:** Adicionado. O docker-compose v1.0 usava tag `:main` mutável, violando o princípio de Reprodutibilidade do ARKHE v1.2. Corrigido na v1.1 com pinagem por digest SHA256.
