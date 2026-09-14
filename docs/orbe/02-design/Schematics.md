# Orbe — Esquemático Elétrico

**Modelo:** ORB-01 · **Versão:** v1.0 · **Data:** 2026-08-30
**Ferramenta recomendada:** KiCad / Eagle / Altium
**Documentos relacionados:** [PSD](../01-conceituacao/PSD.md), [BOM](./BOM.csv)

> **Nota:** Esta é a camada **teórica/visão** da eletrônica do Orbe. A estrutura física v1.0 é mecânica (impressão 3D) e ainda não contém a eletrônica aqui representada. O esquemático documenta o bloco de referência a ser desenvolvido na fase de integração.

---

## 1. Visão Geral Funcional (Diagrama de Blocos)

```
┌────────────────┬───────┬────────────────┐
│  Alimentação    │       │      OPLL      │
│  5V DC (USB-C)  ├───────┤  (0.96 GHz)    │
└────────┬───────┘       └───────┬────────┘
         │                       │
         ▼                       ▼
┌────────────────┬───────┬────────────────┐
│  Memristores    │       │  Metassuperfí- │
│  Si₃N₄ (10⁴)    ├───────┤  cie TiO₂      │
└────────┬───────┘       └────────┬───────┘
         │                        │
         ▼                        ▼
┌────────────────┬───────┬────────────────┐
│  Sensor de     │       │  Microcontro-  │
│  Temperatura   ├───────┤  lador RISC-V  │
│  (TMP117)      │       │  (FE310)       │
└────────────────┴───────┴────────────────┘
```

---

## 2. Blocos Funcionais

### 2.1. Alimentação

- USB-C 5 V / 2 A → regulação para 3.3 V (LDO).
- Proteção de sobrecorrente (polifusível) e polaridade reversa.
- Condicionamento para os planos de alta corrente dos memristores.

### 2.2. OPLL (Optical Phase-Locked Loop)

- Frequência central: 0.96 GHz.
- PLL para geração de handover com estabilidade ±10 ppm.
- Referência: ADF4108.

### 2.3. Matriz de Memristores Si₃N₄

- 10⁴ dispositivos em grade.
- Endereçamento via decodificador (linha/coluna).
- Circuito de leitura/escrita com compensação de fase (1.86 pJ/µm²).

### 2.4. Metassuperfície TiO₂

- Elemento difrativo ALD (feature 100 nm).
- Interface com o caminho óptico (1550 nm).

### 2.5. Sensor de Temperatura

- TMP117 (TI), ±0.1 °C, faixa −55 °C a 150 °C.
- Interface I²C com o microcontrolador.

### 2.6. Microcontrolador RISC-V

- SiFive FE310-G002 (32-bit, ~1 GHz).
- Interface UART 115200, I²C, GPIO para controle do OPLL e matriz.

---

## 3. Conexões de Alimentação e Aterramento

| Bloco | VBUS (5 V) | VCC (3.3 V) | GND |
|-------|-----------|-------------|-----|
| USB-C | x | — | x |
| LDO | x | x | x |
| RISC-V | — | x | x |
| OPLL | — | x | x |
| Memristores | x | — | x |
| TMP117 | — | x | x |
| Metassuperfície | — | x | x |

> Nota: os decoupling capacitors (100 nF) em cada bloco e o plano de aterramento contínuo (GND fill) devem ser adicionados no layout final.

---

## 4. Valores de Componentes de Referência

| Referência | Valor | Observação |
|------------|-------|------------|
| C1–C8 | 100 nF | Decoupling 3.3 V |
| C9 | 10 µF (tântalo) | Bulk 3.3 V |
| R1 | 10 kΩ | Pull-up I²C (TMP117) |
| R2 | 10 kΩ | Pull-up I²C (2º barramento) |
| F1 | Polifusível 2 A | Proteção USB |
| D1 | TVS/ESD | Proteção USB-C |
