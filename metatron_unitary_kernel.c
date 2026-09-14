/*
 * metatron_unitary_kernel.c — Alicerce Metatrônico (kernel núcleo)
 * BLOCO 507 — Cubo de Metatron: evolução unitária de vetor de estado 13-dim.
 * Protocolo: SASC-METATRON-UNITARY-2026
 * Plataforma: AURIX TC4x (Infineon) / host (gcc -std=c99)
 *
 * Ground-truth (vetagem BLOCO 507 v66):
 *  - Houve REJEIÇÃO de: campo phi_sync (coherence.h REAL possui apenas
 *    phi_c/phi_delta/ratio/entropy); structs Metatron/TGN/Gravitons;
 *    "LU com PPU-SIMD (<10 us)" (micro-otimização não verificável);
 *    Redis/TGN/TrustGraphs/SensorAdapter/theorem_generator (inexistentes).
 *  - Este kernel implementa a PARTE VETADA: o cara-gate de Cayley sobre o
 *    grafo completo K13 (78 = C(13,2) conexões, grau 12) e o contrato físico
 *    unitário: conservação de norma, handover = |psi[0]|^2, domínio [0,1].
 *  - A costura com o Observador usa SOMENTE a API real
 *    (CoherenceState de coherence.h + ObservadorPrimordial_Observe).
 *
 * Determinístico: sem srand, sem aleatoriedade, sem rede.
 */

#include <complex.h>
#include <math.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>
#include <stdio.h>
#include <assert.h>
#include "coherence.h"

#define METATRON_DIM     13
#define METATRON_DEGREE  12.0   /* grau do grafo completo K13 (78 = C(13,2)) */

typedef struct {
    double complex U[METATRON_DIM][METATRON_DIM];   /* porta de Cayley do cubo */
    double complex psi[METATRON_DIM];               /* vetor de estado unitário */
    double dt;                                      /* passo temporal */
    double gamma_b;                                 /* campo de fundo (B) */
    double handover;                                /* cache |psi[0]|^2 */
    double norm;                                    /* cache |psi|^2 */
    uint32_t steps;                                 /* passos acumulados */
    bool initialized;
} MetatronCube;

/* --------------------------------------------------------------------------
 * ÁLGEBRA LINEAR COMPLEXA (13x13, determinística, pivô parcial por módulo)
 * -------------------------------------------------------------------------- */

static void cmat_mul(int n, const double complex (*a)[METATRON_DIM],
                     const double complex (*b)[METATRON_DIM],
                     double complex (*out)[METATRON_DIM]) {
    double complex acc;
    for (int i = 0; i < n; i++) {
        for (int j = 0; j < n; j++) {
            acc = 0.0 + 0.0 * I;
            for (int k = 0; k < n; k++) acc += a[i][k] * b[k][j];
            out[i][j] = acc;
        }
    }
}

/* Inversa por Gauss-Jordan com pivô parcial (maior módulo). Retorna 0 se ok. */
static int cmat_inv(int n, const double complex (*a)[METATRON_DIM],
                    double complex (*inv)[METATRON_DIM]) {
    double complex aug[METATRON_DIM][2 * METATRON_DIM];
    if (n > METATRON_DIM || 2 * n > 2 * METATRON_DIM) return -2;

    for (int i = 0; i < n; i++) {
        for (int j = 0; j < n; j++) aug[i][j] = a[i][j];
        for (int j = 0; j < n; j++) aug[i][n + j] = (i == j) ? 1.0 : 0.0;
    }

    for (int col = 0; col < n; col++) {
        int piv = col;
        double best = -1.0;
        for (int r = col; r < n; r++) {
            double mag = cabs(aug[r][col]);
            if (mag > best) { best = mag; piv = r; }
        }
        if (!(best > 1e-300)) return -1;                /* singular */

        if (piv != col) {
            double complex tmp[2 * METATRON_DIM];
            memcpy(tmp, aug[col], sizeof(tmp));
            memcpy(aug[col], aug[piv], sizeof(tmp));
            memcpy(aug[piv], tmp, sizeof(tmp));
        }

        double complex d = aug[col][col];
        for (int c = 0; c < 2 * n; c++) aug[col][c] /= d;

        for (int r = 0; r < n; r++) {
            if (r == col) continue;
            double complex f = aug[r][col];
            if (f != 0.0 + 0.0 * I) {
                for (int c = 0; c < 2 * n; c++) aug[r][c] -= f * aug[col][c];
            }
        }
    }

    for (int i = 0; i < n; i++)
        for (int j = 0; j < n; j++) inv[i][j] = aug[i][n + j];
    return 0;
}

/* --------------------------------------------------------------------------
 * KERNEL DO CUBO
 * -------------------------------------------------------------------------- */

static void metatron_build_hamiltonian(double complex H[METATRON_DIM][METATRON_DIM]) {
    /* H = I + A/g, g = grau = 12; A = adjacência do grafo completo K13. */
    for (int i = 0; i < METATRON_DIM; i++) {
        for (int j = 0; j < METATRON_DIM; j++) {
            H[i][j] = (i == j) ? 1.0 : (1.0 / METATRON_DEGREE);
        }
    }
}

static void metatron_cube_refresh(MetatronCube *cube) {
    double nrm = 0.0;
    for (int i = 0; i < METATRON_DIM; i++) nrm += cabs(cube->psi[i]) * cabs(cube->psi[i]);
    cube->norm = nrm;
    cube->handover = cabs(cube->psi[0]) * cabs(cube->psi[0]);
}

void metatron_cube_init(MetatronCube *cube, double dt, double gamma_b) {
    assert(cube != NULL);
    assert(dt > 0.0);
    assert(gamma_b > 0.0);

    double complex H[METATRON_DIM][METATRON_DIM];
    double complex tempA[METATRON_DIM][METATRON_DIM];   /* I - i*(dt/2)*H_eff */
    double complex tempB[METATRON_DIM][METATRON_DIM];   /* I + i*(dt/2)*H_eff */
    double complex invA[METATRON_DIM][METATRON_DIM];
    double complex U[METATRON_DIM][METATRON_DIM];

    metatron_build_hamiltonian(H);
    for (int i = 0; i < METATRON_DIM; i++) H[i][i] += gamma_b;   /* H_eff */

    double complex half_step = (dt / 2.0) * I;
    for (int i = 0; i < METATRON_DIM; i++) {
        for (int j = 0; j < METATRON_DIM; j++) {
            tempA[i][j] = (i == j) ? 1.0 : 0.0;
            tempB[i][j] = (i == j) ? 1.0 : 0.0;
            tempA[i][j] -= half_step * H[i][j];
            tempB[i][j] += half_step * H[i][j];
        }
    }

    int rc = cmat_inv(METATRON_DIM, tempA, invA);
    assert(rc == 0);
    cmat_mul(METATRON_DIM, invA, tempB, U);

    memset(cube->U, 0, sizeof(cube->U));
    memcpy(cube->U, U, sizeof(U));
    memset(cube->psi, 0, sizeof(cube->psi));
    cube->psi[0] = 1.0 + 0.0 * I;
    cube->dt = dt;
    cube->gamma_b = gamma_b;
    cube->steps = 0;
    metatron_cube_refresh(cube);
    cube->initialized = true;
}

/* Coloca um vetor unitário com |psi[0]|^2 = first_mag (para cenários de colapso). */
void metatron_place_unit_state(MetatronCube *cube, double first_mag) {
    assert(cube != NULL && cube->initialized);
    assert(first_mag >= 0.0 && first_mag <= 1.0);
    memset(cube->psi, 0, sizeof(cube->psi));
    cube->psi[0] = sqrt(first_mag) + 0.0 * I;
    cube->psi[1] = sqrt(1.0 - first_mag) + 0.0 * I;
    cube->steps = 0;
    metatron_cube_refresh(cube);
}

/* Evolução in-place por passos: psi <- U^k psi (uma multiplicacao por passo). */
void metatron_cube_evolve(MetatronCube *cube, uint32_t steps) {
    assert(cube != NULL && cube->initialized);
    double complex next[METATRON_DIM];

    for (uint32_t s = 0; s < steps; s++) {
        memset(next, 0, sizeof(next));
        for (int i = 0; i < METATRON_DIM; i++) {
            for (int j = 0; j < METATRON_DIM; j++) {
                next[i] += cube->U[i][j] * cube->psi[j];
            }
        }
        memcpy(cube->psi, next, sizeof(next));
    }

    cube->steps += steps;
    metatron_cube_refresh(cube);
}

double metatron_cube_handover(const MetatronCube *cube) {
    assert(cube != NULL);
    return cube->handover;
}

double metatron_cube_norm(const MetatronCube *cube) {
    assert(cube != NULL);
    return cube->norm;
}

uint32_t metatron_cube_steps(const MetatronCube *cube) {
    assert(cube != NULL);
    return cube->steps;
}

/* Costura REAL com o Observador: produz o CoherenceState de coherence.h. */
CoherenceState metatron_cube_to_coherence(const MetatronCube *cube) {
    assert(cube != NULL);
    double hv = metatron_cube_handover(cube);
    if (hv < 0.0) hv = 0.0;
    if (hv > 1.0) hv = 1.0;

    CoherenceState coh;
    coh.phi_c = hv;
    coh.phi_delta = 1.0 - hv;
    coh.ratio = hv;
    coh.entropy = 1.0 - hv;
    return coh;
}

/* --------------------------------------------------------------------------
 * TESTES UNITÁRIOS (validação AURIX/host) — inclui o observador ORIGINAL
 * para provar compatibilidade da costura no mesmo TU (guarda OBSERVADOR_UNIT_TEST
 * inexistente => sem conflito de main).
 * -------------------------------------------------------------------------- */

#ifdef METATRON_UNIT_TEST

#include "observador_primordial.c"

static double complex u_dagger_u[13][13];

static void test_unitarity(void) {
    MetatronCube cube;
    metatron_cube_init(&cube, 0.001, 255.0);

    double complex conjt[13][13];
    for (int i = 0; i < 13; i++)
        for (int j = 0; j < 13; j++) conjt[i][j] = conj(cube.U[j][i]);
    cmat_mul(13, conjt, cube.U, u_dagger_u);

    double worst = 0.0;
    for (int i = 0; i < 13; i++) {
        for (int j = 0; j < 13; j++) {
            double complex want = (i == j) ? 1.0 : 0.0;
            double err = cabs(u_dagger_u[i][j] - want);
            if (err > worst) worst = err;
        }
    }
    printf("[MET] unitarity_error(13) = %.3e (tol 1e-9)\n", worst);
    assert(worst < 1e-9);
    printf("✅ test_unitarity PASSED\n");
}

static void test_cayley_invert_roundtrip(void) {
    MetatronCube cube;
    metatron_cube_init(&cube, 0.001, 255.0);

    double complex H[13][13];
    metatron_build_hamiltonian(H);
    for (int i = 0; i < 13; i++) H[i][i] += cube.gamma_b;

    double complex tempA[13][13], invA[13][13], roundtrip[13][13];
    double complex half_step = (cube.dt / 2.0) * I;
    for (int i = 0; i < 13; i++)
        for (int j = 0; j < 13; j++) {
            tempA[i][j] = ((i == j) ? 1.0 : 0.0) - half_step * H[i][j];
        }

    int rc = cmat_inv(13, tempA, invA);
    assert(rc == 0);
    cmat_mul(13, invA, tempA, roundtrip);

    double worst = 0.0;
    for (int i = 0; i < 13; i++)
        for (int j = 0; j < 13; j++) {
            double complex want = (i == j) ? 1.0 : 0.0;
            double err = cabs(roundtrip[i][j] - want);
            if (err > worst) worst = err;
        }
    printf("[MET] inv(A)@A roundtrip error = %.3e (tol 1e-9)\n", worst);
    assert(worst < 1e-9);
    printf("✅ test_cayley_invert_roundtrip PASSED\n");
}

static void test_norm_conservation(void) {
    MetatronCube cube;
    metatron_cube_init(&cube, 0.001, 255.0);
    metatron_cube_evolve(&cube, 1000);
    printf("[MET] norm_after_1000_steps = %.15f\n", metatron_cube_norm(&cube));
    printf("[MET] handover_after_1000_steps = %.15f\n", metatron_cube_handover(&cube));
    assert(cube.steps == 1000);
    assert(fabs(metatron_cube_norm(&cube) - 1.0) < 1e-9);
    assert(metatron_cube_handover(&cube) >= 0.0 && metatron_cube_handover(&cube) <= 1.0);
    printf("✅ test_norm_conservation PASSED\n");
}

static void test_determinism(void) {
    MetatronCube a, b;
    metatron_cube_init(&a, 0.001, 255.0);
    metatron_cube_init(&b, 0.001, 255.0);
    metatron_cube_evolve(&a, 10);
    metatron_cube_evolve(&b, 10);
    assert(memcmp(a.psi, b.psi, sizeof a.psi) == 0);
    assert(memcmp(a.U, b.U, sizeof a.U) == 0);
    assert(a.handover == b.handover);
    assert(a.norm == b.norm);
    printf("✅ test_determinism PASSED\n");
}

static void test_placement_bounds(void) {
    MetatronCube cube;
    metatron_cube_init(&cube, 0.001, 255.0);
    metatron_place_unit_state(&cube, 0.2);
    assert(fabs(metatron_cube_handover(&cube) - 0.2) < 1e-12);
    assert(fabs(metatron_cube_norm(&cube) - 1.0) < 1e-12);
    printf("✅ test_placement_bounds PASSED\n");
}

static void test_observador_integration(void) {
    PrimordialState obs;
    ObservadorPrimordial_Init(&obs);

    /* Corte: cubo fresco, handover ~ 1 → Observador desperto. */
    MetatronCube cube;
    metatron_cube_init(&cube, 0.001, 255.0);
    CoherenceState coh = metatron_cube_to_coherence(&cube);
    assert(coh.phi_c == 1.0 && coh.ratio == 1.0);

    bool awake = ObservadorPrimordial_Observe(&obs, &coh, 0.0);
    assert(awake == true);
    assert(obs.observation_count == 1);
    assert(obs.awake_count == 1);
    assert(ObservadorPrimordial_IsAwake(&obs) == true);

    /* Colapso: handover 0.2 → transição para dormindo. */
    metatron_place_unit_state(&cube, 0.2);
    coh = metatron_cube_to_coherence(&cube);
    awake = ObservadorPrimordial_Observe(&obs, &coh, 10.0);
    assert(awake == false);
    assert(obs.asleep_count == 1);
    assert(obs.observation_count == 2);
    printf("✅ test_observador_integration PASSED\n");
}

int main(void) {
    printf("=== METATRON UNIT TESTS (K13, dt=0.001, gamma_b=255.0) ===\n");
    test_cayley_invert_roundtrip();
    test_unitarity();
    test_norm_conservation();
    test_determinism();
    test_placement_bounds();
    test_observador_integration();
    printf("=== ALL METATRON TESTS PASSED ===\n");
    return 0;
}

#endif /* METATRON_UNIT_TEST */