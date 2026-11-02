# Arkhe-Test: Pipeline Contínuo de Invariantes e Qualidade

> **Princípio Central:** *Nenhuma funcionalidade entra em produção sem um teste que possa reprová-la.*

O `arkhe-test` transforma o whitepaper da Arkhe em evidência auditável, aplicando disciplina *fail-closed* em todo o ciclo de vida de desenvolvimento, rollout e rollback.

---

## 🏛️ Invariantes do Pipeline (I-T01 – I-T08)

1. **I-T01 — Teste antes de merge:** Nenhum PR entra na branch principal sem aprovação em todos os testes de invariantes.
2. **I-T02 — Fail-closed:** Qualquer falha em teste de segurança bloqueia merge ou deploy imediatamente.
3. **I-T03 — Rastreabilidade:** Cada teste referencia os invariantes constitucionais (I-01 a I-10) e a fase do roadmap.
4. **I-T04 — Reprodutibilidade:** Builds e testes herméticos geram resultados bit-a-bit consistentes.
5. **I-T05 — Rollback seguro:** Rollbacks revertem código e configuração, mas **nunca** reescrevem ou truncam MMR, recibos AFiR ou checkpoints (`rollback => evidência existente intacta`).
6. **I-T06 — Feature flags restritas:** Flags podem habilitar comportamentos opcionais (ex. transportes experimentais), mas jamais enfraquecem checagens de segurança (`flag ∉ {signature check, hash check, MMR append-only, role separation}`).
7. **I-T07 — Evidência auditável:** Cada execução gera um relatório estruturado e content-addressed (`reports/security/`).
8. **I-T08 — Teste adversarial contínuo:** A suíte adversarial ataca ativamente o sistema contra replays, adulteração de bytes, falsificação de fraud proofs e colisão de papéis.

---

## 🎯 Matriz de Cobertura dos Invariantes Prioritários (I-01 – I-10)

| Invariante | Descrição | Teste Obrigatório | Critério de Reprovação |
|---|---|---|---|
| **I-01** | Chunk íntegro | Alterar 1 byte e tentar aceitar | Chunk corrompido é aceito |
| **I-02** | Manifesto verificável | Trocar CID ou remover chunk | Manifesto inválido é aceito |
| **I-03** | Recibo AFiR | Assinatura errada ou reuso de nonce | Recibo inválido ou replay aceito |
| **I-04** | MMR append-only | Remover/reordenar histórico de folhas | Reordenação/truncamento aceito |
| **I-05** | Auditoria offline | Corromper bundle .arkbundle e verificar | Verificador offline aceita |
| **I-06** | Vínculo ao dispositivo | Checkpoint com atestação TPM/TEE não autorizada | Checkpoint sem atestação aceito |
| **I-07** | Governança humana | Proposta autônoma alterando constituição sem chancela | Modificação autônoma aplicada |
| **I-08** | Reward por prova | Liberação de recompensa sem recibo ancorado | Pagamento liberado sem prova |
| **I-09** | Evidência chain-agnostic | Verificação offline de bundle sem RPC/rede | Falha indevida offline |
| **I-10** | Papéis disjuntos | Worker atuando como validador no mesmo epoch | Colisão de papéis aceita |

---

## 📂 Estrutura do Repositório

```text
arkhe-test/
├── vectors/
│   ├── afir/           # Vetores válidos e com assinaturas inválidas
│   ├── mmr/            # Vetores de picos e raízes MMR
│   ├── checkpoint/     # Vetores de checkpoint e atestação TPM
│   └── fraud/          # Vetores de provas de fraude
├── suites/
│   ├── unit/           # Serialização canônica, hashing, schema AFiR, MMR e flags
│   ├── property/       # Testes de propriedade
│   ├── integration/    # Testes de integração nó, chunk exchange e rollback
│   ├── adversarial/    # Testes adversariais I-01 até I-10
│   ├── network/        # Resiliência de rede e detecção de pacotes corrompidos
│   ├── mobile/         # Atestação de elemento seguro e limites de hardware
│   └── wasm/           # Sandbox WASM e contenção de memória
├── harness/
│   ├── chaos/          # Injeção de bit-flips e latência
│   ├── fuzz/           # Fuzzing de payload e schemas
│   ├── replay/         # Testes de replay de não-repetição
│   └── rollback/       # Validação de preservação de evidência em rollback
├── reports/
│   ├── coverage/
│   ├── security/       # Relatórios content-addressed com hash BLAKE3/SHA-256
│   └── testnet/
├── policies/
│   ├── flags.toml      # Invariantes que nenhuma feature flag pode desligar
│   ├── release.toml    # Critérios de build hermética e auto-halt
│   └── rollback.toml   # Regras estritas de rollback seguro
└── run_pipeline.py     # Orquestrador CLI do pipeline em 3 estágios
```

---

## 🚀 Como Executar

### Executar Pipeline Completo (Estágios 1, 2 e 3)
```bash
python3 arkhe-test/run_pipeline.py --all
```

### Executar Estágio Específico
```bash
# Estágio 1: Feedback rápido (< 5 min)
python3 arkhe-test/run_pipeline.py --stage 1

# Estágio 2: Integração (< 20 min)
python3 arkhe-test/run_pipeline.py --stage 2

# Estágio 3: Adversarial / Red Team (I-01..I-10)
python3 arkhe-test/run_pipeline.py --stage 3
```
