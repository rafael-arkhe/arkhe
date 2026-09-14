/*
 * zeno_veto.c — Implementação do Efeito Zeno no Kernel AQ2
 * Bloco 1193 — O Efeito Zeno como Mecanismo de Veto
 * Protocolo: SASC-ZENO-VETO-2026
 * Plataforma: AURIX TC4x (Infineon)
 * Compilador: Tasking / HighTec
 */

#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <math.h>
#include <assert.h>
#include "coherence.h"

/* ==========================================================================
 * CONFIGURAÇÃO DO ZENO
 * ========================================================================== */

#define ZENO_TARGET_RATIO          0.618                   /* Limiar φ⁻¹ */
#define ZENO_TOLERANCE             0.01                    /* Histerese de 1% */
#define ZENO_MAX_VETO_HISTORY      1000                    /* Histórico de vetos */
#define ZENO_TAU_COHERENCE_US      50                      /* Tempo de coerência (μs) */
#define ZENO_DECAY_RATE            (1.0 / ZENO_TAU_COHERENCE_US)  /* ~20,000/s */
#define ZENO_EXP_SATURATION        20.0                    /* Z2: saturação do exp() */

/* ==========================================================================
 * ESTRUTURA DE DADOS (Estado local — reentrante)
 * ========================================================================== */

typedef struct {
    double phi_c;
    double phi_delta;
    double ratio;
    uint32_t measurement_count;
    uint32_t veto_count;
    double target_ratio;
    double tolerance;
    double stabilization_rate;
    /* Histórico circular */
    double ratio_history[ZENO_MAX_VETO_HISTORY];
    uint32_t history_ptr;
} ZenoState;

/* ==========================================================================
 * FUNÇÕES DO KERNEL
 * ========================================================================== */

/**
 * @brief Inicializa o mecanismo Zeno (O1: requer ponteiro de estado)
 * @param zeno Estado local do Zeno
 * @param target_ratio Limiar de coerência (padrão: 0.618)
 * @param tolerance Histerese (padrão: 0.01)
 * @param coh Estado de coerência inicial
 */
void Zeno_Init(ZenoState *zeno, double target_ratio, double tolerance, CoherenceState *coh) {
    assert(zeno != NULL && coh != NULL);
    zeno->target_ratio = target_ratio;
    zeno->tolerance = tolerance;
    zeno->measurement_count = 0;
    zeno->veto_count = 0;
    zeno->stabilization_rate = 1.0;
    zeno->history_ptr = 0;

    for (int i = 0; i < ZENO_MAX_VETO_HISTORY; i++) {
        zeno->ratio_history[i] = 0.0;
    }

    /* Estado inicial coerente */
    coh->phi_c = 0.70;
    coh->phi_delta = 1.00;
    coh->ratio = 0.70;
    coh->entropy = 0.30;
}

/**
 * @brief Evolução natural do estado (decoerência)
 * @param coh Estado de coerência
 * @param dt_us Intervalo de tempo em microssegundos
 */
static void Coherence_Evolve(CoherenceState *coh, uint32_t dt_us) {
    assert(coh != NULL);

    /* Z2: saturação do argumento do exp() para evitar underflow */
    double arg = -ZENO_DECAY_RATE * (double)dt_us;
    if (arg < -ZENO_EXP_SATURATION) arg = -ZENO_EXP_SATURATION;
    double decay = exp(arg);

    coh->phi_c *= decay;
    coh->phi_delta *= decay * 0.5;

    /* Z1: validação de divisão por zero, NaN e Infinito */
    if (!isfinite(coh->phi_delta) || coh->phi_delta <= 0.0) {
        coh->ratio = 0.0;
    } else {
        coh->ratio = coh->phi_c / coh->phi_delta;
    }
    coh->entropy = 1.0 - coh->ratio;
}

/**
 * @brief Reconstrução da memória (MemHarness)
 * Restaura o estado coerente após veto
 */
static void MemHarness_Reconstruct(CoherenceState *coh) {
    assert(coh != NULL);
    coh->phi_c = 0.70;
    coh->phi_delta = 1.00;
    coh->ratio = 0.70;
    coh->entropy = 0.30;
}

/**
 * @brief Execute uma medição Zeno
 * @param zeno Estado local do Zeno
 * @param coh Estado de coerência
 * @param dt_us Intervalo desde a última medição (μs)
 * @return true se veto ativado, false caso contrário
 */
bool Zeno_Measure(ZenoState *zeno, CoherenceState *coh, uint32_t dt_us) {
    assert(zeno != NULL && coh != NULL);

    Coherence_Evolve(coh, dt_us);
    zeno->measurement_count++;

    zeno->phi_c = coh->phi_c;
    zeno->phi_delta = coh->phi_delta;
    zeno->ratio = coh->ratio;

    /* Armazena no histórico (aritmética modular segura) */
    zeno->ratio_history[zeno->history_ptr] = zeno->ratio;
    zeno->history_ptr = (zeno->history_ptr + 1) % ZENO_MAX_VETO_HISTORY;

    /* Z6: atualização dinâmica da taxa de estabilização */
    zeno->stabilization_rate = 1.0 - ((double)zeno->veto_count / (double)zeno->measurement_count);

    if (zeno->ratio < (zeno->target_ratio - zeno->tolerance)) {
        zeno->veto_count++;
        zeno->stabilization_rate = 1.0 - ((double)zeno->veto_count / (double)zeno->measurement_count);
        MemHarness_Reconstruct(coh);
        return true;
    }

    return false;
}

/**
 * @brief Calcula a taxa de estabilização
 * @param zeno Estado local do Zeno
 * @return Fração de medições que evitaram colapso [0.0, 1.0]
 */
double Zeno_GetStabilizationRate(const ZenoState *zeno) {
    assert(zeno != NULL);
    if (zeno->measurement_count == 0) return 1.0;
    return 1.0 - ((double)zeno->veto_count / (double)zeno->measurement_count);
}

/**
 * @brief Calcula a taxa média de vetos no último período
 * @param zeno Estado local do Zeno
 * @param window_size Tamanho da janela de histórico
 * @return Vetos por medição no período
 */
double Zeno_GetVetoRate(const ZenoState *zeno, uint32_t window_size) {
    assert(zeno != NULL);
    if (window_size > ZENO_MAX_VETO_HISTORY) window_size = ZENO_MAX_VETO_HISTORY;
    if (zeno->measurement_count < window_size) window_size = zeno->measurement_count;
    if (window_size == 0) return 0.0;

    uint32_t vetos_in_window = 0;

    /* Z4: janela deslizante segura no buffer circular */
    for (uint32_t i = 0; i < window_size; i++) {
        uint32_t idx = (zeno->history_ptr + ZENO_MAX_VETO_HISTORY - 1 - i) % ZENO_MAX_VETO_HISTORY;
        if (zeno->ratio_history[idx] < (zeno->target_ratio - zeno->tolerance)) {
            vetos_in_window++;
        }
    }

    return (double)vetos_in_window / (double)window_size;
}

/**
 * @brief Loop de medição Zeno com Watchdog (Z5)
 * @param zeno Estado local do Zeno
 * @param coh Estado de coerência
 * @param total_cycles Número de ciclos a executar
 * @param dt_us Intervalo de medição (μs)
 */
void Zeno_RunCycles(ZenoState *zeno, CoherenceState *coh, uint32_t total_cycles, uint32_t dt_us) {
    assert(zeno != NULL && coh != NULL);
    Zeno_Init(zeno, ZENO_TARGET_RATIO, ZENO_TOLERANCE, coh);

    uint32_t watchdog_counter = 0;
    for (uint32_t i = 0; i < total_cycles; i++) {
        Zeno_Measure(zeno, coh, dt_us);

        if (zeno->measurement_count % 1000 == 0) {
            if (zeno->stabilization_rate < 0.50) {
                MemHarness_Reconstruct(coh);
                watchdog_counter++;
                if (watchdog_counter > 10) break;
            }
        }
    }
}

/* ==========================================================================
 * INTEGRAÇÃO COM O PROTOCOLO ARKHE
 * ========================================================================== */

/**
 * @brief Converte o estado Zeno para o formato do Protocolo V3.0
 * @param zeno Estado local do Zeno
 * @param coh Estado de coerência
 * @param buffer Buffer de saída
 * @param size Tamanho do buffer
 * @return Número de bytes escritos
 */
uint32_t Zeno_ExportToProtocol(const ZenoState *zeno, const CoherenceState *coh, char *buffer, uint32_t size) {
    assert(zeno != NULL && coh != NULL);
    return (uint32_t)snprintf(buffer, size,
        "{\"gamma_B\": %.4f, "
        "\"phi_c\": %.6f, "
        "\"phi_delta\": %.6f, "
        "\"ratio\": %.6f, "
        "\"entropy\": %.6f, "
        "\"measurements\": %lu, "
        "\"vetos\": %lu, "
        "\"stabilization_rate\": %.4f}",
        zeno->ratio,
        coh->phi_c,
        coh->phi_delta,
        coh->ratio,
        coh->entropy,
        (unsigned long)zeno->measurement_count,
        (unsigned long)zeno->veto_count,
        zeno->stabilization_rate
    );
}

/**
 * @brief Verifica se o sistema está no regime Zeno (coerência sustentada)
 * @param zeno Estado local do Zeno
 * @param coh Estado de coerência
 * @return true se Φ_C/Φ_Δ > 0.618 sustentado por >100 medições
 */
bool Zeno_IsInZenoRegime(const ZenoState *zeno, const CoherenceState *coh) {
    assert(zeno != NULL && coh != NULL);
    return (zeno->measurement_count > 100) &&
           (zeno->stabilization_rate > 0.90) &&
           (coh->ratio > ZENO_TARGET_RATIO);
}

/* ==========================================================================
 * TESTES UNITÁRIOS (para validação no AURIX)
 * ========================================================================== */

#ifdef ZENO_UNIT_TEST

void test_zeno_init(void) {
    ZenoState zeno;
    CoherenceState coh;
    Zeno_Init(&zeno, 0.618, 0.01, &coh);
    assert(zeno.target_ratio == 0.618);
    assert(zeno.tolerance == 0.01);
    assert(zeno.measurement_count == 0);
    assert(zeno.veto_count == 0);
    printf("✅ test_zeno_init PASSED\n");
}

void test_zeno_measure_no_veto(void) {
    ZenoState zeno;
    CoherenceState coh;
    Zeno_Init(&zeno, 0.618, 0.01, &coh);
    coh.phi_c = 0.70;
    coh.phi_delta = 1.00;

    bool veto = Zeno_Measure(&zeno, &coh, 1);
    assert(veto == false);
    assert(zeno.measurement_count == 1);
    assert(zeno.veto_count == 0);
    printf("✅ test_zeno_measure_no_veto PASSED\n");
}

void test_zeno_measure_with_veto(void) {
    ZenoState zeno;
    CoherenceState coh;
    Zeno_Init(&zeno, 0.618, 0.01, &coh);
    /* Razão inicial ~0.30 (abaixo do limiar de veto 0.608) —
       após a evolução o decaimento mantém a razão < 0.608 */
    coh.phi_c = 0.30;
    coh.phi_delta = 1.00;

    bool veto = Zeno_Measure(&zeno, &coh, 1);
    assert(veto == true);
    assert(zeno.veto_count == 1);
    assert(coh.phi_c == 0.70);
    printf("✅ test_zeno_measure_with_veto PASSED\n");
}

void test_stabilization_rate(void) {
    ZenoState zeno;
    CoherenceState coh;
    Zeno_Init(&zeno, 0.618, 0.01, &coh);
    /* Razão inicial abaixo do limiar → veto na primeira medição,
       seguido por reconstrução (sem veto na sequência) */
    coh.phi_c = 0.30;
    coh.phi_delta = 1.00;

    for (int i = 0; i < 10; i++) {
        Zeno_Measure(&zeno, &coh, 1);
    }

    double rate = Zeno_GetStabilizationRate(&zeno);
    assert(rate < 1.0);
    assert(rate > 0.0);
    printf("✅ test_stabilization_rate PASSED (rate=%.4f)\n", rate);
}

int main(void) {
    printf("=== ZENO VETO UNIT TESTS ===\n");
    test_zeno_init();
    test_zeno_measure_no_veto();
    test_zeno_measure_with_veto();
    test_stabilization_rate();
    printf("=== ALL TESTS PASSED ===\n");
    return 0;
}

#endif /* ZENO_UNIT_TEST */
