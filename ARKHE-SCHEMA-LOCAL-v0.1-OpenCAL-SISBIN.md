# Schema v0.1 de Dados de Processo — OpenCAL + Classificação SISBIN
## Documento Técnico: P2.5 — Protótipo de Schema Local

**Documento:** `ARKHE-SCHEMA-LOCAL-v0.1-OpenCAL-SISBIN`  
**Versão:** 0.1  
**Data:** 2026-08-13  
**Status:** 🟡 **DRAFT TÉCNICO — PARA VALIDAÇÃO EM WORKSHOP CNPEM+CTI+ABIN**  
**Selo:** `ARKHE-SCHEMA-LOCAL-v0.1-2026-08-13`  
**Dependências:** SISBIN-TRIADE v1.0 (Parte I, Seção 4.2), ARKHE-3DP-LITHOGRAPHY-MLOPS-NATIONAL-PLAN v3.1 (Seção 4.2, 6.3.1)

---

## 1. Escopo e Princípios

**Escopo:** Este schema define a estrutura de dados para coleta, serialização e classificação de métricas de processo da plataforma OpenCAL em ambiente de pesquisa nacional. É um **schema local** (nível de laboratório), não o Data Lake Nacional (P4). Sua função é:
1. Permitir ingestão imediata de dados de sensores OpenCAL via MQTT.
2. Antecipar a conformidade com a classificação de dados do SISBIN (sigiloso/reservado/secreto/ultrassecreto).
3. Servir como base para consenso no workshop técnico (P2) e evolução para schema nacional v1.0.

**Princípios:**
- **JSON-LD:** Semântica explícita via `@context`; interoperabilidade futura com Data Lake Nacional.
- **MQTT 5.0:** Leveza para edge devices (RPi 5); tópicos hierárquicos por laboratório e máquina.
- **Classificação SISBIN:** Todo dado carrega metadado de classificação desde a origem (privacy/security by design).
- **Extensibilidade:** Campos customizáveis por laboratório sem quebrar validação do schema base.

---

## 2. Arquitetura de Tópicos MQTT

### 2.1 Estrutura Hierárquica

```
arkhe/{laboratorio_id}/{maquina_id}/{categoria}/{classificacao}/{metrica}
```

| Segmento | Valores Válidos | Descrição |
|---|---|---|
| `laboratorio_id` | `lnnano`, `cti`, `ufrgs`, `ufsc`, `usp` | Identificador do laboratório executor |
| `maquina_id` | `opencal-01`, `opencal-02`, `os1-01` | Identificador da máquina (UUID curto recomendado) |
| `categoria` | `processo`, `sensor`, `ambiente`, `sistema` | Domínio da métrica |
| `classificacao` | `publico`, `sigiloso`, `reservado` | Classificação SISBIN simplificada para ambiente acadêmico |
| `metrica` | Ver tabelas abaixo | Nome da métrica |

> **Nota:** Em ambiente acadêmico Finep 2026, `secreto` e `ultrassecreto` não se aplicam. O schema os reserva para evolução SISBIN.

### 2.2 Exemplos de Tópicos

```
arkhe/lnnano/opencal-01/processo/sigiloso/temperatura_resina
arkhe/lnnano/opencal-01/sensor/publico/posicao_rotacao
arkhe/cti/opencal-02/ambiente/publico/temperatura_ambiente
arkhe/ufrgs/opencal-03/sistema/publico/status_conexao
```

---

## 3. Schema JSON-LD: Mensagem Base

### 3.1 Contexto Semântico (@context)

```json
{
  "@context": {
    "arkhe": "https://arkhe.org/schema/v0.1#",
    "sosa": "http://www.w3.org/ns/sosa/",
    "ssn": "http://www.w3.org/ns/ssn/",
    "sisbin": "https://sisbin.gov.br/schema/classificacao#",
    "schema": "https://schema.org/",
    "xsd": "http://www.w3.org/2001/XMLSchema#",
    "qudt": "http://qudt.org/schema/qudt/",
    "unit": "http://qudt.org/vocab/unit/"
  }
}
```

> **Nota:** O namespace `arkhe` é provisório. Após consenso no workshop, deve ser registrado em `https://arkhe.org/schema/` ou equivalente institucional (CNPEM/CTI).

### 3.2 Estrutura da Mensagem

```json
{
  "@context": "https://arkhe.org/schema/v0.1/context.jsonld",
  "@type": "arkhe:OpenCALProcessObservation",
  "@id": "urn:arkhe:obs:{uuid}",

  "schema:identifier": "obs-2026-08-13T14:30:00Z-opencal-01-001",
  "schema:dateCreated": "2026-08-13T14:30:00Z",
  "schema:creator": {
    "@type": "schema:Organization",
    "schema:name": "LNNano/CNPEM",
    "schema:identifier": "03.654.119/0001-85"
  },

  "sisbin:classificacao": {
    "@type": "sisbin:ClassificacaoInformacao",
    "sisbin:nivel": "sigiloso",
    "sisbin:baseLegal": "Lei 12.527/2011, Art. 7º, IV",
    "sisbin:cadeiaCustodia": "urn:arkhe:cadeia:{parent_uuid}"
  },

  "sosa:madeBySensor": {
    "@type": "sosa:Sensor",
    "schema:name": "DHT22-Resina-01",
    "schema:serialNumber": "DHT22-7F3A9B",
    "sosa:observes": "arkhe:TemperaturaResina"
  },

  "sosa:observedProperty": "arkhe:TemperaturaResina",
  "sosa:hasResult": {
    "@type": "qudt:QuantityValue",
    "qudt:numericValue": 28.5,
    "qudt:unit": "unit:DEG_C"
  },

  "arkhe:condicaoProcesso": {
    "arkhe:jobId": "job-2026-08-13-001",
    "arkhe:geometria": "cilindro_10mm",
    "arkhe:resina": "FormLabs_Clear_v2",
    "arkhe:layerAtual": 0,
    "arkhe:totalLayers": 1200
  },

  "arkhe:provenance": {
    "arkhe:softwareVersion": "opencal-br v0.1.0",
    "arkhe:firmwareVersion": "rp5-fw-1.4.2",
    "arkhe:calibracaoSensor": "2026-08-01",
    "arkhe:pipelineVersion": "arkhe-mlops-v0.1.0"
  }
}
```

### 3.3 Campos Obrigatórios vs. Opcionais

| Campo | Obrigatório | Descrição |
|---|---|---|
| `@context` | ✅ | URI do contexto JSON-LD |
| `@type` | ✅ | Tipo da observação |
| `@id` | ✅ | UUID v4 da observação |
| `schema:dateCreated` | ✅ | Timestamp ISO 8601 UTC |
| `schema:creator` | ✅ | Instituição produtora (CNPJ) |
| `sisbin:classificacao` | ✅ | Nível SISBIN + base legal |
| `sosa:madeBySensor` | ✅ | Identificação do sensor |
| `sosa:observedProperty` | ✅ | Propriedade observada (URI arkhe) |
| `sosa:hasResult` | ✅ | Valor numérico + unidade QUDT |
| `arkhe:condicaoProcesso` | ⚠️ Condicional | Obrigatório para categoria `processo`; opcional para `ambiente` |
| `arkhe:provenance` | ⚠️ Recomendado | Versões de software/firmware/calibração |

---

## 4. Schema por Categoria de Métrica

### 4.1 Categoria: `processo` (Parâmetros de Impressão)

| Métrica (tópico) | Tipo | Unidade (QUDT) | Frequência | Classificação |
|---|---|---|---|---|
| `temperatura_resina` | `float` | `unit:DEG_C` | 1 Hz | `sigiloso` (pode revelar formulação) |
| `dose_luz` | `float` | `unit:W-PER-M2` | Por layer | `sigiloso` |
| `tempo_exposicao` | `float` | `unit:SEC` | Por layer | `publico` |
| `velocidade_rotacao` | `float` | `unit:REV-PER-MIN` | 1 Hz | `publico` |
| `posicao_eixo_z` | `float` | `unit:MilliM` | Por layer | `publico` |
| `angulo_projecao` | `float` | `unit:DEG` | Por layer | `publico` |
| `intensidade_projecao` | `float` | `unit:W` | Por layer | `sigiloso` |
| `numero_layer` | `int` | `unit:UNITLESS` | Por layer | `publico` |
| `tempo_layer` | `float` | `unit:SEC` | Por layer | `publico` |
| `status_cura` | `string` (`iniciada` / `em_progresso` / `concluida` / `falha`) | — | Por layer | `publico` |

### 4.2 Categoria: `sensor` (Dados Brutos de Sensores)

| Métrica (tópico) | Tipo | Unidade (QUDT) | Frequência | Classificação |
|---|---|---|---|---|
| `temperatura_sensor` | `float` | `unit:DEG_C` | 1 Hz | `publico` |
| `umidade_relativa` | `float` | `unit:PERCENT` | 1 Hz | `publico` |
| `posicao_rotacao_encoder` | `int` | `unit:UNITLESS` (pulsos) | 10 Hz | `publico` |
| `corrente_motor` | `float` | `unit:A` | 10 Hz | `publico` |
| `tensao_motor` | `float` | `unit:V` | 10 Hz | `publico` |

### 4.3 Categoria: `ambiente` (Condições Ambientais)

| Métrica (tópico) | Tipo | Unidade (QUDT) | Frequência | Classificação |
|---|---|---|---|---|
| `temperatura_ambiente` | `float` | `unit:DEG_C` | 0.1 Hz | `publico` |
| `umidade_ambiente` | `float` | `unit:PERCENT` | 0.1 Hz | `publico` |
| `pressao_atmosferica` | `float` | `unit:PA` | 0.1 Hz | `publico` |
| `vibracao_mesa` | `float` | `unit:M-PER-SEC2` | 100 Hz | `publico` |

### 4.4 Categoria: `sistema` (Metadados de Runtime)

| Métrica (tópico) | Tipo | Unidade | Frequência | Classificação |
|---|---|---|---|---|
| `status_conexao` | `string` (`online` / `offline` / `degradado`) | — | Evento | `publico` |
| `latencia_rede_ms` | `int` | `unit:MilliSEC` | 0.1 Hz | `publico` |
| `uso_cpu_percent` | `float` | `unit:PERCENT` | 0.1 Hz | `publico` |
| `uso_memoria_mb` | `int` | `unit:MB` | 0.1 Hz | `publico` |
| `espaco_disco_gb` | `float` | `unit:GB` | 0.01 Hz | `publico` |
| `versao_software` | `string` | — | Evento | `publico` |

---

## 5. Classificação SISBIN Simplificada para Ambiente Acadêmico

### 5.1 Mapeamento de Níveis

| Nível SISBIN (Completo) | Nível Schema v0.1 | Critério de Aplicação (OpenCAL) |
|---|---|---|
| `publico` | `publico` | Dados de runtime, ambientais, posicionamento mecânico, versões de software aberto |
| `sigiloso` | `sigiloso` | Parâmetros de processo que revelam know-how (dose, temperatura de cura, intensidade óptica); dados de calibração |
| `reservado` | `reservado` | Formulações de resina em teste (antes de patente); resultados de inspeção de defeitos que identificam vulnerabilidades de processo |
| `secreto` | — (reservado para evolução) | Não aplicável em Finep 2026 |
| `ultrassecreto` | — (reservado para evolução) | Não aplicável em Finep 2026 |

### 5.2 Regras de Classificação Automática

```yaml
regras_classificacao:
  - condicao: "categoria == 'processo' AND metrica in ['temperatura_resina', 'dose_luz', 'intensidade_projecao']"
    classificacao: "sigiloso"
    justificativa: "Parâmetros críticos de processo; revelam know-how de otimização"

  - condicao: "categoria == 'processo' AND metrica in ['tempo_exposicao', 'velocidade_rotacao', 'posicao_eixo_z', 'angulo_projecao', 'numero_layer', 'tempo_layer', 'status_cura']"
    classificacao: "publico"
    justificativa: "Parâmetros operacionais genéricos; não revelam formulação"

  - condicao: "categoria == 'sensor' OR categoria == 'ambiente' OR categoria == 'sistema'"
    classificacao: "publico"
    justificativa: "Dados de infraestrutura e ambiente; não sensíveis"

  - condicao: "arkhe:condicaoProcesso.resina contains 'nacional' OR arkhe:condicaoProcesso.resina contains 'experimental'"
    classificacao: "reservado"
    justificativa: "Resina em desenvolvimento; proteção pré-patente"
```

---

## 6. Protocolo de Ingestão MQTT → Kafka → Data Lake Local

### 6.1 Fluxo de Dados

```
┌─────────────────┐     MQTT 5.0      ┌─────────────────┐     Kafka Producer      ┌─────────────────┐
│  OpenCAL (RPi5) │────────────────────▶│  MQTT Broker    │────────────────────────▶│  Kafka (KRaft)  │
│  + Sensores     │  TLS 1.3 + mTLS    │  (Mosquitto/   │  JSON-LD payload      │  Single-node    │
│                 │  Client cert (ICP)  │   EMQX Edge)    │  + classificação      │  (laboratório)  │
└─────────────────┘                     └─────────────────┘                       └─────────────────┘
                                                                                           │
                                                                                           ▼
                                                                                  ┌─────────────────┐
                                                                                  │  MinIO (S3)     │
                                                                                  │  Raw Storage    │
                                                                                  │  (WORM para     │
                                                                                  │   sigiloso+)    │
                                                                                  └─────────────────┘
                                                                                           │
                                                                                           ▼
                                                                                  ┌─────────────────┐
                                                                                  │  PostgreSQL     │
                                                                                  │  (Metadados +   │
                                                                                  │   Catalogação)  │
                                                                                  └─────────────────┘
```

### 6.2 Configuração do Broker MQTT (Mosquitto)

```conf
# mosquitto.conf — Configuração por Laboratório
listener 8883
protocol mqtt

# TLS 1.3 + mTLS
require_certificate true
cafile /etc/mosquitto/ca_certificates/ca.crt
certfile /etc/mosquitto/certs/server.crt
keyfile /etc/mosquitto/certs/server.key
tls_version tlsv1.3

# ACL por classificação
acl_file /etc/mosquitto/acl/classificacao.acl

# Persistência
persistence true
persistence_location /var/lib/mosquitto/

# QoS 1 para dados de processo (garantia de entrega)
# QoS 0 para dados ambientais (tolerante a perda)
```

### 6.3 ACL por Classificação (Exemplo)

```
# classificacao.acl
# Regra: dados sigiloso+ só são publicáveis por dispositivos autenticados
# e consumíveis pelo pipeline Kafka local (não exposto externamente)

user opencal-01
 topic write arkhe/lnnano/opencal-01/+/publico/+
 topic write arkhe/lnnano/opencal-01/+/sigiloso/+
 topic write arkhe/lnnano/opencal-01/+/reservado/+

user kafka-bridge
 topic read arkhe/lnnano/+/+/publico/+
 topic read arkhe/lnnano/+/+/sigiloso/+
 topic read arkhe/lnnano/+/+/reservado/+
 topic write kafka-local/arkhe/+/+/+/+

user dashboard-publico
 topic read arkhe/lnnano/+/+/publico/+
```

---

## 7. Schema de Evolução (v0.1 → v1.0)

| Versão | Mudança | Gatilho |
|---|---|---|
| **v0.1** (atual) | Schema local; 4 categorias; classificação simplificada; 1 broker/lab | Workshop CNPEM+CTI+ABIN (agosto/2026) |
| **v0.2** | Adicionar categoria `qualidade` (inspeção pós-impressão: dimensões, defeitos) | Primeira impressão calibrada (M6) |
| **v0.3** | Adicionar categoria `mlops` (features, predictions, drift) | Modelo de otimização em produção (M12) |
| **v0.4** | Adicionar suporte a `secreto` / `ultrassecreto`; integração com Evidence Bus | Integração SISBIN (2027+) |
| **v1.0** | Schema nacional aprovado por CTI-Arkhe (Comitê Técnico); namespace permanente | Consenso inter-laboratórios + ABIN |

---

## 8. Validação e Testes

### 8.1 JSON Schema de Validação (Draft 2020-12)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://arkhe.org/schema/v0.1/opencal-observation.schema.json",
  "title": "OpenCAL Observation v0.1",
  "type": "object",
  "required": ["@context", "@type", "@id", "schema:dateCreated", "schema:creator", "sisbin:classificacao", "sosa:madeBySensor", "sosa:observedProperty", "sosa:hasResult"],
  "properties": {
    "@context": { "type": "string", "format": "uri" },
    "@type": { "const": "arkhe:OpenCALProcessObservation" },
    "@id": { "type": "string", "pattern": "^urn:arkhe:obs:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$" },
    "schema:dateCreated": { "type": "string", "format": "date-time" },
    "sisbin:classificacao": {
      "type": "object",
      "required": ["sisbin:nivel"],
      "properties": {
        "sisbin:nivel": { "enum": ["publico", "sigiloso", "reservado", "secreto", "ultrassecreto"] },
        "sisbin:baseLegal": { "type": "string" },
        "sisbin:cadeiaCustodia": { "type": "string" }
      }
    },
    "sosa:hasResult": {
      "type": "object",
      "required": ["qudt:numericValue", "qudt:unit"],
      "properties": {
        "qudt:numericValue": { "type": "number" },
        "qudt:unit": { "type": "string", "format": "uri" }
      }
    }
  }
}
```

### 8.2 Teste de Conformidade (Checklist)

- [ ] Mensagem JSON-LD válida segundo schema acima.
- [ ] `@id` é UUID v4 único por observação.
- [ ] `schema:dateCreated` é ISO 8601 UTC (timezone explícito `Z`).
- [ ] `sisbin:nivel` está no enum permitido.
- [ ] `qudt:unit` é URI QUDT válida.
- [ ] Tópico MQTT corresponde à estrutura hierárquica definida.
- [ ] Broker rejeita publicação em tópico de classificação superior à credencial do cliente.
- [ ] Kafka consumer consegue desserializar payload e extrair `sisbin:nivel` para roteamento de tópico.

---

## 9. Exemplo Completo: Ciclo de Impressão

### 9.1 Tópicos Publicados Durante 1 Layer

```
# Layer 47 de 1200 — Cilindro 10mm, Resina FormLabs Clear v2

T+0ms   arkhe/lnnano/opencal-01/processo/publico/numero_layer
        → {"qudt:numericValue": 47, "qudt:unit": "unit:UNITLESS", ...}

T+0ms   arkhe/lnnano/opencal-01/processo/publico/posicao_eixo_z
        → {"qudt:numericValue": 2.35, "qudt:unit": "unit:MilliM", ...}

T+0ms   arkhe/lnnano/opencal-01/processo/sigiloso/dose_luz
        → {"qudt:numericValue": 450.0, "qudt:unit": "unit:W-PER-M2", ...}

T+0ms   arkhe/lnnano/opencal-01/processo/sigiloso/temperatura_resina
        → {"qudt:numericValue": 28.5, "qudt:unit": "unit:DEG_C", ...}

T+500ms arkhe/lnnano/opencal-01/processo/publico/status_cura
        → {"value": "em_progresso", ...}

T+2000ms arkhe/lnnano/opencal-01/processo/publico/status_cura
         → {"value": "concluida", ...}

T+2000ms arkhe/lnnano/opencal-01/sensor/publico/temperatura_sensor
         → {"qudt:numericValue": 29.1, "qudt:unit": "unit:DEG_C", ...}
```

---

## 10. Considerações de Segurança

1. **mTLS obrigatório:** Certificados ICP-Brasil para broker MQTT; client certs por dispositivo (RPi 5).
2. **Criptografia em repouso:** Dados `sigiloso` e `reservado` armazenados em MinIO com SSE-S3 (chave local, não AWS).
3. **Isolamento de rede:** Broker MQTT em VLAN isolada; Kafka acessível apenas via bridge autorizado.
4. **Rotação de credenciais:** Client certs renovados a cada 90 dias; CRL publicada no PostgreSQL de metadados.
5. **Não-repúdio:** Cada mensagem de categoria `processo` deve ser assinada com Ed25519 (futuro: ML-DSA-65 via libharpia) no campo `arkhe:assinatura`.

---

**Selo:** `ARKHE-SCHEMA-LOCAL-v0.1-2026-08-13`  
**Status:** 🟡 **DRAFT TÉCNICO — AGUARDANDO WORKSHOP DE VALIDAÇÃO**  
**Próximo Passo:** Workshop CNPEM + CTI + ABIN (agosto/2026) → consenso → v0.2 → teste em OpenCAL #1 (LNNano, out/2026)  
**Contato Técnico:** Coordenação MLOps / Núcleo de Dados ARKHE
