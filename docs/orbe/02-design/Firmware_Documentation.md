# Firmware Documentation — Orbe v1.0

**Modelo:** ORB-01 · **Versão:** v1.0 · **Data:** 2026-08-30
**Documentos relacionados:** [HDD](./HDD.md), [Schematics](./Schematics.md), [BOM](./BOM.csv)

> **Nota:** A documentação de firmware refere-se à camada **teórica/visão** de processamento coerente do Orbe. A estrutura física v1.0 é mecânica (impressão 3D) e será acompanhada do firmware na fase de integração.

---

## 1. Arquitetura de Firmware

### 1.1. Visão Geral

O firmware do Orbe é implementado como uma máquina de estados finitos (FSM) com 7 estados principais, refletindo o ciclo de coerência descrito no HDD.

### 1.2. Diagrama de Estados

```
┌─────────┐    ┌─────────┐    ┌─────────┐
│ COOLDOWN│───▶│  IDLE   │───▶│ COLLECT │
└─────────┘    └─────────┘    └─────────┘
     ▲              │              │
     │              ▼              ▼
┌─────────┐    ┌─────────┐    ┌─────────┐
│  RELAX  │◀───│  EMIT   │◀───│ UPDATE  │
└─────────┘    └─────────┘    └─────────┘
```

| Estado | Descrição |
|--------|-----------|
| COOLDOWN | Aguarda intervalo entre ciclos |
| IDLE | Pronto, sem processamento ativo |
| COLLECT | Coleta de fotons/RF ambiente |
| UPDATE | Atualiza campo de coerência C(t) |
| EMIT | Emite handover se C(t) ≥ 0.85 |
| RELAX | Relaxa para espera |

### 1.3. Módulos de Software

| Módulo | Descrição | Arquivo |
|--------|-----------|---------|
| `core.c` | Loop principal e FSM | src/core.c |
| `allen_cahn.c` | Simulação da equação de Allen-Cahn | src/allen_cahn.c |
| `opll.c` | Controle do OPLL | src/opll.c |
| `memristor.c` | Interface com memristores | src/memristor.c |
| `handover.c` | Emissão e recepção de handovers | src/handover.c |
| `temp.c` | Leitura do sensor TMP117 | src/temp.c |

---

## 2. API Documentation

### 2.1. Funções Principais

```c
/**
 * @brief Inicializa o Orbe
 * @return 0 em caso de sucesso, -1 em caso de erro
 */
int orbe_init(void);

/**
 * @brief Executa um passo da simulação Allen-Cahn
 * @param dt Passo de tempo (recomendado: 0.001)
 * @return Coerência C(t) atualizada
 */
float allen_cahn_step(float dt);

/**
 * @brief Emite um handover
 * @param state Estado codificado do Orbe (C, F, Satoshi, ω)
 * @return 0 em caso de sucesso
 */
int emit_handover(HandoverState state);

/**
 * @brief Lê a temperatura do núcleo (TMP117)
 * @return Temperatura em Kelvin
 */
float read_core_temp_k(void);

/**
 * @brief Escreve um estado no memristor
 * @param idx Índice do dispositivo
 * @param value Valor (0 ou 1)
 * @return 0 em caso de sucesso
 */
int memristor_write(uint32_t idx, uint8_t value);
```

### 2.2. Estruturas de Dados

```c
typedef struct {
    float coherence;      /* C(t) */
    float frequency;      /* 0.96 GHz central */
    uint64_t satoshi;     /* unidade mínima de estado */
    float omega;          /* frequência angular */
} HandoverState;
```

---

## 3. Memory Map

| Endereço | Tamanho | Descrição |
|----------|---------|-----------|
| 0x0000–0x0FFF | 4 KB | Código do bootloader |
| 0x1000–0x3FFF | 12 KB | Código do firmware |
| 0x4000–0x4FFF | 4 KB | Configuração do OPLL |
| 0x5000–0x5FFF | 4 KB | Estado dos memristores (Satoshi) |

---

## 4. Build e Toolchain

- **Toolchain:** riscv64-unknown-elf-gcc (SiFive Freedom Studio).
- **Placa alvo:** HiFive1 (FE310-G002) — referência.
- **Porta serial:** UART 115200 bps.
- **Build:** `make` (Makefile `firmware/Makefile`).

---

## 5. Testes

| Teste | Procedimento | Critério |
|-------|--------------|----------|
| Boot | UART exibe banner | Mensagem de boot em < 1 s |
| Coerência | Simulação Allen-Cahn contínua | C(t) converge em tempo real |
| Handover | Emitir e verificar frame | Detecção em < 1 ms |
| Temperatura | Ler TMP117 | ±0.1 °C de precisão |

---

## 6. Changelog

| Versão | Data | Alterações |
|--------|------|------------|
| v1.0 | 2026-08-30 | Versão inicial (estrutura/roadmap) |
