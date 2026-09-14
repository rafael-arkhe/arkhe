/*
 * cluster_client.c — Cliente C para Cluster Protocol API (BLOCO 483 — v44)
 * ========================================================================
 * API descentralizada de inferência de IA (OpenAI-compatible, x402,
 * roteamento por provedor: venice/phala/zerog/groq/...).
 *
 * Integração com a Catedral OS:
 *   - Observador Primordial  -> explicações de anomalias de coerência
 *   - Zeno                   -> sugestões de correção (veto)
 *   - Litografia Quântica    -> interpretação de comandos físicos
 *
 * Plataforma alvo: AURIX TC4x (Infineon), mas compila em qualquer C99.
 * Dependência (modo real, CLUSTER_REAL_API): apenas libcurl (-lcurl).
 * Modo unidade (CLUSTER_UNIT_TEST): SEM libcurl (transporte fake
 * determinístico), seguindo o padrão de observador_primordial.c.
 *
 * Protocolo: SASC-EXTERNAL-INFERENCE-2026
 * Selo: CLUSTER-CATEDRAL-2026-09-01
 */

#ifndef _WIN32
#define _POSIX_C_SOURCE 200809L  /* nanosleep (glibc, -std=c99) */
#endif

#include <stdint.h>
#include <stdbool.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#ifdef _WIN32
#include <windows.h>
#endif

/* ==========================================================================
 * CONFIGURAÇÃO
 * ========================================================================== */

#define CLUSTER_BASE_URL        "https://api.clusterprotocol.ai"
#define CLUSTER_API_KEY_ENV     "CLUSTER_API_KEY"
#define CLUSTER_DEFAULT_MODEL   "llama-3.3-70b-instruct"
#define CLUSTER_DEFAULT_PROVIDER "venice"
#define CLUSTER_TIMEOUT_MS      30000
#define CLUSTER_MAX_RETRIES     3
#define CLUSTER_MAX_EXPLANATION 512

/* ==========================================================================
 * ESTRUTURAS
 * ========================================================================== */

typedef struct {
    const char *api_key;          /* Bearer token (ou NULL p/ x402) */
    const char *model;            /* modelo default */
    const char *provider;         /* venice | phala | zerog | groq | ... */
    uint32_t    timeout_ms;
    uint32_t    max_retries;
    bool        use_x402;         /* pagamento por requisição (USDC/Base) */
} ClusterConfig;

typedef struct {
    const char *role;             /* "system" | "user" | "assistant" */
    const char *content;
} ChatMessage;

typedef struct {
    ChatMessage *messages;
    size_t       count;
    double       temperature;
    int          max_tokens;
    bool         stream;
} ChatRequest;

typedef struct {
    char   content[4096];
    int    prompt_tokens;
    int    completion_tokens;
    int    total_tokens;
    bool   ok;                    /* parse bem-sucedido */
} ChatResponse;

/* Estado local para o adaptador do Observador (reentrante) */
typedef struct {
    char   last_explanation[CLUSTER_MAX_EXPLANATION];
    double last_phi_c;
    double last_ratio;
    uint32_t explain_count;
} ClusterObservadorContext;

/* ==========================================================================
 * HELPERS (portáveis)
 * ========================================================================== */

#ifdef CLUSTER_REAL_API
static void *cluster_realloc(void *ptr, size_t size) {
    void *p = realloc(ptr, size);
    return p;
}
#endif /* CLUSTER_REAL_API */

static char *cluster_strdup(const char *src) {
    if (!src) return NULL;
    size_t len = strlen(src);
    char *out = (char *)malloc(len + 1);
    if (!out) return NULL;
    memcpy(out, src, len + 1);
    return out;
}

/* Sleep portável (segundos) — AURIX/GCC, Linux, macOS e Windows. */
static void cluster_sleep_sec(uint32_t seconds) {
#if defined(_WIN32)
    Sleep(seconds * 1000u);
#else
    struct timespec ts = { .tv_sec = seconds, .tv_nsec = 0 };
    nanosleep(&ts, NULL);
#endif
}

#ifdef CLUSTER_REAL_API
static size_t write_cb(void *contents, size_t size, size_t nmemb, void *userp) {
    size_t total = size * nmemb;
    char **buffer = (char **)userp;
    size_t old = (*buffer) ? strlen(*buffer) : 0;
    char *tmp = (char *)cluster_realloc(*buffer, old + total + 1);
    if (!tmp) return 0;
    memcpy(tmp + old, contents, total);
    tmp[old + total] = '\0';
    *buffer = tmp;
    return total;
}
#endif /* CLUSTER_REAL_API */

/* Procura `"key":"<valor>"` (strings) num corpo JSON simples. */
static int json_scan_string(const char *body, const char *key, char *out, size_t out_size) {
    /* formato esperado do payload Cluster: `"key":"value"` */
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

/* Procura `"key":<número>` (usage) — valor numérico sem aspas. */
static int json_scan_int(const char *body, const char *key, int *out) {
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
                if (*sep >= '0' && *sep <= '9') {
                    char tmp[64];
                    size_t n = 0;
                    while (*sep && *sep >= '0' && *sep <= '9' && n + 1 < sizeof(tmp)) tmp[n++] = *sep++;
                    tmp[n] = '\0';
                    *out = (int)strtol(tmp, NULL, 10);
                    return 1;
                }
            }
        }
        c = after;
    }
    return 0;
}

/* ==========================================================================
 * CONSTRUÇÃO DO PAYLOAD JSON
 * ========================================================================== */

static char *cluster_build_json(const ClusterConfig *config, const ChatRequest *req) {
    /* buffer maior o suficiente para messages razoáveis (BLOCO 483) */
    size_t cap = 4096;
    for (size_t i = 0; i < req->count; i++) {
        cap += (req->messages[i].content ? strlen(req->messages[i].content) : 0)
             + (req->messages[i].role ? strlen(req->messages[i].role) : 0)
             + 64;
    }
    char *buf = (char *)malloc(cap);
    if (!buf) return NULL;

    /* Para streaming/full: `"stream": false`; pay-per-token é $0.003. */
    size_t used = 0;
    used += (size_t)snprintf(buf + used, cap - used,
        "{\"model\":\"%s\",\"provider\":\"%s\",\"temperature\":%.2f,"
        "\"max_tokens\":%d,\"stream\":%s,\"messages\":[",
        config->model ? config->model : CLUSTER_DEFAULT_MODEL,
        config->provider ? config->provider : CLUSTER_DEFAULT_PROVIDER,
        req->temperature,
        req->max_tokens,
        req->stream ? "true" : "false");

    for (size_t i = 0; i < req->count; i++) {
        const char *role = req->messages[i].role ? req->messages[i].role : "user";
        const char *content = req->messages[i].content ? req->messages[i].content : "";
        int n = snprintf(buf + used, cap - used,
                         "%s{\"role\":\"%s\",\"content\":\"%s\"}",
                         (i > 0) ? "," : "", role, content);
        if (n < 0 || used + (size_t)n >= cap) break;
        used += (size_t)n;
    }
    snprintf(buf + used, cap - used, "]}");
    return buf;
}

/* ==========================================================================
 * PARSE DA RESPOSTA
 * ========================================================================== */

static void cluster_parse_response(const char *body, ChatResponse *resp) {
    if (!body || !resp) return;
    memset(resp, 0, sizeof(*resp));
    json_scan_string(body, "content", resp->content, sizeof(resp->content));
    json_scan_int(body, "prompt_tokens", &resp->prompt_tokens);
    json_scan_int(body, "completion_tokens", &resp->completion_tokens);
    json_scan_int(body, "total_tokens", &resp->total_tokens);
    resp->ok = (resp->content[0] != '\0');
}

/* ==========================================================================
 * TRANSPORTE
 * ========================================================================== */
/*
 * Dois modos:
 *  - CLUSTER_REAL_API:  POST real via libcurl (requer -lcurl).
 *  - default (teste):   transporte fake determinístico (0 rede).
 */
#ifdef CLUSTER_REAL_API

#include <curl/curl.h>

static int cluster_post(const ClusterConfig *config, const char *url,
                        const char *payload, /*out*/ char **response_body) {
    CURL *curl = curl_easy_init();
    if (!curl) return -1;
    struct curl_slist *headers = NULL;
    headers = curl_slist_append(headers, "Content-Type: application/json");
    if (config->api_key) {
        char auth[384];
        snprintf(auth, sizeof(auth), "Authorization: Bearer %s", config->api_key);
        headers = curl_slist_append(headers, auth);
    }

    curl_easy_setopt(curl, CURLOPT_URL, url);
    curl_easy_setopt(curl, CURLOPT_POSTFIELDS, payload);
    curl_easy_setopt(curl, CURLOPT_HTTPHEADER, headers);
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_cb);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, response_body);
    curl_easy_setopt(curl, CURLOPT_TIMEOUT_MS, config->timeout_ms);
    curl_easy_setopt(curl, CURLOPT_SSL_VERIFYPEER, 1L);

    CURLcode res = curl_easy_perform(curl);
    long http_code = 0;
    curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &http_code);
    curl_easy_cleanup(curl);
    curl_slist_free_all(headers);
    if (res != CURLE_OK) return -1;
    return (int)http_code;
}

void Cluster_Init(void)    { curl_global_init(CURL_GLOBAL_DEFAULT); }
void Cluster_Shutdown(void){ curl_global_cleanup(); }

#else  /* CLUSTER_REAL_API */

/* transporte fake determinístico para testes de unidade (sem rede) */
static int cluster_post(const ClusterConfig *config, const char *url,
                        const char *payload, /*out*/ char **response_body) {
    (void)config; (void)url; (void)payload;
    static const char fake[] =
        "{\"id\":\"chatcmpl-fake483\",\"model\":\"llama-3.3-70b-instruct\","
        "\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":"
        "\"OK catedral:\\u03a6C observado. gamma_B = 1.20 E0 = 0.18 eta = 0.10\"}}],"
        "\"usage\":{\"prompt_tokens\":42,\"completion_tokens\":18,\"total_tokens\":60}}";
    *response_body = cluster_strdup(fake);
    return 200;
}

void Cluster_Init(void)    { /* sem estado em modo teste */ }
void Cluster_Shutdown(void){ /* sem estado em modo teste */ }

#endif /* CLUSTER_REAL_API */

/* ==========================================================================
 * CHAT COMPLETION (API principal)
 * ========================================================================== */

ChatResponse *cluster_chat_completion(const ClusterConfig *config, const ChatRequest *req) {
    ChatResponse *resp = (ChatResponse *)calloc(1, sizeof(ChatResponse));
    if (!resp) return NULL;

    char *payload = cluster_build_json(config, req);
    if (!payload) { free(resp); return NULL; }

    char url[512];
    snprintf(url, sizeof(url), "%s/v1/chat/completions", CLUSTER_BASE_URL);

    char *body = NULL;
    int http = -1;
    for (uint32_t attempt = 0; attempt < config->max_retries; attempt++) {
        body = NULL;
        http = cluster_post(config, url, payload, &body);
        if (http == 200) break;
        if (http == 429) { /* rate limit: backoff simples */
            cluster_sleep_sec(1u);
            continue;
        }
        break; /* 4xx/5xx definitivo; propaga abaixo */
    }

    if (http == 200 && body) {
        cluster_parse_response(body, resp);
    } else {
        /* 402 (x402/sem saldo) ou falha 5xx: resposta vazia + sinalização */
        resp->ok = false;
    }

    if (body) free(body);
    free(payload);
    return resp;
}

const char *cluster_extract_content(const ChatResponse *resp) {
    if (!resp || !resp->ok) return "";
    return resp->content;
}

/* ==========================================================================
 * ADAPTADOR PARA O OBSERVADOR PRIMORDIAL
 * ========================================================================== */
/*
 * Nota de integração (invariante Gap-1): a struct PrimordialState de
 * observador_primordial.c NÃO é alterada (layout fixo AURIX). O contexto de
 * explicação vive no ClusterObservadorContext, reentrante por instância.
 */
void Cluster_ObservadorInit(ClusterObservadorContext *ctx) {
    if (!ctx) return;
    memset(ctx, 0, sizeof(*ctx));
}

void Cluster_ObservadorExplainAnomaly(ClusterObservadorContext *ctx,
                                      const ClusterConfig *config,
                                      double phi_c, double ratio,
                                      const char *context) {
    if (!ctx || !config) return;
    ctx->last_phi_c = phi_c;
    ctx->last_ratio = ratio;
    ctx->explain_count++;

    ChatMessage messages[2] = {
        { .role = "system",
          .content = "Voce e o Observador Primordial da Catedral OS. "
                     "Explique anomalias de coerencia com concisao." },
        { .role = "user", .content = context ? context : "" }
    };
    ChatRequest req = {
        .messages = messages, .count = 2,
        .temperature = 0.30, .max_tokens = 512, .stream = false
    };

    ChatResponse *resp = cluster_chat_completion(config, &req);
    const char *txt = cluster_extract_content(resp);
    snprintf(ctx->last_explanation, sizeof(ctx->last_explanation), "%s", txt);
    if (resp) {
        free(resp);
    }
}

/* ==========================================================================
 * TESTES UNITÁRIOS (padrão observador_primordial.c)
 * ========================================================================== */

#ifdef CLUSTER_UNIT_TEST

static int g_fail = 0;
#define CHECK(cond) do { if (!(cond)) { \
    printf("  FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); g_fail++; \
} } while (0)

static void test_config_defaults(void) {
    ClusterConfig cfg = { .api_key = getenv(CLUSTER_API_KEY_ENV), .model = CLUSTER_DEFAULT_MODEL,
                          .provider = CLUSTER_DEFAULT_PROVIDER, .timeout_ms = CLUSTER_TIMEOUT_MS,
                          .max_retries = CLUSTER_MAX_RETRIES, .use_x402 = false };
    CHECK(cfg.model != NULL);
    CHECK(cfg.provider != NULL);
    CHECK(cfg.max_retries == 3);
}

static void test_build_json_no_crash(void) {
    ClusterConfig cfg = { .api_key = "sk-x", .model = CLUSTER_DEFAULT_MODEL,
                          .provider = "venice", .timeout_ms = 1000, .max_retries = 1, .use_x402 = false };
    ChatMessage msgs[1] = { { .role = "user", .content = "ola" } };
    ChatRequest req = { .messages = msgs, .count = 1, .temperature = 0.7, .max_tokens = 128, .stream = false };
    char *json = cluster_build_json(&cfg, &req);
    CHECK(json != NULL);
    if (json) {
        CHECK(strstr(json, "\"model\":\"llama-3.3-70b-instruct\"") != NULL);
        CHECK(strstr(json, "\"provider\":\"venice\"") != NULL);
        CHECK(strstr(json, "\"role\":\"user\"") != NULL);
        free(json);
    }
}

static void test_parse_response(void) {
    const char *body =
        "{\"choices\":[{\"message\":{\"content\":\"explicacao A\"}}],"
        "\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":15}}";
    ChatResponse resp;
    cluster_parse_response(body, &resp);
    CHECK(resp.ok == true);
    CHECK(strcmp(resp.content, "explicacao A") == 0);
    CHECK(resp.total_tokens == 15);
}

static void test_chat_fake_transport(void) {
    ClusterConfig cfg = { .api_key = "sk-x", .model = CLUSTER_DEFAULT_MODEL,
                          .provider = "venice", .timeout_ms = 1000, .max_retries = 1, .use_x402 = false };
    ChatMessage msgs[1] = { { .role = "user", .content = "estado de coerencia?" } };
    ChatRequest req = { .messages = msgs, .count = 1, .temperature = 0.7, .max_tokens = 256, .stream = false };
    ChatResponse *resp = cluster_chat_completion(&cfg, &req);
    CHECK(resp != NULL);
    if (resp) {
        CHECK(resp->ok == true);
        CHECK(strstr(resp->content, "gamma_B") != NULL);
        free(resp);
    }
}

static void test_observador_adapter(void) {
    ClusterConfig cfg = { .api_key = "sk-x", .model = CLUSTER_DEFAULT_MODEL,
                          .provider = "venice", .timeout_ms = 1000, .max_retries = 1, .use_x402 = false };
    ClusterObservadorContext ctx;
    Cluster_ObservadorInit(&ctx);
    Cluster_ObservadorExplainAnomaly(&ctx, &cfg, 0.85, 0.85,
                                     "Anomalia: PhiC caiu para 0.85 por 3 ciclos");
    CHECK(ctx.explain_count == 1);
    CHECK(ctx.last_explanation[0] != '\0');
    CHECK(ctx.last_phi_c == 0.85);
}

static void test_x402_default_no_key(void) {
    ClusterConfig cfg = { .api_key = NULL, .model = CLUSTER_DEFAULT_MODEL,
                          .provider = "venice", .timeout_ms = 1000, .max_retries = 1, .use_x402 = true };
    ChatMessage msgs[1] = { { .role = "user", .content = "x" } };
    ChatRequest req = { .messages = msgs, .count = 1, .temperature = 0.7, .max_tokens = 64, .stream = false };
    ChatResponse *resp = cluster_chat_completion(&cfg, &req);
    CHECK(resp != NULL);
    if (resp) { free(resp); }
}

int main(void) {
    printf("=== CLUSTER CLIENT UNIT TESTS (BLOCO 483) ===\n");
    Cluster_Init();
    test_config_defaults();
    test_build_json_no_crash();
    test_parse_response();
    test_chat_fake_transport();
    test_observador_adapter();
    test_x402_default_no_key();
    Cluster_Shutdown();
    if (g_fail == 0) {
        printf("=== ALL TESTS PASSED ===\n");
        return 0;
    }
    printf("=== %d TEST(S) FAILED ===\n", g_fail);
    return 1;
}

#endif /* CLUSTER_UNIT_TEST */