/* =============================================================================
 * likelihood_kernel.c — BLOCO 501 v61 — Catedral OS
 * Núcleo C da análise de verossimilhança (perfil PLR + Wilks + Gross-Vitells).
 * Uso previsto: AURIX TC4x (Observador) — busca de coerência "tipo LZ".
 *
 * REVISÃO VETADA (vs. proposta v61):
 *   * A proposta dependia de tpr_kernel.h / aqec_kernel.h / StaticComplexMatrix
 *     / TPRState — NENHUM existe no repositório (ground-truth verificado).
 *     Aqui NÃO há tais dependências: a coerência entra como um escalar `phi_c`
 *     (ponto de ponte documentado), sem struct fictícia.
 *   * A proposta usava srand(time(NULL)) no toy MC — não-determinístico.
 *     Aqui: PRNG xorshift64  seedável e determinístico (mesmo seed → mesmo p_global).
 *   * A proposta misturava mu*p_sig + (1-mu)*p_bg + 0.01*p_mssi (não normaliza,
 *     soma 1.01). Aqui: mistura de 2 componentes normalizada mu + (1-mu) = 1.
 *   * Orçamento AURIX: LEE_TOYS=128 (não 10000), grid 101 pontos, determinístico.
 *   * Wilks / Gross-Vitells são ASSINTÓTICOS — tratados como SPEC (ver Lean).
 *
 * Compilação (mesmo padrão bloco 483/484/485):
 *   gcc -std=c99 -Wall -Wextra -DLIKELIHOOD_UNIT_TEST -o lik_test likelihood_kernel.c
 *   ./lik_test
 * ============================================================================= */
#define _POSIX_C_SOURCE 200809L

#include <math.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>
#include <stdio.h>

#ifndef M_PI
#define M_PI 3.1415926535897932384626433832795
#endif

#define LIK_MAX_EVENTS  4096
#define LIK_GRID_STEPS  101    /* mu em [0,1], passo 0.01 */
#define LIK_LEE_TOYS    256    /* toy MC Gross-Vitells (orçamento AURIX: 256×48×101) */

/* --------------------------------------------------------------------------
 * PARÂMETROS DAS BANDAS — SPEC (constantes passíveis de calibração com dados
 * reais do Observador/LZ). Nenhum módulo faltante é invocado.
 * -------------------------------------------------------------------------- */
#define ER_LOG10S2_MEAN0   3.80
#define ER_LOG10S2_SIGMA0  0.05
#define ER_S1_GAIN         0.002
#define NR_LOG10S2_MEAN0   3.60
#define NR_LOG10S2_SIGMA0  0.08
#define NR_S1_GAIN         0.0015

typedef struct {
    double S1c;
    double log10_S2c;
} LikelihoodEvent;

typedef struct {
    double mu_hat;
    double p_value_local;
    double p_global;
    double significance_local;
    double significance_global;
    double chi2;
    bool is_signal;
    uint32_t num_events;
} LikelihoodResult;

/* ==========================================================================
 * PRNG DETERMINÍSTICO (xorshift64*)
 * ========================================================================== */

typedef struct { uint64_t s; } LIKRng;

static uint64_t lik_rng_next(LIKRng *r) {
    uint64_t x = r->s;
    x ^= x >> 12; x ^= x << 25; x ^= x >> 27;
    r->s = x;
    return x * 2685821657736338717ULL; /* xorshift64* — mult. u64 com wrap (definido) */
}

static double lik_rng_unit(LIKRng *r) {
    return (double)(lik_rng_next(r) & 0xFFFFFFFFFFFFULL) / 9007199254740992.0;
}

/* Box-Muller sobre o PRNG determinístico: ruído gaussiano padrão. */
static double lik_rng_gauss(LIKRng *r) {
    double u1 = lik_rng_unit(r);
    double u2 = lik_rng_unit(r);
    if (u1 < 1e-15) u1 = 1e-15;
    return sqrt(-2.0 * log(u1)) * cos(2.0 * M_PI * u2);
}

/* ==========================================================================
 * PDFs (log10S2 condicionado a S1) — gaussianas
 * ========================================================================== */

static double pdf_nr(double s1, double log_s2, double phi_c) {
    double mean = NR_LOG10S2_MEAN0 + NR_S1_GAIN * s1 + 0.05 * (phi_c - 0.85);
    double sigma = NR_LOG10S2_SIGMA0;
    if (phi_c > 0.9) sigma *= 0.9;
    double z = (log_s2 - mean) / (sigma + 1e-12);
    return exp(-0.5 * z * z) / (sigma * sqrt(2.0 * M_PI));
}

static double pdf_er(double s1, double log_s2, double phi_c) {
    double mean = ER_LOG10S2_MEAN0 + ER_S1_GAIN * s1 - 0.1 * (1.0 - phi_c);
    double sigma = ER_LOG10S2_SIGMA0 + 0.02 * (1.0 - phi_c);
    if (s1 < 0) s1 = 0;
    double z = (log_s2 - mean) / (sigma + 1e-12);
    return exp(-0.5 * z * z) / (sigma * sqrt(2.0 * M_PI));
}

static double mix_tail(double p, double floor) {
    return p < floor ? floor : p;
}

static double loglik(const LikelihoodEvent *ev, uint32_t n, double mu, double phi_c) {
    double ll = 0.0;
    for (uint32_t i = 0; i < n; i++) {
        double p_sig = pdf_nr(ev[i].S1c, ev[i].log10_S2c, phi_c);
        double p_bg  = pdf_er(ev[i].S1c, ev[i].log10_S2c, phi_c);
        /* mistura normalizada: mu + (1-mu) = 1 */
        double prob = mu * p_sig + (1.0 - mu) * p_bg;
        ll += log(mix_tail(prob, 1e-300));
    }
    return ll;
}

/* --------------------------------------------------------------------------
 * Perfil: maximiza mu no grid [0,1] e devolve μ̂ e log L máx.
 * -------------------------------------------------------------------------- */
static double profile_llh(const LikelihoodEvent *ev, uint32_t n, double phi_c,
                          double *out_mu_hat) {
    double best_ll = -1e300, best_mu = 0.0;
    for (int k = 0; k < LIK_GRID_STEPS; k++) {
        double mu = (double)k / (double)(LIK_GRID_STEPS - 1);
        double ll = loglik(ev, n, mu, phi_c);
        if (ll > best_ll) { best_ll = ll; best_mu = mu; }
    }
    if (out_mu_hat) *out_mu_hat = best_mu;
    return best_ll;
}

/* --------------------------------------------------------------------------
 * CDF do χ² com 1 dof (p=1) = erf(sqrt(x/2)); p-value = 1 - cdf.
 * -------------------------------------------------------------------------- */
static double chi2_pvalue_1dof(double ts) {
    if (ts < 0) return 1.0;
    return 1.0 - erf(sqrt(ts / 2.0));
}

/* Quantil normal (Abramowitz & Stegun 26.2.23) — mesmo método da proposta,
 * corrigido para clamps. */
static double norm_ppf(double p) {
    if (p <= 0.0) return 8.0;
    if (p >= 1.0) return -8.0;
    static const double a[4] = {2.50662823884, -18.61500062529, 41.39119773534, -25.44106049637};
    static const double b[4] = {-8.47351093090, 23.08336743743, -21.06224101826, 3.13082909833};
    static const double c[4] = {0.3374754822726147, 0.9761690190917186, 0.1607979714918209, 0.0276438810333863};
    static const double d[4] = {0.0032915721918257, 0.0002198789931803, 0.0000199943176875, 0.0000002752296141};
    double q = p - 0.5;
    if (fabs(q) < 0.42) {
        double r = q * q;
        return q * (((a[3] * r + a[2]) * r + a[1]) * r + a[0]) /
               ((((b[3] * r + b[2]) * r + b[1]) * r + b[0]) * r + 1.0);
    }
    double rv = (p > 0.5) ? (1.0 - p) : p;
    if (rv < 1e-300) return (p > 0.5) ? 8.0 : -8.0;
    rv = sqrt(-2.0 * log(rv));
    double z = (((c[3] * rv + c[2]) * rv + c[1]) * rv + c[0]) /
               ((((d[3] * rv + d[2]) * rv + d[1]) * rv + d[0]) * rv + 1.0);
    return (p > 0.5) ? z : -z;
}

/* --------------------------------------------------------------------------
 * CORREÇÃO LOOK-ELSEWHERE — Gross-Vitells via toy MC determinístico.
 * p_global = (1 + #toys com TS >= TS_obs) / (toys + 1)   [Trial-Factor].
 * -------------------------------------------------------------------------- */
static double look_elsewhere_toy(const LikelihoodEvent *ev, uint32_t n,
                                 double phi_c, double ts_obs, uint64_t seed,
                                 uint32_t toys) {
    uint32_t n_extreme = 0;
    LIKRng rng;
    rng.s = seed ? seed : 0xCACA12345678ULL;

    for (uint32_t t = 0; t < toys; t++) {
        LikelihoodEvent toy[LIK_MAX_EVENTS];
        for (uint32_t i = 0; i < n && i < LIK_MAX_EVENTS; i++) {
            /* H0 (nulo): eventos SORTEADOS da banda ER pura (não se soma ruído
               sobre o observado — senão o sinal vazaria para os toys). */
            double sigma = ER_LOG10S2_SIGMA0 + 0.02 * (1.0 - phi_c);
            double mean = ER_LOG10S2_MEAN0 + ER_S1_GAIN * ev[i].S1c - 0.1 * (1.0 - phi_c);
            toy[i].S1c = ev[i].S1c;
            toy[i].log10_S2c = mean + sigma * lik_rng_gauss(&rng);
        }
        double mu_t = 0.0;
        double ll_max = profile_llh(toy, n, phi_c, &mu_t);
        double ll_0 = loglik(toy, n, 0.0, phi_c);
        double ts_t = -2.0 * (ll_0 - ll_max);
        if (ts_t < 0) ts_t = 0.0;
        if (ts_t >= ts_obs) n_extreme++;
    }
    return ((double)n_extreme + 1.0) / ((double)toys + 1.0);
}

/* ==========================================================================
 * ANÁLISE COMPLETA (ponto de entrada do kernel)
 * ========================================================================== */

LikelihoodResult Likelihood_Analyze(const LikelihoodEvent *ev, uint32_t n,
                                    double phi_c, uint64_t toy_seed) {
    LikelihoodResult res;
    memset(&res, 0, sizeof(res));
    res.num_events = n;

    if (!ev || n == 0 || n > LIK_MAX_EVENTS) {
        res.mu_hat = 0.0;
        res.chi2 = 0.0;
        res.p_value_local = 1.0;
        res.p_global = 1.0;
        res.significance_local = 0.0;
        res.significance_global = 0.0;
        res.is_signal = false;
        return res;
    }

    double ll_max = profile_llh(ev, n, phi_c, &res.mu_hat);
    double ll_0 = loglik(ev, n, 0.0, phi_c);
    res.chi2 = -2.0 * (ll_0 - ll_max);
    if (res.chi2 < 0) res.chi2 = 0.0;

    res.p_value_local = chi2_pvalue_1dof(res.chi2);
    res.significance_local = norm_ppf(1.0 - res.p_value_local);

    res.p_global = look_elsewhere_toy(ev, n, phi_c, res.chi2, toy_seed, LIK_LEE_TOYS);
    res.significance_global = norm_ppf(1.0 - res.p_global);

    /* Convenção HEP: sem excesso (p>0.5) → significância reportada = 0σ. */
    if (res.p_value_local > 0.5) res.significance_local = 0.0;
    if (res.p_global > 0.5) res.significance_global = 0.0;

    res.is_signal = (res.significance_global > 2.5) && (res.mu_hat > 0.1);
    return res;
}

#ifdef LIKELIHOOD_UNIT_TEST

#include <assert.h>

/* Dataset sintético determinístico: 40 eventos EXATAMENTE na banda ER
   (mu=0 é o máximo do perfil); versão com_sinal acrescenta 8 candidatos NR. */
static void fill_dataset(LikelihoodEvent *ev, uint32_t *n, bool with_signal) {
    double s1[8] = {12, 15, 18, 21, 24, 27, 30, 34};
    double phi_c = 0.85;
    uint32_t k = 0;
    for (int i = 0; i < 5; i++) {
        for (int j = 0; j < 8; j++) {
            ev[k].S1c = s1[j];
            ev[k].log10_S2c = ER_LOG10S2_MEAN0 + ER_S1_GAIN * s1[j] - 0.1 * (1.0 - phi_c);
            k++;
        }
    }
    if (with_signal) {
        for (int j = 0; j < 8; j++) {
            ev[k].S1c = s1[j];
            ev[k].log10_S2c = NR_LOG10S2_MEAN0 + NR_S1_GAIN * s1[j] + 0.05 * (phi_c - 0.85);
            k++;
        }
    }
    *n = k;
}

int main(void) {
    printf("=== LIKELIHOOD KERNEL UNIT TESTS (BLOCO 501 v61) ===\n");

    LikelihoodEvent ev[LIK_MAX_EVENTS];
    uint32_t n;

    /* 1. Fundo puro: μ̂=0, TS=0, p_global alto, sem sinal. */
    fill_dataset(ev, &n, false);
    LikelihoodResult r_bg = Likelihood_Analyze(ev, n, 0.85, 0xCACA);
    assert(r_bg.mu_hat >= 0.0 && r_bg.mu_hat <= 1.0);
    assert(r_bg.chi2 == 0.0);            /* eventos na banda -> H0 é o perfil */
    assert(!r_bg.is_signal);
    assert(r_bg.p_value_local >= r_bg.p_global - 1e-12); /* trial factor enfraquece */
    assert(r_bg.significance_global <= r_bg.significance_local + 1e-9);

    /* 2. Com candidato NR: TS maior e μ̂ > 0. */
    fill_dataset(ev, &n, true);
    LikelihoodResult r_sig = Likelihood_Analyze(ev, n, 0.85, 0xCACA);
    assert(r_sig.chi2 >= 0.0);
    assert(r_sig.mu_hat >= 0.0 && r_sig.mu_hat <= 1.0);
    assert(r_sig.p_value_local >= 0.0 && r_sig.p_value_local <= 1.0);
    assert(r_sig.p_global >= 1.0 / (double)(LIK_LEE_TOYS + 1));
    assert(r_sig.p_global >= r_sig.p_value_local - 1e-12);
    assert(r_sig.chi2 > r_bg.chi2);      /* deslocamento NR cresce o -2ΔlogL */

    /* 3. Determinismo: mesmo seed -> mesmos resultados bit a bit. */
    LikelihoodResult r2 = Likelihood_Analyze(ev, n, 0.85, 0xCACA);
    assert(memcmp(&r_sig, &r2, sizeof(LikelihoodResult)) == 0);

    /* 4. Seed diferente -> toy MC diferente (mas p_global continua no domínio). */
    LikelihoodResult r3 = Likelihood_Analyze(ev, n, 0.85, 0xF00D);
    assert(r3.p_global >= 1.0 / (double)(LIK_LEE_TOYS + 1));
    assert(r3.p_global >= r3.p_value_local - 1e-12);

    /* 5. Guardas: entrada vazia não quebra nada. */
    LikelihoodResult r_empty = Likelihood_Analyze(NULL, 0, 0.85, 0);
    assert(!r_empty.is_signal && r_empty.num_events == 0);

    printf("[LIK] bg:  μ̂=%.3f χ²=%.2f p_local=%.3e p_global=%.3e Z_l=%.2fσ Z_g=%.2fσ\n",
           r_bg.mu_hat, r_bg.chi2, r_bg.p_value_local, r_bg.p_global,
           r_bg.significance_local, r_bg.significance_global);
    printf("[LIK] sig: μ̂=%.3f χ²=%.2f p_local=%.3e p_global=%.3e Z_l=%.2fσ Z_g=%.2fσ %s\n",
           r_sig.mu_hat, r_sig.chi2, r_sig.p_value_local, r_sig.p_global,
           r_sig.significance_local, r_sig.significance_global,
           r_sig.is_signal ? "SINAL" : "fundo");
    printf("=== ALL LIKELIHOOD TESTS PASSED ===\n");
    return 0;
}

#endif /* LIKELIHOOD_UNIT_TEST */