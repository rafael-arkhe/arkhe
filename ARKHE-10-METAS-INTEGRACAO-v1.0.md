# ARKHE — Análise Integrada das 10 Metas Estratégicas

**Documento:** `ARKHE-10-METAS-INTEGRACAO-v1.0`  
**Data:** 2026-08-13  
**Selo:** `ARKHE-10-METAS-INTEGRACAO-v1.0-2026-08-13`  
**Status:** 🟢 **ANÁLISE CRÍTICA — PRONTO PARA REVISÃO ESTRATÉGICA**

---

## 1. Sumário Executivo

As 10 metas apresentadas formam um **sistema interdependente** que conecta três vertentes do ecossistema Arkhe:

| Vertente | Metas | Documento Base |
|----------|-------|----------------|
| **Soberania Digital / SISBIN** | P1 (PQC), P3 (Governança), P4 (Data Lake), P5 (Interoperabilidade), P6 (Federados), P9 (Plataforma Unificada) | SISBIN-TRIADE-TECNICA-v1.0 |
| **Produção Nacional de Hardware** | P2 (Requisitos DL), P7 (Certificação HW), P8 (CEITEC→Arkhe), P10 (Exportação AL) | ARKHE-3DP-LITHOGRAPHY-MLOPS-NATIONAL-PLAN-v3.1 |
| **Convergência Arkhe-Cathedral** | P1, P3, P5, P9 | ADR-009-v1.1, ARKHE-RISK-REGISTER-v1.1 |

**Conclusão central:** Nenhuma meta pode ser executada isoladamente. A ordem de implementação é determinada por **dependências técnicas e institucionais**, não por desejo político. A fundação criptográfica (P1) e o schema de dados (P2) são pré-requisitos absolutos para todos os demais itens.

---

## 2. Análise Individual por Meta

### P1 — Definir PQC como padrão para SISBIN, PNH e Arkhe
**Score de prioridade: 20.5/20.5 (Rank #1)**

| Dimensão | Avaliação |
|----------|-----------|
| **Impacto** | Altíssimo — fundação de confiança para toda a cadeia |
| **Esforço** | Médio — libharpia e PCPv2 já existem (CEPESC/ABIN) |
| **Urgência** | Imediata — janela de 48 dias para Finep; ameaça quântica em horizonte 5-10 anos |

**Base factual:**
- CEPESC/ABIN já opera PCPv2 (TRNG quântico) e desenvolve libharpia (ML-KEM + ML-DSA híbrido)
- SISBIN-TRIADE v1.0 propõe: TLS 1.3 + mTLS com X25519 + ML-KEM-768; assinatura ML-DSA-65; hash BLAKE3
- **Gap:** libharpia ainda não passou por verificação formal (Kani/Coq) em escala; PCPv2 é hardware limitado

**Riscos críticos:**
- **R-CRYPTO:** ML-DSA-65 ainda em draft FIPS 204; mudanças na especificação podem quebrar compatibilidade
- **R-SUPPLY:** HSM Thales/Luna 7 é importado; sanções ou embargo afetam reposição
- **R-INSTITUCIONAL:** SISBIN, PNH (Plano Nacional de Hardware) e Arkhe têm governanças distintas; padronização exige decreto interministerial

**Próximo passo concreto:**
1. Publicar **Especificação Técnica PQC-Brasil v0.1** (ML-KEM-768 + ML-DSA-65 + BLAKE3) como RFC aberto
2. Integrar libharpia ao CI/CD do Arkhe (cargo test + Kani proof harness)
3. Submeter à Finep (Rodada 2) projeto de hardening criptográfico como componente do BrazilianCAL

---

### P2 — Mapear requisitos do Data Lake Nacional
**Score: 17.5/20.5 (Rank #4)**

| Dimensão | Avaliação |
|----------|-----------|
| **Impacto** | Alto — sem schema, não há integração |
| **Esforço** | Médio — MQTT + JSON-LD são padrões maduros |
| **Urgência** | Imediata — checklist Finep item #9 (pendente) |

**Base factual:**
- ARKHE-3DP-LITHOGRAPHY-MLOPS-NATIONAL-PLAN-v3.1 lista componentes do Data Lake: padrão de dados (temperatura, dose, posicionamento, tempo), Feature Store, API de dados
- SISBIN-TRIADE propõe: Kafka (KRaft) + OpenLineage + MinIO + PostgreSQL 16 TDE
- **Gap:** Não há schema detalhado (campos, frequência, unidades, classificação SISBIN) documentado

**Dependências:**
- Requer definição de classificação de dados (sigiloso/reservado/secreto/ultrassecreto) — vem do SISBIN
- Requer escolha de serialização (Avro? Parquet? Protobuf?) — impacta performance do Kafka

**Próximo passo concreto:**
1. Workshop técnico (2 dias) com CNPEM/LNNano, CTI, ABIN/CEPESC, Dataprev
2. Entrega: **Schema v0.1** em JSON-LD com contexto semântico para litografia 3D + sensores industriais
3. Publicar como padrão aberto no repositório ARKHE (licença CC-BY-SA)

---

### P3 — Unificar governança de dados (Evidence Bus + TOONs + sensores)
**Score: 19.0/20.5 (Rank #2)**

| Dimensão | Avaliação |
|----------|-----------|
| **Impacto** | Altíssimo — accountability perante CCAI; rastreabilidade de decisões |
| **Esforço** | Alto — integra 3 sistemas com culturas organizacionais distintas |
| **Urgência** | Curto prazo — depende de P1 e P2 |

**Base factual:**
- Evidence Bus já definido no SISBIN: entries com `prev_hash, content_hash, signer_did, timestamp, classification, signature`
- TOONs (Tokens of Origin and Novelty) usados no ciclo de fabricação Arkhe (Seção 9.2 do v3.1)
- Sensores: pipeline MQTT + Telegraf proposto; SAP PM (IW21/IW31) para manutenção preditiva

**Convergência técnica proposta:**
```
Sensor IoT (OpenCAL/OS1) → MQTT Broker → Kafka Topic → Evidence Bus Entry
                                    ↓
                              OpenLineage Run → Atlas Entity → TOON de Fabricação
                                    ↓
                              OPA Decision Log → AccessEvidence → CCAI Auditor
```

**Riscos:**
- **R-PERFORMANCE:** Evidence Bus append-only em 250+ órgãos gera volume massivo; requer arquitetura de shard por câmara temática
- **R-ADOPTION:** Órgãos federados (UFs, municípios) não têm maturidade para DID + capability tokens

**Próximo passo concreto:**
1. Implementar **Evidence Bus Node** em Rust (crate `arkhe-evidence-bus`) com suporte a ML-DSA-65
2. Piloto em 2 órgãos: ABIN (zona crítica) + CNPEM (zona pública/restrita)
3. Métrica de sucesso: latência de append < 50ms; throughput > 10k entries/segundo

---

### P4 — Implementar Data Lake Nacional compartilhado
**Score: 19.0/20.5 (Rank #2)**

| Dimensão | Avaliação |
|----------|-----------|
| **Impacto** | Altíssimo — habilita MLOps industrial em escala |
| **Esforço** | Alto — infraestrutura multi-cloud soberana (AMSR) |
| **Urgência** | Curto prazo — depende de P2 e P3 |

**Base factual:**
- AMSR (Arquitetura Multi-Cloud Soberano Resiliente) proposta no SISBIN-TRIADE: 4 regiões, 3 macrorregiões, operadores diversificados (Serpro, Dataprev, RNP, UFC/CISSA, LNCC)
- Investimento estimado: R$ 900M complementares aos R$ 1B já investidos
- **Gap:** RNP e cooperativas estaduais ainda não são operadores de produção; Dataprev NE em expansão

**Arquitetura de referência:**
| Camada | Tecnologia | Operador |
|--------|-----------|----------|
| Ingestão | Kafka (KRaft) + MQTT | RNP (Norte) + Dataprev (NE) |
| Armazenamento | MinIO (S3) + Ceph/Rook | Serpro (CO) + LNCC (SE) |
| Processamento | Spark/Flink + Kubeflow | Dataprev + RNP |
| Governança | Apache Atlas + OPA | ABIN/CEPESC |
| Criptografia | libharpia + HSM Thales | CEPESC (todas as regiões) |

**Próximo passo concreto:**
1. **Fase piloto (6 meses):** 2 regiões (Brasília + Fortaleza) com replicação síncrona
2. Dataset inicial: dados anonimizados dos protótipos OpenCAL (3 laboratórios)
3. Gate de go/no-go: RPO < 1min para dados restritos; custo/GB < R$ 0,50

---

### P5 — Desenvolver padrões de interoperabilidade entre os sistemas
**Score: 14.0/20.5 (Rank #6)**

| Dimensão | Avaliação |
|----------|-----------|
| **Impacto** | Alto — sem interoperabilidade, cada órgão é uma ilha |
| **Esforço** | Alto — negociação política + técnica |
| **Urgência** | Médio prazo — depende de P3 e P4 |

**Base factual:**
- SISBIN já propõe: API Gateway (Kong/Envoy) + OpenAPI 3.0 + DID + capability tokens
- Arkhe usa: gRPC + AXI-Stream (hardware) + webhook (Windmill externo)
- PNH usa: SAP PM/MM (IW21, IW31, ME51N) para ERP industrial

**Padrões propostos:**
1. **ARKHE-IF-001:** Interface de dados de processo (JSON-LD + MQTT) — para sensores e MLOps
2. **ARKHE-IF-002:** Protocolo de evidências criptográficas (gRPC + Evidence Bus) — para governança
3. **ARKHE-IF-003:** Gateway federado (Kong + OPA + mTLS) — para órgãos SISBIN
4. **ARKHE-IF-004:** ERP industrial (SAP IDoc / OData) — para cadeia de suprimentos CEITEC

**Próximo passo concreto:**
1. Constituir **Comitê Técnico de Interoperabilidade** (CTI-Arkhe) com representantes do MCTI, MDIC, ABIN, Serpro, Dataprev
2. Publicar 4 RFCs iniciais em 90 dias
3. Implementar reference implementation em Rust (crate `arkhe-interop`)

---

### P6 — Expandir para órgãos federados e estaduais
**Score: 13.5/20.5 (Rank #8)**

| Dimensão | Avaliação |
|----------|-----------|
| **Impacto** | Alto — escala e legitimidade democrática |
| **Esforço** | Médio — requer capacitação, não apenas tecnologia |
| **Urgência** | Médio prazo — depende de P4 e P5 |

**Base factual:**
- SISBIN-TRIADE identifica risco T3: "APT infiltra via órgão federado com baixa maturidade de segurança"
- AMSR propõe cooperativas estaduais (modelo OCEPAR) como operadores na Região Sul
- **Gap:** 90% dos municípios brasileiros não têm CIO; infraestrutura de TI precária

**Estratégia de expansão:**
| Nível | Entrada | Capacitação | Autonomia de Dados |
|-------|---------|-------------|-------------------|
| **Municípios < 50k hab.** | API Gateway apenas (metadados) | Programa SEBRAE-TI básico | Somente leitura de alertas |
| **Municípios 50k–500k** | Kafka Consumer (dados anonimizados) | Residência em MLOps (3 meses) | Ingestão de dados locais |
| **Estados / UFs** | Full node do Evidence Bus | Certificação ABIN em governança de dados | Produtor e consumidor |

**Próximo passo concreto:**
1. Parceria com **FNP (Frente Nacional de Prefeitos)** e **CONSEGI** para mapeamento de maturidade
2. Programa piloto em 5 estados: SP, MG, CE, RS, GO (diversidade geográfica e econômica)
3. Métrica: 50 municípios conectados ao Data Lake em 12 meses

---

### P7 — Certificar hardware nacional (CEITEC, OpenCAL)
**Score: 14.0/20.5 (Rank #6)**

| Dimensão | Avaliação |
|----------|-----------|
| **Impacto** | Alto — habilita cadeia de suprimentos nacional |
| **Esforço** | Alto — certificação é processo de 12-24 meses |
| **Urgência** | Médio prazo — depende de P1 (PQC para assinatura de firmware) |

**Base factual:**
- CEITEC-SiC: R$ 220M aportados; produção prevista 2S/2026; requer certificação AEC-Q101 para automotivo
- OpenCAL: licença UC Regents (uso comercial requer acordo); TRL 5-6; BOM ~US$ 200-1.500
- **Gap:** Não há laboratório de certificação de semicondutores de potência no Brasil; depende de testes no exterior (US$ 500k-1M)

**Roteiro de certificação:**
| Hardware | Certificação | Responsável | Prazo | Custo Estimado |
|----------|-------------|-------------|-------|----------------|
| CEITEC-SiC (MOSFET, SBD) | AEC-Q101 | CEITEC + parceiro automotivo | 18 meses | R$ 2-3M |
| OpenCAL (RPi + projetor) | INMETRO / ABNT NBR | LNNano + CTI | 6 meses | R$ 200k |
| OS1-BR (light engine) | CE + FCC (driver novo) | CTI Renato Archer | 12 meses | R$ 500k |
| Módulo PQC (libharpia + FPGA) | FIPS 140-3 (Level 2) | CEPESC/ABIN | 24 meses | R$ 5M |

**Próximo passo concreto:**
1. Negociar acordo de licenciamento comercial OpenCAL com UC Berkeley (via FAPESP ou Finep)
2. Estabelecer parceria com **Stellantis / BYD / VW** para co-certificação AEC-Q101 do SiC
3. Criar **Laboratório de Certificação de Hardware Nacional** no LNNano (campanha Finep Rodada 2)

---

### P8 — Integrar CEITEC (SiC) na cadeia de suprimentos da Arkhe
**Score: 8.5/20.5 (Rank #9)**

| Dimensão | Avaliação |
|----------|-----------|
| **Impacto** | Médio — nicho estratégico, mas volume limitado |
| **Esforço** | Médio — integração ERP + qualificação de fornecedor |
| **Urgência** | Longo prazo — depende de P7 |

**Base factual:**
- SAP MM (ME51N) pode gerenciar fornecedores nacionais de óxidos de terras raras (XK01)
- CEITEC-SiC é fornecedor potencial de: diodos, MOSFETs, módulos de potência para VE e fotovoltaica
- **Gap:** CEITEC não tem capacidade de packaging e teste final; produto é wafer ou die bruto

**Integração proposta:**
```
CEITEC (wafer SiC) → Packaging Nacional (a desenvolver) → Módulo de Potência
        ↓
   SAP MM (XK01) → Arkhe-flock (TOON de origem) → Evidence Bus (assinatura PQC)
        ↓
   Montadoras (Stellantis/BYD) ← Ordem de Serviço (IW31) ← Manutenção Preditiva
```

**Próximo passo concreto:**
1. Mapear capacidade real de produção da CEITEC (wafers/mês, yield, defeitos)
2. Contratar empresa de packaging (potencial: **HT Micron** em São Leopoldo, ou parceria com **Infineon** para transferência de tecnologia)
3. Integrar ao BOM do OpenCAL: RPi 5 + CEITEC-SiC (regulador de potência) + projetor DLP

---

### P9 — Lançar Plataforma Unificada de Soberania Digital
**Score: 16.0/20.5 (Rank #5)**

| Dimensão | Avaliação |
|----------|-----------|
| **Impacto** | Altíssimo — visão unificadora de 10+ anos |
| **Esforço** | Altíssimo — requer convergência institucional |
| **Urgência** | Longo prazo — é a síntese, não o início |

**Base factual:**
- SISBIN-TRIADE propõe AMSR: 4 regiões, 6 operadores, crypto-agnostic zones
- ARKHE-3DP-LITHOGRAPHY v3.1 propõe: máquina nacional com MLOps embarcado (2033)
- Palantir (no v2.0 do Hardware Nacional) oferece camada enterprise, mas com risco de lock-in

**Arquitetura da Plataforma Unificada:**
```
┌─────────────────────────────────────────────────────────────────────────────┐
│           PLATAFORMA UNIFICADA DE SOBERANIA DIGITAL (PUSD)               │
├─────────────────────────────────────────────────────────────────────────────┤
│  CAMADA 4: ORQUESTRAÇÃO ENTERPRISE (opcional)                             │
│  Palantir Foundry / Arkhe-flock / Windmill externo                          │
├─────────────────────────────────────────────────────────────────────────────┤
│  CAMADA 3: MLOps + IA (Arkhe-inference + Kubeflow + MLflow)               │
│  Otimização de processo, CQ preditivo, manutenção preditiva               │
├─────────────────────────────────────────────────────────────────────────────┤
│  CAMADA 2: FABRICAÇÃO INTELIGENTE (OpenCAL / OS1-BR / CEITEC-SiC)        │
│  Litografia 3D + semicondutores de potência + sensores IoT                │
├─────────────────────────────────────────────────────────────────────────────┤
│  CAMADA 1: CADEIA DE INSUMOS (Terras Raras + Resinas + Componentes)        │
│  Colossus / Serra Verde / CEITEC / fornecedores nacionais                   │
├─────────────────────────────────────────────────────────────────────────────┤
│  CAMADA 0: GOVERNANÇA + INFRAESTRUTURA (AMSR + Evidence Bus + PQC)       │
│  SISBIN + Brasil Semicon + Finep + BNDES + FAPs + CCAI                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Próximo passo concreto:**
1. NÃO lançar ainda — é prematuro antes de P1-P8 consolidados
2. Criar **Conselho de Arquitetura de Soberania Digital** (CASD) presidido pelo MCTI
3. Publicar **Livro Branco da Soberania Digital Brasileira** em 2027 (síntese técnica + política)

---

### P10 — Estabelecer padrões de exportação para América Latina
**Score: 8.5/20.5 (Rank #10)**

| Dimensão | Avaliação |
|----------|-----------|
| **Impacto** | Médio — projeção de soft power e receita |
| **Esforço** | Médio — acordos comerciais + certificação internacional |
| **Urgência** | Longo prazo — depende de P7, P8, P9 |

**Base factual:**
- ARKHE-3DP-LITHOGRAPHY v3.1 prevê: "Primeira exportação: 2035"
- MetaLitho3D-BR e Tabletop EUV são horizontes de 10+ anos
- **Gap:** Brasil não tem tradição de exportação de equipamentos de precisão; barreiras técnicas (UL, CE, FCC) são complexas

**Mercados prioritários:**
| País | Oportunidade | Barreira | Estratégia |
|------|-------------|----------|------------|
| **Argentina** | Microfluídica para saúde (OS1-BR) | Restrições cambiais | Acordo de compensação governo-a-governo |
| **Chile** | Mineração + litografia 3D para sensores | Alto padrão de certificação | Parceria com Codelco para resinas com terras raras |
| **Colômbia** | Defesa + soberania digital | Conflito armado limita infraestrutura | Projeto piloto via BID (US$ 6-12B para América Latina) |
| **México** | Nearshoring de semicondutores (USMCA) | Competição direta com EUA | Foco em nicho: SiC para VE (BYD/Stellantis) |

**Próximo passo concreto:**
1. Mapear acordos de cooperação técnica existentes (MRE + ABC + BID)
2. Propor **Certificação ARKHE-LATAM**: padrão regional de interoperabilidade baseado em Evidence Bus + PQC
3. Primeiro produto para exportação: **OpenCAL kits educacionais** (baixo valor, alto impacto simbólico)

---

## 3. Matriz de Riscos Transversais

| ID | Risco | Prob. | Impacto | Mitigação | Owner |
|---|---|---|---|---|---|
| RT1 | **Fragmentação institucional** — MCTI, MDIC, ABIN, Serpro, Dataprev têm agendas conflitantes | Alta | Alto | CASD (Conselho de Arquitetura) com poder de decisão, não apenas consultivo | Casa Civil |
| RT2 | **Subfinanciamento crônico** — R$ 900M (AMSR) + R$ 1,5B (PNH) são insuficientes para escala industrial | Alta | Alto | BID + Fundos Setoriais + PPP (parceria com Viridis, USA Rare Earth) | MF/MFazenda |
| RT3 | **Fuga de cérebros** — engenheiros PQC e MLOps migram para EUA/Europa | Alta | Alto | Programa de residência com bolsas competitivas (R$ 15k/mês) + vínculo de 3 anos | CNPq/CAPES |
| RT4 | **Vendor lock-in estrutural** — VMware/Broadcom, AWS Outposts, Palantir criam dependência irreversível | Média | Alto | Arquitetura híbrida: open-source como base + enterprise como camada opcional (ADR-009) | Arquiteto Arkhe |
| RT5 | **Descontinuidade política** — mudança de governo em 2026/2030 interrompe programas de longo prazo | Alta | Alto | Lei da Litografia 3D Nacional + Lei de Soberania Digital (projeto de lei de iniciativa popular) | Congresso Nacional |
| RT6 | **Atraso na libharpia** — verificação formal de PQC leva >24 meses | Média | Alto | Parceria com UFC/CISSA + LNCC; usar C++ reference implementation (pqm4) como fallback | CEPESC/ABIN |
| RT7 | **Resistência da indústria** — montadoras preferem importar chips certificados (ON Semi, Infineon) | Média | Médio | Co-certificação AEC-Q101 + subsídio cruzado (Lei do Semicondutor) | MDIC |

---

## 4. Roadmap Integrado (2026–2036)

```
2026 Q3–Q4  [FASE 0]  P1 (PQC) + P2 (Schema Data Lake)
            └── Gates: libharpia CI passando; Schema v0.1 publicado; Finep submetida

2027 Q1–Q2  [FASE 1a] P3 (Governança) + P7 (Certificação HW)
            └── Gates: Evidence Bus piloto ABIN+CNPEM; OpenCAL #1-#3 operacionais

2027 Q3–Q4  [FASE 1b] P4 (Data Lake) + P5 (Interoperabilidade RFCs)
            └── Gates: Data Lake 2 regiões; 4 RFCs publicadas; SAP PM/MM PoC

2028–2029   [FASE 2]  P6 (Federados) + P8 (CEITEC→Arkhe)
            └── Gates: 50 municípios conectados; CEITEC-SiC em produção piloto

2030–2033   [FASE 3]  P9 (Plataforma Unificada)
            └── Gates: AMSR 4 regiões; máquina nacional com IA; TOONs em produção

2033–2036   [FASE 4]  P10 (Exportação AL)
            └── Gates: 1ª exportação; Certificação ARKHE-LATAM; R$ 100M em receita
```

---

## 5. Checklist de Decisões Imediatas (Próximos 30 Dias)

- [ ] **D1:** Aprovar ADR-009 v1.1 (Windmill externo) — requer parecer Legal sobre AGPLv3
- [ ] **D2:** Definir schema MQTT/JSON-LD v0.1 para Data Lake Nacional — workshop CNPEM+CTI+ABIN
- [ ] **D3:** Submeter projeto à Finep Rodada 2 (prazo: 30/09/2026) — BrazilianCAL + MLOps básico
- [ ] **D4:** Publicar RFC PQC-Brasil v0.1 (ML-KEM-768 + ML-DSA-65 + BLAKE3) — CEPESC/ABIN
- [ ] **D5:** Constituir CASD (Conselho de Arquitetura de Soberania Digital) — decreto interministerial
- [ ] **D6:** Negociar licença comercial OpenCAL com UC Berkeley — via FAPESP ou Finep
- [ ] **D7:** Mapear maturidade de TI de 50 municípios piloto — parceria FNP/CONSEGI
- [ ] **D8:** Aprovar orçamento AMSR Fase 1 (R$ 150M) — BID + Fundos Setoriais

---

**Selo:** `ARKHE-10-METAS-INTEGRACAO-v1.0-2026-08-13`  
**Status:** 🟢 **ANÁLISE COMPLETA — AGUARDANDO DECISÕES EXECUTIVAS**  
**Próxima Revisão:** Após constituição do CASD e submissão Finep Rodada 2  
**Fontes:** SISBIN-TRIADE-TECNICA-v1.0, ARKHE-3DP-LITHOGRAPHY-MLOPS-NATIONAL-PLAN-v3.1, Plano_Nacional_Producao_Hardware_v2.0, ADR-009-v1.1, ARKHE-RISK-REGISTER-v1.1
