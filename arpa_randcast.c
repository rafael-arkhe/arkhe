/* =============================================================================
 * arpa_randcast.c — BLOCO 484 v45 — Catedral OS
 * Consumo de aleatoriedade verificável (ARPA Randcast / BLS-TSS) via EVM.
 *
 * Revisão vetada (vs. proposta original):
 *   * Proposta assinava eth_sendTransaction com "data":"0x..." fictício (sempre
 *     reverteria on-chain) e dependia de json-c. Aqui o cliente C é READ-ONLY:
 *     monitora o evento RandomnessFulfilled via eth_getLogs (sem assinar, sem
 *     segredo na AURIX). A assinatura/requisição é feita pelo arpa_integration.py.
 *   * Aleatoriedade on-chain é uint256 (32 bytes) — NÃO bytes32 no ABI de evento
 *     (RandomnessFulfilled(bytes32 indexed, uint256, uint256)).
 *   * JSON construído/scaneado com o mesmo scanner leve e provado do bloco 483
 *     (sem json-c). Real = libcurl apenas; unidade = transporte fake.
 *
 * Modos (padrão bloco_483):
 *   gcc -std=c99 -Wall -Wextra -DARPA_UNIT_TEST arpa_randcast.c   (0 rede)
 *   gcc -std=c99 -DCLUSTER_REAL_API ...  ->  ARPA_REAL_API (requer -lcurl)
 *
 * Invariantes: Gap-1 (semente alimenta Zeno/QSP; Φ_C não é tocado),
 * Loopseal-2 (ledger on-chain), Eth-2 (somente endereço/requestId consultados).
 * ============================================================================= */
#ifndef _WIN32
#define _POSIX_C_SOURCE 200809L
#endif

#include <stdint.h>
#include <stdbool.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#define ARPA_RPC_URL_ENV         "ARPA_RPC_URL"
#define ARPA_CONSUMER_ADDR_ENV   "ARPA_CONSUMER_ADDRESS"
#define ARPA_FULFILLED_TOPIC0    "0x43bb0ea8184311848533a3793417198bab8a0c056ae1842abe24397adfc03b44"

typedef struct {
    const char *rpc_url;
    const char *consumer_address;
    uint8_t randomness[32];
    bool has_randomness;
    int   rpc_calls;
} ARPAState;

/* ==========================================================================
 * HELPERS
 * ========================================================================== */

static int arpa_hexval(char c) {
    if (c >= '0' && c <= '9') return (int)(c - '0');
    if (c >= 'a' && c <= 'f') return (int)(c - 'a') + 10;
    if (c >= 'A' && c <= 'F') return (int)(c - 'A') + 10;
    return -1;
}

static size_t arpa_hex_to_bytes(const char *hex, size_t max_bytes,
                                uint8_t *out, size_t out_sz) {
    if (!hex || hex[0] != '0' || (hex[1] != 'x' && hex[1] != 'X')) return 0;
    size_t i = 2, n = 0;
    while (hex[i] && n + 1 <= out_sz && n < max_bytes) {
        int hi = arpa_hexval(hex[i]);
        int lo = (hex[i + 1] != '\0') ? arpa_hexval(hex[i + 1]) : -1;
        if (hi < 0 || lo < 0) break;
        out[n++] = (uint8_t)((hi << 4) | lo);
        i += 2;
    }
    return n;
}

static uint64_t arpa_bytes_to_u64(const uint8_t b[8]) {
    uint64_t v = 0;
    for (int i = 0; i < 8; i++) v = (v << 8) | b[i];
    return v;
}

/* Resto de divisão 64-bit por divisor 32-bit (sem lib __int128 — AURIX seg). */
static uint64_t arpa_mod_u64(uint64_t v, uint64_t d) {
    while (v >= d) v -= d;
    return v;
}

/* scanner `"key":"value"` (mesmo scanner validado do bloco 483) */
static int json_scan_string(const char *body, const char *key, char *out, size_t out_size) {
    size_t key_len = strlen(key);
    const char *c = body;
    while ((c = strstr(c, key)) != NULL) {
        const char *before = c - 1;
        const char *after = c + key_len;
        if (before >= body && *before == '"' && *after == '"') {
            const char *sep = after + 1;
            while (*sep && (*sep == ' ' || *sep == '\t')) sep++;
            if (*sep == ':') {
                sep++;
                while (*sep && (*sep == ' ' || *sep == '\t')) sep++;
                if (*sep == '"') {
                    size_t n = 0;
                    sep++;
                    while (*sep && *sep != '"' && n + 1 < out_size) out[n++] = *sep++;
                    out[n] = '\0';
                    return 1;
                }
            }
        }
        c = after;
    }
    if (out_size > 0) out[0] = '\0';
    return 0;
}

/* ==========================================================================
 * TRANSPORTE
 * ========================================================================== */

static const char *ARPA_FAKE_LOG =
    "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":["
    "{\"address\":\"0x4242424242424242424242424242424242424242\","
    "\"topics\":[\"0x43bb0ea8184311848533a3793417198bab8a0c056ae1842abe24397adfc03b44\","
    "\"0xcacacacacacacacacacacacacacacacacacacacacacacacacacacacacacacaca\"],"
    "\"data\":\"0x00000104d814a6765bb19d2433869cb9346aa48762dff4a1b83e189382aa84f"
    "000000000000000000000000000000000000000000000000000000006875a360\","
    "\"blockNumber\":\"0x1\",\"transactionHash\":\"0xababababababababababababababab"
    "ababababababababababababababababab\",\"transactionIndex\":\"0x0\","
    "\"logIndex\":\"0x0\",\"blockHash\":\"0xcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"
    "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd\",\"removed\":false}]}";

#ifdef ARPA_REAL_API

#include <curl/curl.h>

static size_t arpa_write_cb(void *contents, size_t size, size_t nmemb, void *userp) {
    size_t total = size * nmemb;
    char **buffer = (char **)userp;
    size_t old = (*buffer) ? strlen(*buffer) : 0;
    char *tmp = (char *)realloc(*buffer, old + total + 1);
    if (!tmp) return 0;
    memcpy(tmp + old, contents, total);
    tmp[old + total] = '\0';
    *buffer = tmp;
    return total;
}

static int arpa_rpc(ARPAState *state, const char *body, char **out) {
    if (!state || !state->rpc_url) return 0;
    CURL *curl = curl_easy_init();
    if (!curl) return 0;
    *out = NULL;
    struct curl_slist *headers = NULL;
    headers = curl_slist_append(headers, "Content-Type: application/json");
    curl_easy_setopt(curl, CURLOPT_URL, state->rpc_url);
    curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body);
    curl_easy_setopt(curl, CURLOPT_HTTPHEADER, headers);
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, arpa_write_cb);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, out);
    curl_easy_setopt(curl, CURLOPT_TIMEOUT_MS, 30L * 1000L);
    CURLcode rc = curl_easy_perform(curl);
    curl_slist_free_all(headers);
    curl_easy_cleanup(curl);
    return rc == CURLE_OK && *out != NULL;
}

#else /* ARPA_UNIT_TEST (fake determinístico, 0 rede) */

static int arpa_rpc(ARPAState *state, const char *body, char **out) {
    (void)state; (void)body;
    *out = (char *)ARPA_FAKE_LOG; /* estática — não liberar */
    return 1;
}

#endif

/* ==========================================================================
 * POLL DA ALEATORIEDADE (eth_getLogs)
 * ========================================================================== */

static void arpa_build_getlogs_body(char *buf, size_t cap, const char *consumer,
                                    const char *request_topic) {
    snprintf(buf, cap,
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"eth_getLogs\",\"params\":["
        "{\"fromBlock\":\"0x0\",\"toBlock\":\"latest\","
        "\"address\":\"%s\",\"topics\":[\"%s\",\"%s\"]}]}",
        consumer ? consumer : "",
        ARPA_FULFILLED_TOPIC0,
        request_topic ? request_topic : "");
}

int arpa_poll_randomness(ARPAState *state, const char *request_id_hex) {
    if (!state) return 0;
    char body[1024];
    char topic[67];
    snprintf(topic, sizeof(topic), "0x%s",
             request_id_hex ? request_id_hex + (strncmp(request_id_hex, "0x", 2) == 0 ? 2 : 0)
                            : "0000000000000000000000000000000000000000000000000000000000000000");
    arpa_build_getlogs_body(body, sizeof(body), state->consumer_address, topic);

    char *resp = NULL;
    int ok = arpa_rpc(state, body, &resp);
    state->rpc_calls++;
    if (!ok || !resp) return 0;

    /* extrai "data":"0x<64hex>[timestamp 64hex]" — bytes 0..31 = randomness */
    char data[600];
    state->has_randomness = false;
    if (json_scan_string(resp, "data", data, sizeof(data))) {
        state->has_randomness =
            arpa_hex_to_bytes(data, 32, state->randomness, sizeof(state->randomness)) == 32;
    }
#ifdef ARPA_REAL_API
    if (resp) free(resp);
#endif
    return state->has_randomness;
}

/* ==========================================================================
 * DERIVAÇÃO DE SEMENTES (Gap-1: nunca altera Φ_C)
 * ========================================================================== */

double arpa_random_float(const ARPAState *state) {
    if (!state || !state->has_randomness) return -1.0;
    uint64_t top = arpa_bytes_to_u64(state->randomness);
    double f = (double)(top >> 11) / 9007199254740992.0; /* 53 bits, [0,1) */
    if (f >= 1.0) f = 0.9999999999999999;
    return f;
}

uint64_t arpa_random_int_in(const ARPAState *state, uint64_t lo, uint64_t hi) {
    if (!state || !state->has_randomness || hi < lo) return lo;
    uint64_t top = arpa_bytes_to_u64(state->randomness);
    uint64_t span = hi - lo + 1;
    return lo + arpa_mod_u64(top, span);
}

/* ==========================================================================
 * CICLO DE VIDA
 * ========================================================================== */

void ARPA_Init(ARPAState *state) {
    memset(state, 0, sizeof(*state));
    state->rpc_url = getenv(ARPA_RPC_URL_ENV);
    state->consumer_address = getenv(ARPA_CONSUMER_ADDR_ENV);
#ifndef ARPA_REAL_API
    (void)state;
#endif
}

#ifdef ARPA_UNIT_TEST

#include <assert.h>

int main(void) {
    printf("=== ARPA RANDCAST UNIT TESTS (BLOCO 484) ===\n");
    ARPAState st;
    ARPA_Init(&st);

    /* requestId fake = 0xcaca... (corresponde ao log fake) */
    const char *req = "0xcacacacacacacacacacacacacacacacacacacacacacacacacacacacacacacaca";
    int ok = arpa_poll_randomness(&st, req);
    assert(ok == 1);
    assert(st.has_randomness == true);
    assert(st.rpc_calls == 1);
    /* bytes 0..5 deve reproduzir o uint256 fake 0x00000104d814a6... */
    assert(st.randomness[0] == 0x00);
    assert(st.randomness[1] == 0x00);
    assert(st.randomness[2] == 0x01);
    assert(st.randomness[3] == 0x04);
    assert(st.randomness[4] == 0xd8);
    assert(st.randomness[5] == 0x14);

    double f = arpa_random_float(&st);
    assert(f >= 0.0 && f < 1.0);

    uint64_t seed = arpa_random_int_in(&st, 1000, 2000);
    assert(seed >= 1000 && seed <= 2000);

    /* derivações são determinísticas (mesma seed -> mesmos valores) */
    uint64_t seed2 = arpa_random_int_in(&st, 1000, 2000);
    assert(seed == seed2);
    double f2 = arpa_random_float(&st);
    assert(f == f2);

    printf("float=%.6f int=%llu bytes=%02x%02x%02x…\n",
           f, (unsigned long long)seed, st.randomness[0], st.randomness[1], st.randomness[2]);
    printf("=== ALL ARPA TESTS PASSED ===\n");
    return 0;
}

#endif /* ARPA_UNIT_TEST */