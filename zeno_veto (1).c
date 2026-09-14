/*
 * zeno_veto.c — Implementação do Efeito Zeno no Kernel AQ2
 * Bloco 1193 — O Efeito Zeno como Mecanismo de Veto
 * Protocolo: SASC-ZENO-VETO-2026
 * Plataforma: AURIX TC4x (Infineon)
 * Compilador: Tasking / HighTec
 */

#include <stdint.h>
#include <stdbool.h>
#include <math.h>

/* ==========================================================================
 * CONFIGURAÇÃO DO ZENO
 * ========================================================================== */

#define ZENO_TARGET_RATIO       0.618f      /* Limiar φ⁻¹ */
#define ZENO_TOLERANCE          0.01f       /* Histerese de 1% */
#define ZENO_MAX_VETO_HISTORY   1000        /* Histórico de vetos */
#define ZENO_TAU_COHERENCE_US   50          /* Tempo de coerência (μs) */
#define ZENO_DECAY_RATE         (1.0f / ZENO_TAU_COHERENCE_US) /* ~20,000/s */

/* ==========================================================================
 * ESTRUTURAS DE DADOS
 * ========================================================================== */

typedef struct {
    float phi_c;            /* Coerência do observador */
    float phi_delta;        /* Coerência do observado */
    float ratio;            /* Razão Φ_C / Φ_Δ */
    float entropy;          /* Entropia = 1 - ratio */
} CoherenceState;

typedef struct {
    float phi_c;
    float phi_delta;
    float ratio;
    uint32_t measurement_count;
    uint32_t veto_count;
    float target_ratio;
    float tolerance;
    float stabilization_rate;
    /* Histórico circular */
    float ratio_history[ZENO_MAX_VETO_HISTORY];
    uint32_t history_ptr;
} ZenoState;

/* ==========================================================================
 * VARIÁVEIS GLOBAIS
 * ========================================================================== */

static ZenoState g_zeno;
static CoherenceState g_coherence;

/* ==========================================================================
 * FUNÇÕES DO KERNEL
 * ========================================================================== */

/**
 * @brief Inicializa o mecanismo Zeno
 * @param target_ratio Limiar de coerência (padrão: 0.618)
 * @param tolerance Histerese (padrão: 0.01)
 */
void Zeno_Init(float target_ratio, float tolerance) {
    g_zeno.target_ratio = target_ratio;
    g_zeno.tolerance = tolerance;
    g_zeno.measurement_count = 0;
    g_zeno.veto_count = 0;
    g_zeno.stabilization_rate = 1.0f;
    g_zeno.history_ptr = 0;

    for (int i = 0; i < ZENO_MAX_VETO_HISTORY; i++) {
        g_zeno.ratio_history[i] = 0.0f;
    }

    /* Estado inicial coerente */
    g_coherence.phi_c = 0.70f;
    g_coherence.phi_delta = 1.00f;
    g_coherence.ratio = 0.70f;
    g_coherence.entropy = 0.30f;
}

/**
 * @brief Evolução natural do estado (decoerência)
 * @param dt_us Intervalo de tempo em microssegundos
 */
static void Coherence_Evolve(uint32_t dt_us) {
    float decay = expf(-ZENO_DECAY_RATE * (float)dt_us);
    g_coherence.phi_c *= decay;
    g_coherence.phi_delta *= decay * 0.5f;  /* Decoerência mais lenta do observado */
    g_coherence.ratio = g_coherence.phi_c / (g_coherence.phi_delta + 1e-12f);
    g_coherence.entropy = 1.0f - g_coherence.ratio;
}

/**
 * @brief Reconstrução da memória (MemHarness)
 * Restaura o estado coerente após veto
 */
static void MemHarness_Reconstruct(void) {
    g_coherence.phi_c = 0.70f;
    g_coherence.phi_delta = 1.00f;
    g_coherence.ratio = 0.70f;
    g_coherence.entropy = 0.30f;
}

/**
 * @brief Executa uma medição Zeno
 * @param dt_us Intervalo desde a última medição (μs)
 * @return true se veto ativado, false caso contrário
 */
bool Zeno_Measure(uint32_t dt_us) {
    /* Evolução natural */
    Coherence_Evolve(dt_us);

    g_zeno.measurement_count++;

    /* Calcula a razão atual */
    g_zeno.phi_c = g_coherence.phi_c;
    g_zeno.phi_delta = g_coherence.phi_delta;
    g_zeno.ratio = g_coherence.ratio;

    /* Armazena no histórico */
    g_zeno.ratio_history[g_zeno.history_ptr % ZENO_MAX_VETO_HISTORY] = g_zeno.ratio;
    g_zeno.history_ptr++;

    /* Verifica se a razão está abaixo do limiar (com histerese) */
    if (g_zeno.ratio < (g_zeno.target_ratio - g_zeno.tolerance)) {
        g_zeno.veto_count++;
        /* Aciona o Veto de Anúbis: reconstrução da memória */
        MemHarness_Reconstruct();
        return true;  /* Veto ativado */
    }

    return false;  /* Sem veto */
}

/**
 * @brief Calcula a taxa de estabilização
 * @return Fração de medições que evitaram colapso [0.0, 1.0]
 */
float Zeno_GetStabilizationRate(void) {
    if (g_zeno.measurement_count == 0) return 1.0f;
    return 1.0f - ((float)g_zeno.veto_count / (float)g_zeno.measurement_count);
}

/**
 * @brief Calcula a taxa média de vetos no último período
 * @param window_size Tamanho da janela de histórico
 * @return Vetos por medição no período
 */
float Zeno_GetVetoRate(uint32_t window_size) {
    if (window_size > ZENO_MAX_VETO_HISTORY) window_size = ZENO_MAX_VETO_HISTORY;
    if (g_zeno.history_ptr < window_size) window_size = g_zeno.history_ptr;

    uint32_t vetos_in_window = 0;
    uint32_t start = (g_zeno.history_ptr > window_size) ? (g_zeno.history_ptr - window_size) : 0;

    for (uint32_t i = start; i < g_zeno.history_ptr; i++) {
        if (g_zeno.ratio_history[i % ZENO_MAX_VETO_HISTORY] < 
            (g_zeno.target_ratio - g_zeno.tolerance)) {
            vetos_in_window++;
        }
    }

    return (float)vetos_in_window / (float)window_size;
}

/**
 * @brief Loop principal do kernel AQ2
 * Executa o ciclo de medição Zeno continuamente
 */
void AQ2_MainLoop(void) {
    Zeno_Init(ZENO_TARGET_RATIO, ZENO_TOLERANCE);

    uint32_t dt_us = 1;  /* Medição a cada 1 μs */

    while (1) {
        bool veto = Zeno_Measure(dt_us);

        if (veto) {
            /* Log do veto (para debug/auditoria) */
            /* TODO: Enviar para o handover logger */
        }

        /* Verificação periódica da taxa de estabilização */
        if (g_zeno.measurement_count % 1000 == 0) {
            float stab = Zeno_GetStabilizationRate();
            float veto_rate = Zeno_GetVetoRate(1000);

            /* Se a taxa de estabilização cair abaixo de 90%, 
               aciona alarme de degradação */
            if (stab < 0.90f) {
                /* TODO: Acionar protocolo de emergência */
            }
        }

        /* Delay de 1 μs (implementação depende do hardware) */
        /* TODO: Usar timer do AURIX TC4x */
    }
}

/* ==========================================================================
 * INTEGRAÇÃO COM O PROTOCOLO ARKHE
 * ========================================================================== */

/**
 * @brief Converte o estado Zeno para o formato do Protocolo V3.0
 * @param buffer Buffer de saída
 * @param size Tamanho do buffer
 * @return Número de bytes escritos
 */
uint32_t Zeno_ExportToProtocol(char *buffer, uint32_t size) {
    return snprintf(buffer, size,
        "{\"gamma_B\": %.4f, "
        "\"phi_c\": %.6f, "
        "\"phi_delta\": %.6f, "
        "\"ratio\": %.6f, "
        "\"entropy\": %.6f, "
        "\"measurements\": %lu, "
        "\"vetos\": %lu, "
        "\"stabilization_rate\": %.4f}",
        g_zeno.ratio,  /* gamma_B mapeado para ratio */
        g_coherence.phi_c,
        g_coherence.phi_delta,
        g_coherence.ratio,
        g_coherence.entropy,
        (unsigned long)g_zeno.measurement_count,
        (unsigned long)g_zeno.veto_count,
        g_zeno.stabilization_rate
    );
}

/**
 * @brief Verifica se o sistema está no regime Zeno (coerência sustentada)
 * @return true se Φ_C/Φ_Δ > 0.618 sustentado por >100 medições
 */
bool Zeno_IsInZenoRegime(void) {
    return (g_zeno.measurement_count > 100) && 
           (g_zeno.stabilization_rate > 0.90f) &&
           (g_coherence.ratio > ZENO_TARGET_RATIO);
}

/* ==========================================================================
 * TESTES UNITÁRIOS (para validação no AURIX)
 * ========================================================================== */

#ifdef ZENO_UNIT_TEST

#include <assert.h>
#include <stdio.h>

void test_zeno_init(void) {
    Zeno_Init(0.618f, 0.01f);
    assert(g_zeno.target_ratio == 0.618f);
    assert(g_zeno.tolerance == 0.01f);
    assert(g_zeno.measurement_count == 0);
    assert(g_zeno.veto_count == 0);
    printf("✅ test_zeno_init PASSED\n");
}

void test_zeno_measure_no_veto(void) {
    Zeno_Init(0.618f, 0.01f);
    g_coherence.phi_c = 0.70f;
    g_coherence.phi_delta = 1.00f;

    bool veto = Zeno_Measure(1);
    assert(veto == false);
    assert(g_zeno.measurement_count == 1);
    assert(g_zeno.veto_count == 0);
    printf("✅ test_zeno_measure_no_veto PASSED\n");
}

void test_zeno_measure_with_veto(void) {
    Zeno_Init(0.618f, 0.01f);
    g_coherence.phi_c = 0.60f;  /* Abaixo do limiar */
    g_coherence.phi_delta = 1.00f;

    bool veto = Zeno_Measure(1);
    assert(veto == true);
    assert(g_zeno.veto_count == 1);
    assert(g_coherence.phi_c == 0.70f);  /* Reconstruído */
    printf("✅ test_zeno_measure_with_veto PASSED\n");
}

void test_stabilization_rate(void) {
    Zeno_Init(0.618f, 0.01f);
    g_coherence.phi_c = 0.60f;
    g_coherence.phi_delta = 1.00f;

    for (int i = 0; i < 10; i++) {
        Zeno_Measure(1);
    }

    float rate = Zeno_GetStabilizationRate();
    assert(rate < 1.0f);
    assert(rate > 0.0f);
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
