# Hardware Design Document — Orbe v1.0

**Substrato:** 156 — Dipolo de Arkhe (estrutura Helmholtz)
**Versão:** v1.0
**Data:** 2026-08-30
**Documentos de referência:** [PRD](../01-conceituacao/PRD.md), [PSD](../01-conceituacao/PSD.md), [BOM](./BOM.csv)

---

## 1. Introdução

- **Escopo:** Design do hardware da estrutura Helmholtz do Orbe, documentando a geometria, o processo de fabricação (G-code corrigido) e a base para a montagem de componentes.
- **Definições:**
  - **Coerência C(t):** métrica de coerência do campo (conceito teórico).
  - **Handover:** transmissão de estado entre Orbes (conceito teórico).
  - **Satoshi:** unidade mínima de estado armazenado.
- **Documentos de referência:** PRD, PSD, BOM.

---

## 2. Visão Geral do Sistema

### 2.1. Princípio de Operação

O Orbe físico é uma **bobina de Helmholtz** composta por dois anéis coaxiais. O princípio de operação funcional (visão/roadmap) é um ciclo de 7 etapas:

1. Coleta de fótons e RF ambiente.
2. Atualização do campo de coerência C(t).
3. Verificação: C(t) ≥ 0.85?
4. Emissão de handover (se sim).
5. Recepção de handovers (escuta passiva).
6. Aprendizado via consenso distribuído.
7. Relaxamento para estado de espera.

> Na versão v1.0 (física), a estrutura materializa os dois planos de bobina; o ciclo acima é a camada teórica a ser integrada.

### 2.2. Diagrama de Blocos Funcional

```
                 ┌──────────────────────────────┐
     ┌──────────▶│       ANEL Z+ (superior)     │
     │           │  R_ex=50mm  R_int=39.3mm     │
     │           │  centro X=60, Y=60-c          │
     │           └──────────────┬───────────────┘
     │                          │
     │        suportes de ponte │ (4 abas, M106 S255)
     │                          ▼
     │           ┌──────────────────────────────┐
     │           │       ANEL Z- (inferior)     │
     │           │  R_ex=50mm  R_int=39.3mm     │
     │  Base     │  centro X=60, Y=160-c        │
     └───────────│  (plano de montagem)         │
                 └──────────────────────────────┘
```

> `-c` indica o deslocamento de escala aplicado (correção de 56 mm → 50 mm).

### 2.3. Arquitetura de Hardware

- **Estrutura mecânica:** dois anéis + pontes (impressão 3D).
- **Plano de montagem:** superfícies no anel inferior para fixação de eletrônica (visão).
- **Trilha de sinais (visão):** interfaces óptica/elétrica descritas no PSD.

---

## 3. Especificação de Interfaces

### 3.1. Interface Mecânica (estrutura)

| Parâmetro | Valor |
|-----------|-------|
| Raio externo | 50.0 mm |
| Raio interno | 39.3 mm |
| Largura do anel | 10.7 mm |
| Altura do anel | 5.0 mm |
| Eixo das bobinas | Compartilhado (coaxial) |

### 3.2. Interface Óptica (visão)

| Parâmetro | Valor |
|-----------|-------|
| Comprimento de onda | 1550 nm |
| Modulação | PSK |
| Taxa de dados | 10 kbps |

### 3.3. Interface Elétrica (visão)

| Parâmetro | Valor |
|-----------|-------|
| Tensão | 5 V DC |
| Corrente | 2 A (pico) |
| Conector | USB-C |

---

## 4. Justificativa de Design

### 4.1. Decisões de Projeto

| Decisão | Justificativa |
|---------|---------------|
| Geometria de Helmholtz | Campo magnético quase uniforme no centro; dois anéis coaxiais |
| Escala 50/56 (R 56→50 mm) | Atende especificação de diâmetro 100 mm exigida pelo PRD |
| Escala centrada nos eixos | Preserva o eixo compartilhado e o afastamento entre os planos das bobinas |
| Impressão em PLA (protótipo) | Baixo custo; validação geométrica |
| PEEK/PA12+CF (produção) | Compatível com 77 K (ver PSD) |
| Pontes com resfriamento | Garante integridade dos suportes entre os anéis |
| Retração 1.5 mm absoluta | Evita stringing em travels; corrigido por análise estática |

### 4.2. Trade-offs

- **Raio 50 mm vs. 56 mm:** redução de tamanho para atender à especificação de 100 mm.
- **PLA vs. PEEK:** PLA para protótipo, PEEK para produção a 77 K.
- **Extrusão absoluta contínua:** adicionamos `G92 E0` entre camadas do anel Z+ para evitar sub-extrusão (o anel Z- já é contínuo).

### 4.3. Correções incorporadas (análise estática)

| Correção | Motivo |
|----------|--------|
| `G92 E0` no início de cada camada do anel Z+ | O arquivo original reiniciava E sem reset, causando recuo de ~17.4 mm por camada |
| Z-hop de 10 mm na transição Z+→Z- | Evita dragar o bico sobre o material |
| Retração em travels | Evita stringing |
| M106/M107 nas pontes | Resfriamento ativo obrigatório para spans sem suporte |
