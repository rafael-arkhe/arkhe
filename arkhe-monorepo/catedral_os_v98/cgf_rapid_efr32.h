/*
 * ========================================================================
 * cgf_rapid_efr32.h — Nó Sensor de Borda (C para EFR32MG24)
 * ========================================================================
 * Catedral OS v8.7 — Substrato 163/168
 * Nó de borda que coleta o CGF (coerência) e o reporta ao orquestrador.
 *
 * Target  : Silicon Labs EFR32MG24 (Series 2, 2.4 GHz Zigbee/Thread)
 *           - Secure Vault, AES-256, HW acceleration
 * Toolchain: GNU ARM Embedded (arm-none-eabi-gcc) + Gecko SDK
 *
 * Função: Integra como nó sensor RAPID dentro da rede da Clareira.
 *         Cada nó mede o α local (coerência) via CGF e o propaga pelo
 *         mesh 6LoWPAN/Zigbee ao gateway (cathedral_orchestrator.py).
 *
 * Invariantes:
 *   [Ghost-1] Substrate Integrity: build hash verificado no boot.
 *   [Gap-2]   Entropy Budget: seed de TRV (True Random Value) interno.
 *   [Runtime-3] Healthcheck Response: heartbeat a cada 60s.
 * ========================================================================
 */

#ifndef CGF_RAPID_EFR32_H
#define CGF_RAPID_EFR32_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ------------------------------------------------------------------------ *
 * Constantes do Nó Sensor
 * ------------------------------------------------------------------------ */
#define CGF_HEARTBEAT_MS        60000UL    /* Runtime-3: heartbeat 60s      */
#define CGF_ALPHA_HIGH          0.95f      /* Veto de Anúbis: α>=0.95       */
#define CGF_ALPHA_WARN          0.85f      /* warning / escalate            */
#define CGF_BUF_LEN             128        /* janela de histórico por nó    */
#define CGF_PAYLOAD_MAX         64         /* MTU do frame RAPID            */

/* Limiar de emergência: dispara kill-switch local (< 0.2s de recuperação) */
#define CGF_RECOVERY_MS         200UL

/* ------------------------------------------------------------------------ *
 * Estado do nó sensor
 * ------------------------------------------------------------------------ */
typedef enum {
    CGF_FIRMWARE_BOOTING = 0,
    CGF_FIRMWARE_ACTIVE,
    CGF_FIRMWARE_DEGRADED,
    CGF_FIRMWARE_VETO_ARMED,      /* α >= 0.85 */
    CGF_FIRMWARE_VETO_ACTIVATED,  /* α >= 0.95 (NÃO standby) */
    CGF_FIRMWARE_OFFLINE
} cgf_firmware_state_t;

typedef struct {
    uint8_t  node_id[8];          /* EUI-64 do EFR32MG24                     */
    cgf_firmware_state_t state;   /* estado operacional do firmware          */
    float    alpha_local;         /* coerência local CGF (0.0 .. 1.0)        */
    uint16_t heartbeat_count;     /* contagem de heartbeats emitidos         */
    uint32_t last_heartbeat_ms;   /* instante do último heartbeat            */
    uint8_t  buffer[CGF_BUF_LEN]; /* histórico circular de amostras          */
    uint16_t buffer_head;         /* cabeça do buffer circular               */
    uint8_t  build_sha[32];       /* SHA-256 do firmware (Ghost-1)           */
} cgf_sensor_t;

/* ------------------------------------------------------------------------ *
 * API pública do nó sensor
 * ------------------------------------------------------------------------ */

/**
 * @brief Inicializa o nó sensor de borda.
 * @param sensor   estrutura do nó a inicializar
 * @param node_id  EUI-64 do dispositivo (8 bytes)
 * @param build_sha assinatura SHA-256 do firmware (32 bytes)
 *
 * @note Requer radio stack (6LoWPAN/Zigbee) já inicializada pelo SDK.
 * @invariant Ghost-1: verifica o hash de build antes de ativar.
 */
void cgf_sensor_init(cgf_sensor_t *sensor,
                     const uint8_t node_id[8],
                     const uint8_t build_sha[32]);

/**
 * @brief Alimenta uma nova amostra de coerência (α) ao nó.
 * @param sensor       nó sensor
 * @param alpha_sample amostra de coerência bruta em [0.0, 1.0]
 *
 * Atualiza o buffer circular e reavalia o estado do firmware,
 * armando ou ATIVANDO o veto conforme α.
 */
void cgf_sensor_feed(cgf_sensor_t *sensor, float alpha_sample);

/**
 * @brief Efetua a atualização periódica (call a cada tick de 1 ms no SysTick).
 * @param sensor nó sensor
 *
 * Emite heartbeat quando cgf_heartbeat_due() retorna true,
 * implementando o Runtime-3 de healthcheck.
 */
void cgf_sensor_tick(cgf_sensor_t *sensor);

/**
 * @brief Verifica se o heartbeat de 60 s deve ser emitido.
 * @param sensor     nó sensor
 * @param now_ms     tick corrente (ms)
 * @retval true      momento de emitir heartbeat
 */
bool cgf_heartbeat_due(const cgf_sensor_t *sensor, uint32_t now_ms);

/**
 * @brief Constrói o payload RAPID (JSON leve) para transmissão.
 * @param sensor  nó sensor
 * @param out     buffer de saída (>= CGF_PAYLOAD_MAX bytes)
 * @param out_len capacidade do buffer de saída
 * @return        número de bytes escritos
 */
size_t cgf_sensor_frame(const cgf_sensor_t *sensor,
                        char *out, size_t out_len);

/* ------------------------------------------------------------------------ *
 * Gerenciamento de tempo (porta para o SysTick / RTC do EFR32MG24)
 * ------------------------------------------------------------------------ */

/**
 * @brief Leitura do tick de tempo em milissegundos.
 * @return tick corrente em ms (função portável fornecida pela placa)
 */
uint32_t cgf_now_ms(void);

#ifdef __cplusplus
}
#endif

#endif /* CGF_RAPID_EFR32_H */
