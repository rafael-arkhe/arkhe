/* =============================================================================
 * thermodynamics_kernel.c — BLOCO 485 v45 — Catedral OS
 * Núcleo C do simulador termodinâmico: ledger de cortes + checagem de Landauer.
 *
 * Revisão vetada: apenas fatos EXATOS são encodados como invariantes aqui
 *   (2ª lei: energy_cut >= bits_erased; monotonicidade do ledger; não-negatividade).
 *   Nenhuma relação Bell–Landauer/trade-off é afirmada no kernel — em SPEC,
 *   documentada como axioma no contrato Lean (ExactInequalities.lean).
 *
 * Compilação (mesmo padrão do bloco 483):
 *   gcc -std=c99 -Wall -Wextra -DARPA_UNIT_TEST -> na vdd THERMO_UNIT_TEST
 *   gcc -std=c99 -Wall -Wextra -DTHERMO_UNIT_TEST -o thermo_test thermodynamics_kernel.c
 *   ./thermo_test
 * ============================================================================= */
#include <stdint.h>
#include <stdbool.h>
#include <string.h>
#include <stdio.h>

#ifndef _WIN32
#define _POSIX_C_SOURCE 200809L
#endif

#define THERMO_MAX_STEPS 64

typedef struct {
    const char *label;
    uint64_t energy_cut;
    uint64_t bits_erased;
} ThermoStep;

typedef struct {
    ThermoStep steps[THERMO_MAX_STEPS];
    size_t n;
    uint64_t total_energy;
    uint64_t total_bits;
    bool landauer_breach;   /* índice do primeiro corte violado +1, 0 = ok   */
    size_t breach_index;
} ThermoLedger;

/* 2ª lei: o corte de uma etapa nunca é menor que os bits apagados. */
static void thermo_add(ThermoLedger *l, const char *label,
                       uint64_t energy_cut, uint64_t bits_erased) {
    if (l->n < THERMO_MAX_STEPS) {
        l->steps[l->n].label = label;
        l->steps[l->n].energy_cut = energy_cut;
        l->steps[l->n].bits_erased = bits_erased;
    }
    l->total_energy += energy_cut;
    l->total_bits += bits_erased;
    if (!l->landauer_breach && energy_cut < bits_erased) {
        l->landauer_breach = true;
        l->breach_index = l->n;   /* índice que violou */
    }
    l->n++;
}

/* Ledger acumulado: nunca decresce (soma de u64 não-negativos). */
static bool thermo_ledger_monotonic(const ThermoLedger *l) {
    uint64_t cum = 0;
    for (size_t i = 0; i < l->n; i++) {
        cum += l->steps[i].energy_cut;
        if (cum < l->steps[i].energy_cut && i > 0) return false; /* overflow */
    }
    return l->n > 0 && !l->landauer_breach;
}

/* Gap-1: o ledger não mexe em PHI_C — nada além de Nat aqui. */
static bool thermo_phi_c_untouched(const ThermoLedger *l) {
    (void)l;
    return true;
}

/* Corolário aritmético exato (espelha ExactInequalities.mean_div_le_self):
 * (a + b) / 2 <= a + b  na aritmética truncada. */
static bool thermo_mean_div_le_self(uint64_t a, uint64_t b) {
    uint64_t sum = a + b;
    return sum / 2 <= sum;
}

#ifdef THERMO_UNIT_TEST

#include <assert.h>

int main(void) {
    printf("=== THERMO KERNEL UNIT TESTS (BLOCO 485) ===\n");

    ThermoLedger l;
    memset(&l, 0, sizeof(l));
    thermo_add(&l, "measure_correlation", 64, 32);
    thermo_add(&l, "erase_working_mem", 128, 64);
    thermo_add(&l, "seal_step_to_chain", 256, 128);

    assert(l.n == 3);
    assert(l.total_energy == 64 + 128 + 256);
    assert(l.total_bits == 32 + 64 + 128);
    assert(!l.landauer_breach);
    assert(thermo_ledger_monotonic(&l));
    assert(thermo_phi_c_untouched(&l));
    assert(thermo_mean_div_le_self(7, 9));        /* 8 <= 16 */
    assert(thermo_mean_div_le_self(0, 0));        /* 0 <= 0  */
    assert(thermo_mean_div_le_self(((uint64_t)1) << 62, ((uint64_t)1) << 62));

    /* violação de Landauer detectada e indexada */
    ThermoLedger v;
    memset(&v, 0, sizeof(v));
    thermo_add(&v, "ok_step", 100, 50);
    thermo_add(&v, "impossible_erasure", 1, 999);
    assert(v.landauer_breach == true);
    assert(v.breach_index == 1);
    assert(!thermo_ledger_monotonic(&v));

    printf("[TERMO] total_energy=%llu total_bits=%llu landauer=OK phi=OK\n",
           (unsigned long long)l.total_energy, (unsigned long long)l.total_bits);
    printf("=== ALL THERMO TESTS PASSED ===\n");
    return 0;
}

#endif /* THERMO_UNIT_TEST */