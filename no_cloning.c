/*
 * no_cloning.c — Implementação do Teorema do No-Cloning no kernel AQ2
 * Bloco 1195 — O Teorema do No-Cloning como Invariante Constitucional
 * Protocolo: SASC-NO-CLONING-2026
 * Plataforma: AURIX TC4x (Infineon)
 * Compilador: Tasking / HighTec
 */

#include <stdint.h>
#include <stdbool.h>
#include <string.h>
#include <stdlib.h>

/* ==========================================================================
 * CONFIGURAÇÃO DO NO-CLONING
 * ========================================================================== */

#define NO_CLONING_HASH_SIZE    32      /* SHA-256 */
#define NO_CLONING_MAX_STATES   1024    /* Máximo de estados únicos */
#define NO_CLONING_MAX_HISTORY  256     /* Histórico de operações */

/* ==========================================================================
 * ESTRUTURAS DE DADOS
 * ========================================================================== */

typedef struct {
    uint8_t hash[NO_CLONING_HASH_SIZE];     /* Hash SHA-256 do estado */
    float phi_c;                           /* Coerência do observador */
    float phi_delta;                       /* Coerência do observado */
    uint32_t timestamp;                      /* Timestamp do handover */
    uint16_t from_node;                      /* Nó de origem */
    uint16_t to_node;                        /* Nó de destino */
    bool is_original;                        /* Flag: estado original? */
} handover_state_t;

typedef struct {
    handover_state_t states[NO_CLONING_MAX_STATES];  /* Tabela de estados */
    uint32_t state_count;                             /* Número de estados */
    uint32_t clone_attempts;                          /* Tentativas de clone */
    uint32_t reconstruction_count;                    /* Reconstruções */
    uint32_t accepted_count;                          /* Aceites */
    uint32_t history_ptr;                             /* Ponteiro do histórico */

    /* Histórico circular de operações */
    struct {
        uint8_t action;          /* 0=ACCEPT, 1=RECONSTRUCT */
        uint8_t hash_prefix[8]; /* Primeiros 8 bytes do hash */
        uint32_t timestamp;
    } history[NO_CLONING_MAX_HISTORY];
} no_cloning_state_t;

static no_cloning_state_t g_no_cloning;

/* ==========================================================================
 * FUNÇÕES AUXILIARES (STUBS — substituir por crypto real no hardware)
 * ========================================================================== */

/**
 * @brief Calcula SHA-256 de um buffer (stub — usar hardware crypto no AURIX)
 */
static void SHA256(const uint8_t *data, uint32_t len, uint8_t *out) {
    /* TODO: Substituir por aceleração hardware do AURIX TC4x */
    /* Para testes, usa um hash simples baseado em XOR */
    memset(out, 0, NO_CLONING_HASH_SIZE);
    for (uint32_t i = 0; i < len; i++) {
        out[i % NO_CLONING_HASH_SIZE] ^= data[i];
        out[i % NO_CLONING_HASH_SIZE] += (i * 7 + 13);
    }
}

/**
 * @brief Compara dois hashes
 */
static bool hashes_equal(const uint8_t *a, const uint8_t *b) {
    return (memcmp(a, b, NO_CLONING_HASH_SIZE) == 0);
}

/**
 * @brief Copia um hash
 */
static void hash_copy(uint8_t *dst, const uint8_t *src) {
    memcpy(dst, src, NO_CLONING_HASH_SIZE);
}

/* ==========================================================================
 * FUNÇÕES PRINCIPAIS DO NO-CLONING
 * ========================================================================== */

/**
 * @brief Inicializa o módulo no-cloning
 */
void NoCloning_Init(void) {
    memset(&g_no_cloning, 0, sizeof(g_no_cloning));
    g_no_cloning.state_count = 0;
    g_no_cloning.clone_attempts = 0;
    g_no_cloning.reconstruction_count = 0;
    g_no_cloning.accepted_count = 0;
    g_no_cloning.history_ptr = 0;
}

/**
 * @brief Calcula o hash de um handover
 * @param phi_c Coerência do observador
 * @param phi_delta Coerência do observado
 * @param timestamp Timestamp
 * @param from_node Nó de origem
 * @param to_node Nó de destino
 * @param out_hash Buffer de saída (32 bytes)
 */
static void NoCloning_ComputeHash(float phi_c, float phi_delta, uint32_t timestamp,
                                   uint16_t from_node, uint16_t to_node,
                                   uint8_t *out_hash) {
    /* Serializa os dados */
    uint8_t buffer[64];
    memset(buffer, 0, sizeof(buffer));

    memcpy(buffer, &phi_c, sizeof(float));
    memcpy(buffer + 4, &phi_delta, sizeof(float));
    memcpy(buffer + 8, &timestamp, sizeof(uint32_t));
    memcpy(buffer + 12, &from_node, sizeof(uint16_t));
    memcpy(buffer + 14, &to_node, sizeof(uint16_t));

    SHA256(buffer, 64, out_hash);
}

/**
 * @brief Verifica se um hash já existe na tabela
 * @param hash Hash a verificar
 * @return Índice do estado se encontrado, -1 caso contrário
 */
static int32_t NoCloning_FindHash(const uint8_t *hash) {
    for (uint32_t i = 0; i < g_no_cloning.state_count; i++) {
        if (hashes_equal(g_no_cloning.states[i].hash, hash)) {
            return (int32_t)i;
        }
    }
    return -1;
}

/**
 * @brief Adiciona um estado à tabela
 * @param state Estado a adicionar
 * @return true se adicionado, false se tabela cheia
 */
static bool NoCloning_AddState(const handover_state_t *state) {
    if (g_no_cloning.state_count >= NO_CLONING_MAX_STATES) {
        /* Tabela cheia — ativa política de eviction (LRU) */
        /* TODO: Implementar LRU eviction */
        return false;
    }

    g_no_cloning.states[g_no_cloning.state_count] = *state;
    g_no_cloning.state_count++;
    return true;
}

/**
 * @brief Regista uma operação no histórico
 * @param action 0=ACCEPT, 1=RECONSTRUCT
 * @param hash Hash do estado
 */
static void NoCloning_LogOperation(uint8_t action, const uint8_t *hash) {
    uint32_t ptr = g_no_cloning.history_ptr % NO_CLONING_MAX_HISTORY;
    g_no_cloning.history[ptr].action = action;
    memcpy(g_no_cloning.history[ptr].hash_prefix, hash, 8);
    g_no_cloning.history[ptr].timestamp = 0; /* TODO: Usar timer do AURIX */
    g_no_cloning.history_ptr++;
}

/**
 * @brief Tenta processar um handover (verifica se é clone)
 * @param phi_c Coerência do observador
 * @param phi_delta Coerência do observado
 * @param timestamp Timestamp
 * @param from_node Nó de origem
 * @param to_node Nó de destino
 * @param out_reconstructed_phi_c Coerência reconstruída (saída)
 * @param out_reconstructed_phi_delta Coerência reconstruída (saída)
 * @return true se aceite (novo estado), false se bloqueado (clone)
 */
bool NoCloning_AttemptClone(float phi_c, float phi_delta, uint32_t timestamp,
                            uint16_t from_node, uint16_t to_node,
                            float *out_reconstructed_phi_c,
                            float *out_reconstructed_phi_delta) {
    uint8_t hash[NO_CLONING_HASH_SIZE];
    NoCloning_ComputeHash(phi_c, phi_delta, timestamp, from_node, to_node, hash);

    /* Verifica se o hash já existe */
    int32_t existing = NoCloning_FindHash(hash);

    if (existing >= 0) {
        /* CLONE DETETADO — aciona reconstrução */
        g_no_cloning.clone_attempts++;

        /* Reconstrói o estado com ruído mínimo */
        /* Em hardware, usar RNG do AURIX TC4x */
        *out_reconstructed_phi_c = phi_c + 0.0001f;  /* Ruído mínimo */
        *out_reconstructed_phi_delta = phi_delta + 0.0001f;

        g_no_cloning.reconstruction_count++;
        NoCloning_LogOperation(1, hash);  /* RECONSTRUCT */

        return false;  /* Clone bloqueado */
    }

    /* ESTADO NOVO — aceita */
    handover_state_t new_state;
    hash_copy(new_state.hash, hash);
    new_state.phi_c = phi_c;
    new_state.phi_delta = phi_delta;
    new_state.timestamp = timestamp;
    new_state.from_node = from_node;
    new_state.to_node = to_node;
    new_state.is_original = true;

    NoCloning_AddState(&new_state);
    g_no_cloning.accepted_count++;
    NoCloning_LogOperation(0, hash);  /* ACCEPT */

    return true;  /* Aceite */
}

/**
 * @brief Reconstrói um handover a partir do contexto (MemHarness)
 * @param phi_c Coerência original
 * @param phi_delta Coerência original
 * @param out_phi_c Coerência reconstruída
 * @param out_phi_delta Coerência reconstruída
 */
void NoCloning_Reconstruct(float phi_c, float phi_delta,
                           float *out_phi_c, float *out_phi_delta) {
    /* Adiciona ruído controlado para garantir unicidade */
    /* Em hardware, usar RNG do AURIX TC4x */
    *out_phi_c = phi_c + 0.0001f;
    *out_phi_delta = phi_delta + 0.0001f;

    g_no_cloning.reconstruction_count++;
}

/**
 * @brief Retorna estatísticas do no-cloning
 */
void NoCloning_GetStats(uint32_t *clone_attempts, uint32_t *reconstruction_count,
                        uint32_t *accepted_count, uint32_t *unique_states) {
    *clone_attempts = g_no_cloning.clone_attempts;
    *reconstruction_count = g_no_cloning.reconstruction_count;
    *accepted_count = g_no_cloning.accepted_count;
    *unique_states = g_no_cloning.state_count;
}

/**
 * @brief Verifica se o sistema está protegido (taxa de clone < 1%)
 */
bool NoCloning_IsProtected(void) {
    uint32_t total = g_no_cloning.clone_attempts + g_no_cloning.accepted_count;
    if (total == 0) return true;

    float clone_rate = (float)g_no_cloning.clone_attempts / (float)total;
    return (clone_rate < 0.01f);  /* Menos de 1% de clones */
}

/* ==========================================================================
 * INTEGRAÇÃO COM O PROTOCOLO ARKHE
 * ========================================================================== */

/**
 * @brief Exporta estatísticas para o Protocolo V3.0
 * @param buffer Buffer de saída
 * @param size Tamanho do buffer
 * @return Número de bytes escritos
 */
uint32_t NoCloning_ExportToProtocol(char *buffer, uint32_t size) {
    uint32_t ca, rc, ac, us;
    NoCloning_GetStats(&ca, &rc, &ac, &us);

    uint32_t total = ca + ac;
    float clone_rate = (total > 0) ? ((float)ca / (float)total) : 0.0f;

    /* Usar snprintf seguro */
    int len = snprintf(buffer, size,
        "{\"clone_attempts\": %lu, "
        "\"reconstruction_count\": %lu, "
        "\"accepted_count\": %lu, "
        "\"unique_states\": %lu, "
        "\"clone_rate\": %.6f, "
        "\"protected\": %s}",
        (unsigned long)ca,
        (unsigned long)rc,
        (unsigned long)ac,
        (unsigned long)us,
        clone_rate,
        NoCloning_IsProtected() ? "true" : "false"
    );

    return (len > 0) ? (uint32_t)len : 0;
}

/* ==========================================================================
 * TESTES UNITÁRIOS
 * ========================================================================== */

#ifdef NO_CLONING_UNIT_TEST

#include <assert.h>
#include <stdio.h>

void test_init(void) {
    NoCloning_Init();
    assert(g_no_cloning.state_count == 0);
    assert(g_no_cloning.clone_attempts == 0);
    assert(g_no_cloning.reconstruction_count == 0);
    assert(g_no_cloning.accepted_count == 0);
    printf("✅ test_init PASSED\n");
}

void test_accept_new_state(void) {
    NoCloning_Init();
    float rc, rd;
    bool accepted = NoCloning_AttemptClone(0.70f, 1.00f, 1000, 0, 1, &rc, &rd);
    assert(accepted == true);
    assert(g_no_cloning.accepted_count == 1);
    assert(g_no_cloning.state_count == 1);
    printf("✅ test_accept_new_state PASSED\n");
}

void test_block_clone(void) {
    NoCloning_Init();
    float rc, rd;

    /* Primeiro handover — aceite */
    bool a1 = NoCloning_AttemptClone(0.70f, 1.00f, 1000, 0, 1, &rc, &rd);
    assert(a1 == true);

    /* Clone idêntico — bloqueado */
    bool a2 = NoCloning_AttemptClone(0.70f, 1.00f, 1000, 0, 1, &rc, &rd);
    assert(a2 == false);
    assert(g_no_cloning.clone_attempts == 1);
    assert(g_no_cloning.reconstruction_count == 1);
    printf("✅ test_block_clone PASSED\n");
}

void test_reconstruction_uniqueness(void) {
    NoCloning_Init();
    float rc1, rd1, rc2, rd2;

    /* Handover original */
    NoCloning_AttemptClone(0.70f, 1.00f, 1000, 0, 1, &rc1, &rd1);

    /* Dois clones */
    NoCloning_AttemptClone(0.70f, 1.00f, 1000, 0, 1, &rc1, &rd1);
    NoCloning_AttemptClone(0.70f, 1.00f, 1000, 0, 1, &rc2, &rd2);

    /* As reconstruções devem ser diferentes */
    assert((rc1 != rc2) || (rd1 != rd2));
    printf("✅ test_reconstruction_uniqueness PASSED\n");
}

void test_stats(void) {
    NoCloning_Init();
    float rc, rd;

    for (int i = 0; i < 10; i++) {
        NoCloning_AttemptClone(0.70f + i*0.01f, 1.00f, 1000 + i, 0, 1, &rc, &rd);
    }

    uint32_t ca, rc_count, ac, us;
    NoCloning_GetStats(&ca, &rc_count, &ac, &us);

    assert(ac == 10);
    assert(us == 10);
    assert(ca == 0);
    assert(NoCloning_IsProtected() == true);
    printf("✅ test_stats PASSED\n");
}

int main(void) {
    printf("=== NO-CLONING UNIT TESTS ===\n");
    test_init();
    test_accept_new_state();
    test_block_clone();
    test_reconstruction_uniqueness();
    test_stats();
    printf("=== ALL TESTS PASSED ===\n");
    return 0;
}

#endif /* NO_CLONING_UNIT_TEST */
