/*
 * observador_primordial.c — Observador Primordial (Kernel AQ2)
 * Bloco 1193 — O Observador que Testemunha o Estado de Coerência
 * Protocolo: SASC-OBSERVADOR-PRIMORDIAL-2026
 * Plataforma: AURIX TC4x (Infineon)
 * Compilador: Tasking / HighTec
 */

#include <stdint.h>
#include <stdbool.h>
#include <string.h>
#include <stdio.h>
#include <assert.h>
#include "coherence.h"

/* ==========================================================================
 * CONFIGURAÇÃO DO OBSERVADOR
 * ========================================================================== */

#define OBSERVADOR_LIMIAR        0.618                    /* Limiar de coerência */
#define OBSERVADOR_MAX_HISTORY   1024                     /* Histórico circular */

/* ==========================================================================
 * ESTRUTURA DE DADOS (Estado local — reentrante)
 * ========================================================================== */

typedef struct {
    uint32_t observation_count;
    uint32_t awake_count;
    uint32_t asleep_count;
    double total_awake_time;
    double last_coherence;
    double last_awake_start;
    bool is_awake;
    bool was_awake;
    bool is_processing;     /* O2: Flag anti-reentrância */

    struct {
        double time;
        double phi_c;
        double phi_delta;
        double ratio;
        bool is_awake;
    } history[OBSERVADOR_MAX_HISTORY];
    uint32_t history_ptr;
} PrimordialState;

/* ==========================================================================
 * FUNÇÕES DO KERNEL
 * ========================================================================== */

/**
 * @brief Inicializa o observador (O1: requer ponteiro de estado — reentrante)
 * @param obs Estado local do observador
 */
void ObservadorPrimordial_Init(PrimordialState *obs) {
    assert(obs != NULL);
    memset(obs, 0, sizeof(PrimordialState));
}

/**
 * @brief Testemunha o estado de coerência (não o modifica)
 * @param obs Estado local do observador
 * @param state Estado de coerência observado
 * @param time_ms Instante da observação (ms)
 * @return true se o observador está desperto
 */
bool ObservadorPrimordial_Observe(PrimordialState *obs, const CoherenceState *state, double time_ms) {
    assert(obs != NULL && state != NULL);
    obs->observation_count++;
    obs->last_coherence = state->ratio;

    bool was_awake = obs->was_awake;
    obs->is_awake = (state->ratio > OBSERVADOR_LIMIAR);
    obs->was_awake = obs->is_awake;

    if (obs->is_awake && !was_awake) {
        obs->awake_count++;
        obs->last_awake_start = time_ms;
    } else if (!obs->is_awake && was_awake) {
        obs->asleep_count++;
        obs->total_awake_time += (time_ms - obs->last_awake_start);
    }

    uint32_t ptr = obs->history_ptr;
    obs->history[ptr].time = time_ms;
    obs->history[ptr].phi_c = state->phi_c;
    obs->history[ptr].phi_delta = state->phi_delta;
    obs->history[ptr].ratio = state->ratio;
    obs->history[ptr].is_awake = obs->is_awake;

    obs->history_ptr = (ptr + 1) % OBSERVADOR_MAX_HISTORY;
    return obs->is_awake;
}

bool ObservadorPrimordial_IsAwake(const PrimordialState *obs) {
    assert(obs != NULL);
    return obs->is_awake;
}

void ObservadorPrimordial_GetStats(const PrimordialState *obs, uint32_t *obs_count, uint32_t *awake_count,
                                    uint32_t *asleep_count, double *total_awake_time,
                                    double *last_coherence, bool *is_awake) {
    assert(obs != NULL);
    *obs_count = obs->observation_count;
    *awake_count = obs->awake_count;
    *asleep_count = obs->asleep_count;
    *total_awake_time = obs->total_awake_time;
    *last_coherence = obs->last_coherence;
    *is_awake = obs->is_awake;
}

double ObservadorPrimordial_GetAwakeFraction(const PrimordialState *obs, double total_time_ms) {
    assert(obs != NULL);
    if (total_time_ms <= 0.0) return 0.0;
    return obs->total_awake_time / total_time_ms;
}

bool ObservadorPrimordial_IsSustainedAwake(const PrimordialState *obs) {
    assert(obs != NULL);
    if (obs->observation_count < 100) return false;

    for (uint32_t i = 0; i < 100; i++) {
        uint32_t idx = (obs->history_ptr + OBSERVADOR_MAX_HISTORY - 1 - i) % OBSERVADOR_MAX_HISTORY;
        if (!obs->history[idx].is_awake) return false;
    }
    return true;
}

CoherenceState ObservadorPrimordial_Cycle(PrimordialState *obs, CoherenceState state, double time_ms,
                                          bool (*zeno_veto_func)(CoherenceState*)) {
    assert(obs != NULL);

    /* O2: Guarda contra recursão e loop de realimentação */
    if (obs->is_processing) return state;
    obs->is_processing = true;

    bool awake = ObservadorPrimordial_Observe(obs, &state, time_ms);

    if (!awake && zeno_veto_func != NULL) {
        zeno_veto_func(&state);
    }

    /* Segunda observação pós-intervenção */
    ObservadorPrimordial_Observe(obs, &state, time_ms);

    obs->is_processing = false;
    return state;
}

uint32_t ObservadorPrimordial_ExportToProtocol(const PrimordialState *obs, char *buffer, uint32_t size) {
    assert(obs != NULL);
    uint32_t obs_count, awake, asleep;
    double awake_time, last_coh;
    bool awake_flag;

    ObservadorPrimordial_GetStats(obs, &obs_count, &awake, &asleep, &awake_time, &last_coh, &awake_flag);

    int len = snprintf(buffer, size,
        "{\"observation_count\": %lu, "
        "\"awake_count\": %lu, "
        "\"asleep_count\": %lu, "
        "\"total_awake_time\": %.6f, "
        "\"last_coherence\": %.6f, "
        "\"is_awake\": %s, "
        "\"is_sustained\": %s}",
        (unsigned long)obs_count, (unsigned long)awake, (unsigned long)asleep,
        awake_time, last_coh,
        awake_flag ? "true" : "false",
        ObservadorPrimordial_IsSustainedAwake(obs) ? "true" : "false"
    );

    return (len > 0) ? (uint32_t)len : 0;
}

/* ==========================================================================
 * TESTES UNITÁRIOS (para validação no AURIX)
 * ========================================================================== */

#ifdef OBSERVADOR_UNIT_TEST

void test_obs_init(void) {
    PrimordialState obs;
    ObservadorPrimordial_Init(&obs);
    assert(obs.observation_count == 0);
    assert(obs.awake_count == 0);
    assert(obs.asleep_count == 0);
    assert(obs.is_awake == false);
    printf("✅ test_obs_init PASSED\n");
}

void test_obs_observe_awake(void) {
    PrimordialState obs;
    ObservadorPrimordial_Init(&obs);

    CoherenceState st = { .phi_c = 0.70, .phi_delta = 1.00, .ratio = 0.70, .entropy = 0.30 };
    bool awake = ObservadorPrimordial_Observe(&obs, &st, 0.0);
    assert(awake == true);
    assert(obs.observation_count == 1);
    assert(obs.awake_count == 1);
    assert(obs.is_awake == true);
    printf("✅ test_obs_observe_awake PASSED\n");
}

void test_obs_transition(void) {
    PrimordialState obs;
    ObservadorPrimordial_Init(&obs);

    CoherenceState st = { .phi_c = 0.70, .phi_delta = 1.00, .ratio = 0.70, .entropy = 0.30 };
    ObservadorPrimordial_Observe(&obs, &st, 0.0);          /* awake */
    st.ratio = 0.40;
    ObservadorPrimordial_Observe(&obs, &st, 10.0);         /* asleep */
    assert(obs.asleep_count == 1);
    assert(obs.total_awake_time == 10.0);
    printf("✅ test_obs_transition PASSED\n");
}

void test_obs_sustained_awake(void) {
    PrimordialState obs;
    ObservadorPrimordial_Init(&obs);

    CoherenceState st = { .phi_c = 0.70, .phi_delta = 1.00, .ratio = 0.80, .entropy = 0.20 };
    for (int i = 0; i < 150; i++) {
        ObservadorPrimordial_Observe(&obs, &st, (double)i);
    }
    assert(ObservadorPrimordial_IsSustainedAwake(&obs) == true);
    printf("✅ test_obs_sustained_awake PASSED\n");
}

int main(void) {
    printf("=== OBSERVADOR PRIMORDIAL UNIT TESTS ===\n");
    test_obs_init();
    test_obs_observe_awake();
    test_obs_transition();
    test_obs_sustained_awake();
    printf("=== ALL TESTS PASSED ===\n");
    return 0;
}

#endif /* OBSERVADOR_UNIT_TEST */
