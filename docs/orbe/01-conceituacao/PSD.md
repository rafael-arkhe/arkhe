# Product Specification Document — Orbe v1.0

**Substrato:** 156 — Dipolo de Arkhe (estrutura Helmholtz)
**Modelo:** ORB-01
**Versão:** v1.0
**Data:** 2026-08-30
**Documentos relacionados:** [PRD](./PRD.md), [HDD](../02-design/HDD.md), [BOM](../02-design/BOM.csv)

---

## 1. Identificação do Produto

| Campo | Valor |
|-------|-------|
| Nome | Orbe |
| Modelo | ORB-01 |
| Substrato | 156 — Dipolo de Arkhe |
| Versão | v1.0 |
| Geometria | Duas bobinas de Helmholtz coaxiais |
| Método de fabricação | Manufatura aditiva (FDM) |

---

## 2. Especificações Técnicas

### 2.1. Dimensões e Massa

#### 2.1.1. Estrutura completa (montada)

| Parâmetro | Valor | Tolerância |
|-----------|-------|------------|
| Diâmetro externo | 100.0 mm | ±0.5 mm |
| Raio externo | 50.0 mm | ±0.25 mm |
| Raio interno | 39.3 mm | ±0.25 mm |
| Largura do anel (radial) | 10.7 mm | ±0.2 mm |
| Altura de cada anel | 5.0 mm | ±0.1 mm |
| Distância entre planos dos anéis (centro a centro, visão) | 100.0 mm | ±0.5 mm |
| Massa (protótipo PLA) | ~150 g | ±20 g |

#### 2.1.2. Sistema de coordenadas da mesa (peça)

| Eixo | Alcance | Extensão |
|------|---------|----------|
| X | 10.0–110.0 mm | 100.0 mm (diâmetro) |
| Y | 10.0–210.0 mm | 200.0 mm (eixo das bobinas) |
| Z | 0.2–5.2 mm | 5.0 mm em cada anel + pontes |

> Os valores já refletem a correção de escala 50/56 aplicada pelo `patch_orbe_gcode.py`, centrada nos eixos das bobinas.

### 2.2. Materiais

| Componente | Material | Especificação |
|------------|----------|---------------|
| Chassi/estrutura (protótipo) | PLA | 200–220 °C, impressão FDM |
| Chassi/estrutura (produção, 77 K) | PEEK ou PA12+CF | Resistente a criogenia |
| Suportes de ponte | Mesmo material do anel | Com resfriamento ativo (ventoinha) |

#### 2.2.1. Conceito teórico (materiais, visão)

| Componente | Material | Especificação |
|------------|----------|---------------|
| Metassuperfície | TiO₂ | ALD, feature size 100 nm |
| Memristores | Si₃N₄ com grafeno | 10⁴ dispositivos , 1.86 pJ/µm² |
| Núcleo criogênico | Plasma de xenônio | 0.1–10 Torr, 77 K |

### 2.3. Parâmetros Operacionais (visão)

| Parâmetro | Valor | Tolerância |
|-----------|-------|------------|
| Frequência de handover | 0.96 GHz | ±10 ppm |
| Coerência mínima | 0.85 | ±0.01 |
| Potência consumida | 1.5 W (média) | ±0.5 W |
| Temperatura operacional | 77 K–300 K | — |

### 2.4. Parâmetros de Impressão (produto real)

| Parâmetro | Valor | Observação |
|-----------|-------|------------|
| Nozzle | 0.4 mm | — |
| Altura de camada | 0.2 mm | — |
| Infill | 20% | — |
| Extrusão | Absoluta (M82) | — |
| Retração | 1.5 mm (absoluta) | Inserida nos travels |
| Z-hop (transição de anel) | 10.0 mm | Segurança |
| Resfriamento de ponte | M106 S255 | 100% |
| Velocidade de impressão | 1500 mm/min | Padrão do arquivo |

---

## 3. Interfaces

### 3.1. Estrutura Física

| Interface | Descrição |
|-----------|-----------|
| Mecânica | Encaixe para componentes; superfícies de suporte das bobinas |
| Termomecânica | Flange/assento para montagem criogênica (produção) |

### 3.2. Conceito Teórico

| Interface | Descrição |
|-----------|-----------|
| Óptica | Handovers em 0.96 GHz |
| Elétrica | USB-C (5 V, 2 A) |
| Mecânica | Rosca de fixação M6 |
| Comunicação | Serial (UART) a 115200 bps |

---

## 4. Documentação de Fabricação

| Item | Arquivo | Local |
|------|---------|-------|
| Modelo gerador G-code | `Orbe_Helmholtz.gcode` (original) | raiz do workspace |
| G-code corrigido (produção) | `Orbe_Helmholtz_100mm_corrigido.gcode` | raiz do workspace |
| Script de correção | `patch_orbe_gcode.py` | raiz do workspace |
| Instruções de fabricação | [Manufacturing Documentation](../03-fabricacao/Manufacturing_Documentation.md) | Fase 3 |

---

## 5. Controle de Alterações

| Versão | Data | Alterações |
|--------|------|------------|
| v1.0 | 2026-08-30 | Versão inicial com base na análise estática do G-code real |
