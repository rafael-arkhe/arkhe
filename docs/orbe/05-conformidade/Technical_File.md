# Arquivo Técnico — Orbe v1.0

**Modelo:** ORB-01 · **Versão:** v1.0 · **Data:** 2026-08-30
**Finalidade:** Documentação para certificação (CE — modelo de referência)

> **⚠️ ESTADO:** Estrutura do arquivo técnico para o processo de certificação. As seções marcadas **PENDENTE** exigem testes em laboratório acreditado e ainda **não foram executadas**. Este documento não deve ser apresentado como prova de conformidade até que os relatórios sejam produzidos.

---

## 1. Descrição do Produto

- **Nome:** Orbe — estrutura de bobinas de Helmholtz.
- **Modelo:** ORB-01.
- **Substrato:** 156 — Dipolo de Arkhe.
- **Uso pretendido:** Estrutura mecânica para experimentos e desenvolvimento (v1.0) e, futuramente, dispositivo de processamento coerente.
- **Método de fabricação:** Manufatura aditiva (FDM).

### 1.1. Especificações Técnicas

| Parâmetro | Valor |
|-----------|-------|
| Diâmetro externo | 100 ±0.5 mm |
| Material | PLA (protótipo) / PEEK (produção) |
| Frequência de handover (visão) | 0.96 GHz |

---

## 2. Desenhos de Design

| Seção | Documento | Local |
|-------|-----------|-------|
| 2.1 Esquemáticos elétricos | Schematics.md | `docs/orbe/02-design/` |
| 2.2 Layout de PCB | *PENDENTE* | — |
| 2.3 Desenhos mecânicos | Mechanical_Drawings.md | `docs/orbe/02-design/` |
| 2.4 BOM | BOM.csv | `docs/orbe/02-design/` |

---

## 3. Relatórios de Teste

| Seção | Status |
|-------|--------|
| 3.1 Testes funcionais | *PENDENTE* — testes dimensionais sugeridos em Manufacturing_Documentation.md |
| 3.2 Testes de EMC | *PENDENTE* — exige laboratório acreditado |
| 3.3 Testes de segurança | *PENDENTE* |

> Nenhum relatório de teste oficial foi gerado na data deste documento.

---

## 4. Avaliação de Risco

| Seção | Status |
|-------|--------|
| 4.1 Análise de riscos (FMEA) | *PENDENTE* — a completar na fase de integração |

---

## 5. Declaração de Conformidade (DoC)

- A DoC CE não deve ser assinada até a conclusão dos testes das seções 3 e 4.
- Ver modelo em `DoC_CE.md`.

---

## 6. Observações Regulatórias

- A estrutura **física passiva** (v1.0) pode não exigir certificação RF, pois não contém transmissor ativo.
- A **eletrônica de handover** (0.96 GHz, visão) exigirá FCC (EUA), CE-RED (UE) e ANATEL (Brasil) antes da comercialização.
- As certificações aplicáveis serão detalhadas conforme o produto definitivo (ver Certificações.md).
