# Product Requirements Document — Orbe v1.0

**Substrato:** 156 — Dipolo de Arkhe (estrutura Helmholtz)
**Versão:** v1.0
**Data:** 2026-08-30
**Status:** Rascunho para revisão
**Documentos relacionados:** [PSD](./01-conceituacao/PSD.md), [HDD](./02-design/HDD.md), [BOM](./02-design/BOM.csv)

---

## 1. Visão Geral

- **Nome do Produto:** Orbe (estrutura de bobinas de Helmholtz)
- **Modelo:** ORB-01
- **Propósito:** Dispositivo de processamento coerente implementado sobre uma estrutura física de bobinas de Helmholtz impressa em 3D, que serve de base mecânica para experimentos de coerência, handover e consolidação de estados.
- **Público-alvo:** Pesquisadores, entusiastas de computação quântica e laboratórios.
- **Ambiente de operação:** Laboratório (temperatura ambiente para protótipo PLA; 77 K–300 K para versão de produção em PEEK/PA12+CF).

### 1.1. Natureza Híbrida do Produto

Este documento descreve, de forma híbrida, duas camadas do Orbe:

| Camada | Descrição | Maturidade |
|--------|-----------|------------|
| **Estrutura física (real)** | Estrutura mecânica de duas bobinas de Helmholtz coaxiais, impressa em 3D (R=50 mm). | Implementada — fabricável a partir do G-code corrigido (`Orbe_Helmholtz_100mm_corrigido.gcode`). |
| **Conceito teórico/visão** | Processador coerente com campo C(t), handovers, memristores e metassuperfície. | Visão/roadmap — alvo de pesquisa subsequente. |

A documentação formaliza a **estrutura física** como produto real e registra o **conceito teórico** como requisitos de visão.

---

## 2. Requisitos Funcionais

### 2.1. Estrutura Física (produto real)

| ID | Requisito | Prioridade |
|----|-----------|------------|
| RF-01 | Deve conter duas bobinas de Helmholtz coaxiais, montadas verticalmente | Crítica |
| RF-02 | O diâmetro externo do conjunto deve ser 100 mm (±0.5 mm) | Crítica |
| RF-03 | Cada anel deve ter raio externo 50 mm e raio interno ~39 mm | Crítica |
| RF-04 | As bobinas devem ser unidas por suportes de ponte (≥4 abas) | Alta |
| RF-05 | A estrutura deve ser imprimível em impressora 3D FDM (nozzle 0.4 mm) | Alta |
| RF-06 | Deve aceitar encaixe/fixação de componentes eletrônicos | Média |
| RF-07 | Deve ser produzida com retração e resfriamento adequados (qualidade de impressão) | Alta |

### 2.2. Conceito Teórico (visão/roadmap)

| ID | Requisito | Prioridade |
|----|-----------|------------|
| RF-20 | O Orbe deve gerar coerência plasmóide C(t) mensurável | Crítica (visão) |
| RF-21 | O Orbe deve emitir handovers em ~0.96 GHz | Alta (visão) |
| RF-22 | O Orbe deve armazenar estado em memristores Si₃N₄ | Alta (visão) |
| RF-23 | O Orbe deve receber handovers de outros Orbes | Média (visão) |

---

## 3. Requisitos de Desempenho

### 3.1. Estrutura Física

| Métrica | Valor | Tolerância |
|---------|-------|------------|
| Diâmetro externo | 100 mm | ±0.5 mm |
| Alcance X da peça | 10–110 mm | (escala 50/56) |
| Alcance Y da peça | 10–210 mm | (dois anéis) |
| Altura de camada | 0.2 mm | — |
| Camadas por anel | 25 | — |
| Largura do anel | 10.7 mm (12 mm → 10.7 mm) | ±0.2 mm |
| Retração | 1.5 mm (absoluta, M82) | — |
| Resfriamento de ponte | 100% (M106 S255) | — |

### 3.2. Conceito Teórico

| Métrica | Valor |
|---------|-------|
| Coerência C(t) | ≥ 0.85 (limiar de handover) |
| Frequência de handover | 0.96 GHz ± 10 ppm |
| Alcance de comunicação | 10 km (linha de visada) |
| Latência de handover | < 1 ms |

---

## 4. Restrições Físicas

| Parâmetro | Valor | Tolerância |
|-----------|-------|------------|
| Diâmetro | 100 mm | ±0.5 mm |
| Massa (protótipo PLA) | ~150 g | ±20 g |
| Temperatura operacional | 77 K–300 K (alvo) | — |
| Temperatura de impressão (PLA) | 200–220 °C | — |
| Tensão de operação (eletrônica, visão) | 2.5 V DC | — |

---

## 5. Requisitos de Segurança

- [ ] Proteção contra sobrecorrente (eletrônica, fase visão)
- [ ] Circuit breaker controlado por DAO (fase visão)
- [ ] Redundância de memristores (paridade) (fase visão)
- [ ] Vedação hermética IP68+ (versão criogênica)
- [ ] Aviso de superfície fria (≤ 77 K) na versão criogênica
- [ ] Uso apenas de materiais compatíveis com 77 K (PEEK/PA12+CF) na versão de produção

---

## 6. Critérios de Aceitação

1. A estrutura impressa atinge diâmetro externo de 100 ±0.5 mm.
2. Os dois anéis permanecem coaxiais (eixo compartilhado).
3. Os suportes de ponte conectam os anéis sem defeitos visíveis (resfriamento ativo).
4. Não há retração inadequada — extrusão absoluta contínua dentro de cada anel.
5. O G-code de produção é o `Orbe_Helmholtz_100mm_corrigido.gcode` (corrigido conforme análise estática).

---

## 7. Fora de Escopo (v1.0)

- Eletrônica embarcada de controle (fase visão)
- Metassuperfície e memristores (fase visão)
- Certificação de telecomunicações (fase visão; ver Fase 5)
- Criogenia ativa integrada
