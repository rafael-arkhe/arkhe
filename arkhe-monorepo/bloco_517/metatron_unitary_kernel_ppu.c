/*
 * metatron_unitary_kernel_ppu.c — v77 VETADO — BLOCO 517
 *
 * Inversao 13x13 via decomposicao LU com pivo parcial determinístico.
 * O algoritmo preserva o contrato do BLOCO 507: unidade de norma e
 * handover |psi[0]|^2 / ||psi||^2 como os únicos invariantes mensuraveis.
 *
 * Vetagem:
 *  - PPU_SIMD abaixo sao DICAS de compilacao (pragmas), nao alegação de
 *    latencia (<10 us) — impossivel de verificar neste repositorio, mesmo
 *    erro rejeitado no BLOCO 507 (M1).
 *  - Pivo parcial determinístico: selecao por modulo |U[i][k]| (estavel,
 *    sem aleatoriedade de hardware).
 *  - A inversa e verificada por residuo: max|inv(A)@A - I|.
 *
 * Compilacao (fora do repo, com gcc): gcc -std=c99 -Wall -Wextra -DMETATRON_UNIT_TEST -o met_ppu metatron_unitary_kernel_ppu.c -lm
 */

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include <math.h>
#include <complex.h>

#define METATRON_N 13

#ifdef __TASKING__
#define PPU_SIMD _Pragma("simd")
#elif defined(__GNUC__)
#define PPU_SIMD _Pragma("GCC ivdep")
#else
#define PPU_SIMD
#endif

typedef struct {
    double complex psi[METATRON_N];
} DSPR_MetatronPsi;

/* Decomposicao LU com pivo parcial deterministico (retorna false se singular). */
static bool metatron_lu_decompose(const double complex A[METATRON_N][METATRON_N],
                                  double complex L[METATRON_N][METATRON_N],
                                  double complex U[METATRON_N][METATRON_N],
                                  int pivot[METATRON_N]) {
    for (int i = 0; i < METATRON_N; i++) {
        pivot[i] = i;
        for (int j = 0; j < METATRON_N; j++) {
            L[i][j] = 0.0;
            U[i][j] = A[i][j];
        }
        L[i][i] = 1.0;
    }
    for (int k = 0; k < METATRON_N; k++) {
        /* pivo parcial: maior |U[i][k]| entre i>=k */
        double best = 0.0;
        int best_row = k;
        PPU_SIMD
        for (int i = k; i < METATRON_N; i++) {
            double mag = cabs(U[i][k]);
            if (mag > best) { best = mag; best_row = i; }
        }
        if (best < 1e-300) return false;                  /* singular */
        if (best_row != k) {                              /* troca de linhas */
            for (int j = 0; j < METATRON_N; j++) {
                double complex t = U[k][j]; U[k][j] = U[best_row][j]; U[best_row][j] = t;
            }
            int tp = pivot[k]; pivot[k] = pivot[best_row]; pivot[best_row] = tp;
        }
        const double complex uk_inv = 1.0 / U[k][k];
        PPU_SIMD
        for (int i = k + 1; i < METATRON_N; i++) {
            L[i][k] = U[i][k] * uk_inv;
            for (int j = k; j < METATRON_N; j++) {
                U[i][j] -= L[i][k] * U[k][j];
            }
        }
    }
    return true;
}

/* Inversa via LU: resolve A X = I. Retorna false se A for singular. */
bool metatron_matrix_inverse_ppu(const double complex A[METATRON_N][METATRON_N],
                                 double complex invA[METATRON_N][METATRON_N]) {
    double complex L[METATRON_N][METATRON_N];
    double complex U[METATRON_N][METATRON_N];
    int pivot[METATRON_N];
    if (!metatron_lu_decompose(A, L, U, pivot)) return false;

    /* colunas da identidade permutada */
    for (int jcol = 0; jcol < METATRON_N; jcol++) {
        double complex rhs[METATRON_N];
        double complex y[METATRON_N];
        for (int i = 0; i < METATRON_N; i++) rhs[i] = (pivot[i] == jcol) ? 1.0 : 0.0;
        /* forward substitution: L y = rhs */
        for (int i = 0; i < METATRON_N; i++) {
            double complex s = rhs[i];
            for (int k = 0; k < i; k++) s -= L[i][k] * y[k];
            y[i] = s;
        }
        /* backward substitution: U x = y */
        for (int i = METATRON_N - 1; i >= 0; i--) {
            double complex s = y[i];
            for (int k = i + 1; k < METATRON_N; k++) s -= U[i][k] * invA[k][jcol];
            invA[i][jcol] = s / U[i][i];
        }
    }
    return true;
}

/* handover = |psi[0]|^2 / ||psi||^2, em [0,1] se a norma for preservada. */
double metatron_handover(const double complex psi[METATRON_N]) {
    double nrm = 0.0;
    for (int i = 0; i < METATRON_N; i++) nrm += (creal(psi[i]) * creal(psi[i]) + cimag(psi[i]) * cimag(psi[i]));
    if (!(nrm > 0.0)) return 0.0;
    double p0 = creal(psi[0]) * creal(psi[0]) + cimag(psi[0]) * cimag(psi[0]);
    return p0 / nrm;
}

#ifdef METATRON_UNIT_TEST

#include <stdio.h>

/* residuo max|inv(A)@A - I| */
static double roundtrip_error(const double complex A[METATRON_N][METATRON_N],
                              const double complex invA[METATRON_N][METATRON_N]) {
    double worst = 0.0;
    for (int i = 0; i < METATRON_N; i++) {
        for (int j = 0; j < METATRON_N; j++) {
            double complex s = -((i == j) ? 1.0 : 0.0);
            for (int k = 0; k < METATRON_N; k++) s += invA[i][k] * A[k][j];
            double m = cabs(s);
            if (m > worst) worst = m;
        }
    }
    return worst;
}

/* A = M^H M + I  (positiva-definida, hermitiana), M determinística derivada do indice. */
static void build_pd_matrix(double complex A[METATRON_N][METATRON_N]) {
    double complex M[METATRON_N][METATRON_N];
    for (int i = 0; i < METATRON_N; i++) {
        for (int j = 0; j < METATRON_N; j++) {
            double re = 0.25 * ((i * 13 + j) % 7) - 1.5;
            double im = 0.20 * ((j * 13 + i) % 5) - 0.8;
            M[i][j] = re + im * I;
        }
    }
    for (int i = 0; i < METATRON_N; i++) {
        for (int j = 0; j < METATRON_N; j++) {
            double complex s = (i == j) ? 1.0 : 0.0;
            for (int k = 0; k < METATRON_N; k++) s += conj(M[k][i]) * M[k][j];
            A[i][j] = s;
        }
    }
}

int main(void) {
    double complex A[METATRON_N][METATRON_N];
    double complex invA[METATRON_N][METATRON_N];
    double complex psi[METATRON_N];
    int fails = 0;

    build_pd_matrix(A);
    if (!metatron_matrix_inverse_ppu(A, invA)) {
        printf("[MET] FAIL: inversao acusou singularidade\n");
        return 1;
    }
    double err = roundtrip_error(A, invA);
    if (err > 1e-12) fails++;
    printf("[MET] unitarity_error(13)            = %.3g   (inv(A)@A - I)\n", err);
    printf("[MET] inv(A)@A roundtrip error       = %.3g\n", err);

    for (int i = 0; i < METATRON_N; i++) {
        double re = (i * 37) % 101 - 50;
        double im = (i * 11) % 71 - 35;
        psi[i] = (re + im * I) / 137.0;
    }
    double h = metatron_handover(psi);
    if (h < 0.0 || h > 1.0) fails++;
    printf("[MET] handover(psi)                  = %.6f   (∈ [0,1])\n", h);

    /* psi normalizado: norma deve ser 1 e handover = |psi[0]|^2 */
    double nrm = 0.0;
    for (int i = 0; i < METATRON_N; i++) nrm += cabs(psi[i]) * cabs(psi[i]);
    for (int i = 0; i < METATRON_N; i++) psi[i] /= sqrt(nrm);
    double p0 = cabs(psi[0]) * cabs(psi[0]);
    double h2 = metatron_handover(psi);
    if (fabs(h2 - p0) > 1e-12) fails++;
    printf("[MET] handover(normalized)           = %.6f   (|psi0|^2 = %.6f)\n", h2, p0);

    printf("[MET] %s\n", fails == 0 ? "ALL CHECKS PASS" : "CHECKS FAILED");
    return fails;
}

#endif /* METATRON_UNIT_TEST */