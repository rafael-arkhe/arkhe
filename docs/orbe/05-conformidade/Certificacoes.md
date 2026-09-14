# Orbe — Certificações Regulatórias

**Modelo:** ORB-01 · **Versão:** v1.0 · **Data:** 2026-08-30

> **⚠️ STATUS:** Mapa de certificações aplicáveis. A estrutura física passiva v1.0 (sem transmissor) tem requisitos limitados; a versão com eletrônica de handover (0.96 GHz) exigirá as certificações abaixo antes da comercialização.

---

## 1. Matriz de Certificações

| Órgão | Certificação | Documentos Necessários | Aplicável a |
|-------|--------------|------------------------|-------------|
| **FCC (EUA)** | FCC ID | Testes RF/EMC, manual do usuário com avisos FCC | Versão com RF |
| **CE (Europa)** | Marcação CE | Arquivo técnico, DoC | Versão com eletrônica |
| **ANATEL (Brasil)** | Certificação de telecom | Testes de RF, manual técnico | Versão com RF (0.96 GHz) |
| **RoHS** | Conformidade | Declaração de substâncias | Todas as versões com eletrônica |

---

## 2. Estrutura Física Passiva (v1.0)

- A estrutura impressa em 3D é passiva (não emite RF ativamente).
- Aplicam-se boas práticas de segurança de produto e rotulagem.
- Não há transmissor → não requer FCC ID / ANATEL na v1.0.

---

## 3. Caminho para a Versão com RF (visão)

1. Realizar testes EMC/RF em laboratório acreditado.
2. Gerar relatórios e incorporá-los ao Arquivo Técnico.
3. Submeter a FCC, ANATEL e (se aplicável) CE-RED.
4. Emitir a DoC CE somente após aprovação dos testes.
5. Atualizar o manual do usuário com avisos regulatórios.

---

## 4. Status por Certificação

| Certificação | Status |
|--------------|--------|
| Marcação CE | ⏳ Pendente (depende do produto final) |
| FCC ID | ⏳ Pendente (requer transmissor) |
| ANATEL | ⏳ Pendente (requer transmissor) |
| RoHS | ⏳ Pendente (requer eletrônica/BOM final) |

---

## 5. Referências Normativas

| Padrão | Descrição |
|--------|-----------|
| BS 8888 | Documentação técnica de produto (desenhos) |
| IEC 82079-1 | Preparação de instruções de uso (manuais) |
| IEC 61010 | Requisitos de segurança |
| ISO 9001 | Gestão da qualidade |
| IEEE 1874 | Esquema de documentação (reparo/montagem) |
| ANSI Z535 | Sinais de segurança de produto |
