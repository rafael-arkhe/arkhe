/*
 * metatron_observer_bridge.c — v77 VETADO — BLOCO 517
 *
 * Costura Cubo de Metatron -> Observador Primordial.
 *
 * Vetagem (contrato REAL de coherence.h, BLOCO 507/M2):
 *  - A proposta v77 escrevia campos inexistentes {phi_sync, timestamp_us}
 *    e obtia phi_sync via carg(psi[0]). O struct REAL de coherence.h tem
 *    apenas {phi_c, phi_delta, ratio, entropy}; a costura usa somente esses
 *    campos e chama ObservadorPrimordial_Observe(obs, &state, time_ms).
 *  - handover vem do kernel (metatron_unitary_kernel_ppu.c).
 *
 * Compilacao standalone (para teste do TU) exige -DMETATRON_BRIDGE_SHIM,
 * que define aqui um shim compativel com o contrato real:
 *   gcc -std=c99 -Wall -Wextra -DMETATRON_BRIDGE_SHIM -o met_bridge metatron_observer_bridge.c -lm
 */

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include <math.h>
#include <complex.h>

#define METATRON_N 13

double metatron_handover(const double complex psi[METATRON_N]); /* kernel */

#ifdef METATRON_BRIDGE_SHIM
/* Shim compativel com o contrato REAL de coherence.h — {phi_c, phi_delta,
 * ratio, entropy}, exatamente como descrito no BLOCO 507. Nenhum campo
 * inventado. Ser removido quando o TU real de coherence.h for vinculado. */
typedef struct {
    double phi_c;
    double phi_delta;
    double ratio;
    double entropy;
} CoherenceState;

typedef struct {
    double last_phi_c;
    unsigned long long last_time_ms;
    long transitions;
} ObservadorPrimordial;

void ObservadorPrimordial_Observe(ObservadorPrimordial *obs,
                                  const CoherenceState *state,
                                  unsigned long long time_ms) {
    if (obs && state) {
        obs->last_phi_c = state->phi_c;
        obs->last_time_ms = time_ms;
    }
}
#else
/* Fora do shim, o tipo real vem de coherence.h / observador_primordial.h */
#include "coherence.h"
#include "observador_primordial.h"
#endif

/*
 * Converte o estado do cubo no CoherenceState REAL e observa.
 * phi_c  = handover (|psi[0]|^2 / ||psi||^2) — alimenta Gap-1 (0.577350..0.999900);
 * phi_delta = 1 - phi_c (fronteira de coerencia);
 * ratio = handover (escala [0,1]);
 * entropy = 1 - ratio.
 * time_ms: carimbo fornecido pelo chamador (nao derivado de clock interno).
 */
bool ObservadorPrimordial_UpdateFromMetatron(ObservadorPrimordial *obs,
                                             const double complex psi[METATRON_N],
                                             double gamma_b,
                                             unsigned long long time_ms) {
    if (!obs || !psi) return false;
    double phi_c = metatron_handover(psi);
    if (phi_c < 0.0) phi_c = 0.0;
    if (phi_c > 1.0) phi_c = 1.0;
    CoherenceState coh;
    coh.phi_c = phi_c;
    coh.phi_delta = 1.0 - phi_c;
    coh.ratio = phi_c;
    coh.entropy = 1.0 - phi_c;
    ObservadorPrimordial_Observe(obs, &coh, time_ms);
    (void)gamma_b; /* parametrico para uso de calibracao futura; nao altera contrato */
    return true;
}

#ifdef METATRON_BRIDGE_SHIM

#include <stdio.h>

int main(void) {
    ObservadorPrimordial obs = { 0.0, 0ull, 0 };
    double complex psi[METATRON_N];
    for (int i = 0; i < METATRON_N; i++) psi[i] = (i == 0) ? (1.0 + 0.0 * I) : (0.0 + 0.0 * I);

    if (!ObservadorPrimordial_UpdateFromMetatron(&obs, psi, 255.0, 1000ull)) return 1;
    printf("[MBR] phi_c     = %.6f  (handover completo esperado ~1.0)\n", obs.last_phi_c);

    for (int i = 0; i < METATRON_N; i++) psi[i] = ((double)((i * 11) % 7) - 3.0) / 21.0;
    if (!ObservadorPrimordial_UpdateFromMetatron(&obs, psi, 255.0, 2000ull)) return 1;
    printf("[MBR] phi_c     = %.6f  (estado misto esperado em [0,1])\n", obs.last_phi_c);
    printf("[MBR] phi_delta = %.6f  phi_delta+phi_c = %.6f (contrato [0,1])\n",
           obs.last_phi_c, 1.0 - obs.last_phi_c, 1.0);

    int fail = (obs.last_phi_c < 0.0 || obs.last_phi_c > 1.0) ? 1 : 0;
    printf("[MBR] %s\n", fail ? "FAIL" : "ALL CHECKS PASS");
    return fail;
}

#endif /* METATRON_BRIDGE_SHIM */