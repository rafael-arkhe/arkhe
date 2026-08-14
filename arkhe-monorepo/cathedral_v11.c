/*
 * ╔═══════════════════════════════════════════════════════════════════════════╗
 * ║   🏛️  CATHEDRAL ENGINE v11.1 — The Manifold (hardened)                  ║
 * ╠═══════════════════════════════════════════════════════════════════════════╣
 * ║   Correções v11.1:                                                        ║
 * ║   1. Buffer overflow fix — alocação dinâmica em schnorr/vrf              ║
 * ║   2. Manifold reconstruction — adiciona média de volta                   ║
 * ║   3. Predição real — regressão linear em latent space (out-of-sample)    ║
 * ║   4. Transubstantiation funcional — execução via memfd + reaping         ║
 * ║   5. Recepção de blocos — thread worker, verificação vs pubkey do sender ║
 * ║   6. Serialização portable — big-endian, packed, doubles quantizados     ║
 * ║   7. Chave privada protegida — mlock + zeroize explícito                 ║
 * ║   8. Remove rand()/LCG — getrandom obrigatório (sem fallback)            ║
 * ║   9. VRF verification em verify_block                                    ║
 * ║  10. Bekenstein normalizado [0,1] com detecção de colapso                ║
 * ║  11. Self-test corrigido (range s >= n real, wire round-trip)            ║
 * ╚═══════════════════════════════════════════════════════════════════════════╝
 */

#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/syscall.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <fcntl.h>
#include <time.h>
#include <stdint.h>
#include <math.h>
#include <errno.h>
#include <sys/mman.h>
#include <signal.h>
#include <getopt.h>
#include <stdarg.h>
#include <pthread.h>

#ifndef MFD_CLOEXEC
#define MFD_CLOEXEC 0x0001U
#endif

#define MULTICAST_IP    "239.255.255.250"
#define UDP_PORT        9999
#define DISC_PORT       9998
#define BK_RADIUS       0.1
#define BK_MASS         1.0
#define QME_THRESH      0.7
#define VERSION         11
#define STATE_SZ        256
#define MAX_PAYLOAD     2048
#define CYCLE_US        1000000

#define MANIFOLD_DIM    3
#define STI_HISTORY     6
#define STI_PREDICT     2
#define MEMORY_FACTOR   0.5
#define NUM_NEURONS     10
#define TAU_DELAY       1

#define LATENT_SCALE    1000
#define MAX_CHILDREN    16

static int g_verbose = 0;
static volatile int g_running = 1;
static pthread_mutex_t g_state_lock = PTHREAD_MUTEX_INITIALIZER;

/* ========== LOGGING ========== */
static void log_msg(const char *fmt, ...) {
    char buf[1024];
    time_t t = time(NULL);
    int n = strftime(buf, sizeof(buf), "[%H:%M:%S] ", localtime(&t));
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(buf + n, sizeof(buf) - n, fmt, ap);
    va_end(ap);
    fputs(buf, stderr);
}

static void hex_dump(const char *label, const uint8_t *d, size_t len) {
    if (!g_verbose) return;
    fprintf(stderr, "  %s: ", label);
    for (size_t i = 0; i < len && i < 32; i++) fprintf(stderr, "%02x", d[i]);
    if (len > 32) fprintf(stderr, "...");
    fprintf(stderr, "\n");
}

static void sig_handler(int s) { (void)s; g_running = 0; }

static void secure_zero(void *p, size_t n) {
    volatile uint8_t *v = (volatile uint8_t *)p;
    while (n--) *v++ = 0;
}

/* CSPRNG estrito: aborta se getrandom falhar. Nunca degrada para LCG. */
static void crypto_random_bytes(uint8_t *out, size_t n) {
    size_t off = 0;
    while (off < n) {
        ssize_t got = syscall(SYS_getrandom, out + off, n - off, 0);
        if (got <= 0) {
            log_msg("🛑 getrandom failed (errno=%d) — aborting\n", errno);
            exit(1);
        }
        off += (size_t)got;
    }
}

/* ========== SHA-256 ========== */
#define RR(x,n) (((x)>>(n))|((x)<<(32-(n))))
#define CH(x,y,z) (((x)&(y))^(~(x)&(z)))
#define MA(x,y,z) (((x)&(y))^((x)&(z))^((y)&(z)))
#define EP0(x) (RR(x,2)^RR(x,13)^RR(x,22))
#define EP1(x) (RR(x,6)^RR(x,11)^RR(x,25))
#define S0(x) (RR(x,7)^RR(x,18)^((x)>>3))
#define S1(x) (RR(x,17)^RR(x,19)^((x)>>10))

static const uint32_t K256[64] = {
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,
    0x923f82a4,0xab1c5ed5,0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,
    0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,0xe49b69c1,0xefbe4786,
    0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,
    0x06ca6351,0x14292967,0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,
    0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,0xa2bfe8a1,0xa81a664b,
    0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,
    0x5b9cca4f,0x682e6ff3,0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,
    0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
};

static void sha256_tf(uint32_t *h, const uint8_t *blk) {
    uint32_t w[64];
    for (int i = 0; i < 16; i++)
        w[i] = ((uint32_t)blk[i*4]<<24)|((uint32_t)blk[i*4+1]<<16)|
               ((uint32_t)blk[i*4+2]<<8)|blk[i*4+3];
    for (int i = 16; i < 64; i++)
        w[i] = S1(w[i-2]) + w[i-7] + S0(w[i-15]) + w[i-16];
    uint32_t a=h[0],b=h[1],c=h[2],d=h[3],e=h[4],f=h[5],g=h[6],hh=h[7];
    for (int i = 0; i < 64; i++) {
        uint32_t t1 = hh+EP1(e)+CH(e,f,g)+K256[i]+w[i];
        uint32_t t2 = EP0(a)+MA(a,b,c);
        hh=g; g=f; f=e; e=d+t1; d=c; c=b; b=a; a=t1+t2;
    }
    h[0]+=a; h[1]+=b; h[2]+=c; h[3]+=d; h[4]+=e; h[5]+=f; h[6]+=g; h[7]+=hh;
}

static int sha256(const uint8_t *in, size_t len, uint8_t out[32]) {
    uint32_t h[8]={0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,
                   0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19};
    uint64_t bits = (uint64_t)len * 8;
    size_t plen = ((len + 9 + 63) / 64) * 64;
    uint8_t *p = calloc(plen, 1);
    if (!p) return -1;
    memcpy(p, in, len);
    p[len] = 0x80;
    for (int i = 0; i < 8; i++) p[plen-1-i] = (uint8_t)(bits >> (i*8));
    for (size_t i = 0; i < plen; i += 64) sha256_tf(h, p+i);
    free(p);
    for (int i = 0; i < 8; i++) {
        out[i*4]=(h[i]>>24)&0xff; out[i*4+1]=(h[i]>>16)&0xff;
        out[i*4+2]=(h[i]>>8)&0xff; out[i*4+3]=h[i]&0xff;
    }
    return 0;
}

static int hmac_sha256(const uint8_t *key, size_t klen,
                       const uint8_t *msg, size_t mlen, uint8_t out[32]) {
    uint8_t k[64], tk[32], ipad[64], opad[64];
    memset(k, 0, 64);
    if (klen > 64) { if (sha256(key, klen, tk) < 0) return -1; memcpy(k, tk, 32); }
    else memcpy(k, key, klen);
    for (int i = 0; i < 64; i++) { ipad[i] = k[i]^0x36; opad[i] = k[i]^0x5c; }
    uint8_t *inner = malloc(64 + mlen);
    if (!inner) return -1;
    memcpy(inner, ipad, 64); memcpy(inner + 64, msg, mlen);
    uint8_t ih[32]; if (sha256(inner, 64 + mlen, ih) < 0) { free(inner); return -1; }
    free(inner);
    uint8_t *outer = malloc(96);
    if (!outer) return -1;
    memcpy(outer, opad, 64); memcpy(outer + 64, ih, 32);
    int ret = sha256(outer, 96, out);
    free(outer);
    secure_zero(k, 64); secure_zero(tk, 32);
    return ret;
}

/* ========== U256 ARITHMETIC ========== */
typedef struct { uint64_t d[4]; } u256;

static void u256_zero(u256 *a) { memset(a, 0, 32); }
static void u256_one(u256 *a) { u256_zero(a); a->d[0] = 1; }
static int  u256_is_zero(const u256 *a) { return !(a->d[0]|a->d[1]|a->d[2]|a->d[3]); }
static int  u256_bit(const u256 *a, int b) { return (a->d[b>>6] >> (b&63)) & 1; }

static int u256_cmp(const u256 *a, const u256 *b) {
    for (int i = 3; i >= 0; i--) {
        if (a->d[i] > b->d[i]) return 1;
        if (a->d[i] < b->d[i]) return -1;
    }
    return 0;
}

static void u256_add(u256 *r, const u256 *a, const u256 *b) {
    uint64_t c = 0;
    for (int i = 0; i < 4; i++) {
        __uint128_t s = (__uint128_t)a->d[i] + b->d[i] + c;
        r->d[i] = (uint64_t)s; c = (uint64_t)(s >> 64);
    }
}

static void u256_sub(u256 *r, const u256 *a, const u256 *b) {
    uint64_t c = 0;
    for (int i = 0; i < 4; i++) {
        __uint128_t d = (__uint128_t)a->d[i] - b->d[i] - c;
        r->d[i] = (uint64_t)d; c = (uint64_t)((d >> 64) & 1);
    }
}

static void u256_shr1(u256 *a) {
    for (int i = 0; i < 3; i++) a->d[i] = (a->d[i] >> 1) | (a->d[i+1] << 63);
    a->d[3] >>= 1;
}

static void u256_from_hex(u256 *a, const char *h) {
    u256_zero(a); size_t len = strlen(h);
    for (size_t i = 0; i < len; i++) {
        char c = h[len-1-i];
        int v = (c>='0'&&c<='9') ? c-'0' : (c>='a'&&c<='f') ? c-'a'+10 :
                (c>='A'&&c<='F') ? c-'A'+10 : -1;
        if (v < 0) continue;
        a->d[i>>4] |= (uint64_t)v << (4*(i&15));
    }
}

static void u256_from_be(u256 *r, const uint8_t *b) {
    u256_zero(r);
    for (int i = 0; i < 4; i++)
        for (int j = 0; j < 8; j++)
            r->d[i] |= (uint64_t)b[31-i*8-j] << (j*8);
}

static void u256_to_be(const u256 *a, uint8_t *b) {
    for (int i = 0; i < 4; i++)
        for (int j = 0; j < 8; j++)
            b[31-i*8-j] = (a->d[i] >> (j*8)) & 0xFF;
}

static int u256_random(u256 *r) {
    uint8_t buf[32];
    crypto_random_bytes(buf, 32);   /* estrito, sem fallback */
    u256_from_be(r, buf);
    return 0;
}

static u256 fp, fn, fgx, fgy;
static int finit = 0;

static void field_init(void) {
    if (finit) return;
    u256_from_hex(&fp,  "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F");
    u256_from_hex(&fn,  "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141");
    u256_from_hex(&fgx, "79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798");
    u256_from_hex(&fgy, "483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8");
    finit = 1;
}

/* NOTA: fp_mul, sc_mul, fp_inv, sc_inv NÃO são constant-time.
 * Para produção, usar libsodium/secp256k1 (ladder de Montgomery). */
static void fp_add(u256 *r, const u256 *a, const u256 *b) {
    /* Soma com acumulador de 5 limbs: o carry do cruzamento de 2^256 NÃO é
     * descartado (bug corrigido — v11.0 dropava o carry quando a+b >= 2^256). */
    uint64_t w[5] = {0, 0, 0, 0, 0};
    __uint128_t s = (__uint128_t)a->d[0] + b->d[0];
    w[0] = (uint64_t)s;
    uint64_t c = (uint64_t)(s >> 64);
    for (int i = 1; i < 4; i++) {
        __uint128_t t = (__uint128_t)a->d[i] + b->d[i] + c;
        w[i] = (uint64_t)t;
        c = (uint64_t)(t >> 64);
    }
    w[4] = c;
    int ge = w[4] != 0;
    if (!ge) {
        int cmp = 0;
        for (int i = 3; i >= 0; i--) {
            if (w[i] > fp.d[i]) { cmp = 1; break; }
            if (w[i] < fp.d[i]) { cmp = -1; break; }
        }
        ge = (cmp >= 0);
    }
    if (ge) {
        uint64_t borrow = 0;
        for (int i = 0; i < 4; i++) {
            uint64_t nb = ((__uint128_t)fp.d[i] + borrow > w[i]) ? 1 : 0;
            w[i] = w[i] - fp.d[i] - borrow;
            borrow = nb;
        }
    }
    r->d[0] = w[0]; r->d[1] = w[1]; r->d[2] = w[2]; r->d[3] = w[3];
}
static void fp_sub(u256 *r, const u256 *a, const u256 *b) {
    if (u256_cmp(a, b) >= 0) u256_sub(r, a, b);
    else { u256 t; u256_sub(&t, &fp, b); u256_add(r, a, &t); }
}
static void fp_neg(u256 *r, const u256 *a) {
    if (u256_is_zero(a)) u256_zero(r); else u256_sub(r, &fp, a);
}

static void fp_mul(u256 *r, const u256 *a, const u256 *b) {
    uint64_t l[8] = {0};
    for (int i = 0; i < 4; i++) {
        __uint128_t c = 0;
        for (int j = 0; j < 4; j++) {
            c += (__uint128_t)a->d[i] * b->d[j] + l[i + j];
            l[i + j] = (uint64_t)c;
            c >>= 64;
        }
        l[i + 4] = (uint64_t)c;
    }
    /* Redução: 2^256 ≡ 2^32 + 977 (mod p). Dobra o topo nos bits baixos. */
    uint64_t w[8];
    memcpy(w, l, 64);
    for (int it = 0; it < 8; it++) {
        uint64_t h0 = w[4], h1 = w[5], h2 = w[6], h3 = w[7];
        if (!(h0 | h1 | h2 | h3)) break;
        w[4] = w[5] = w[6] = w[7] = 0;
        uint64_t src[4] = {h0, h1, h2, h3};
        /* adiciona h * 977 */
        __uint128_t carry = 0;
        for (int i = 0; i < 4; i++) {
            __uint128_t p = (__uint128_t)src[i] * 977 + carry;
            carry = p >> 64;
            __uint128_t s = (__uint128_t)w[i] + (uint64_t)p;
            w[i] = (uint64_t)s;
            carry += s >> 64;
        }
        for (int i = 4; i < 8 && carry; i++) {
            __uint128_t s = (__uint128_t)w[i] + carry;
            w[i] = (uint64_t)s;
            carry = s >> 64;
        }
        /* adiciona h * 2^32 */
        carry = 0;
        for (int i = 0; i < 4; i++) {
            uint64_t lo = src[i] << 32, hi = src[i] >> 32;
            __uint128_t s = (__uint128_t)w[i] + lo + carry;
            w[i] = (uint64_t)s;
            carry = (s >> 64) + hi;
        }
        for (int i = 4; i < 8 && carry; i++) {
            __uint128_t s = (__uint128_t)w[i] + carry;
            w[i] = (uint64_t)s;
            carry = s >> 64;
        }
    }
    r->d[0] = w[0]; r->d[1] = w[1]; r->d[2] = w[2]; r->d[3] = w[3];
    while (u256_cmp(r, &fp) >= 0) u256_sub(r, r, &fp);
}

static void fp_inv(u256 *r, const u256 *a) {
    u256 exp,base=*a,result;u256_from_hex(&exp,"FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2D");u256_one(&result);
    while(!u256_is_zero(&exp)){if(exp.d[0]&1){u256 t;fp_mul(&t,&result,&base);result=t;}u256 t;fp_mul(&t,&base,&base);base=t;u256_shr1(&exp);}*r=result;
}

static void sc_add(u256 *r, const u256 *a, const u256 *b) {
    /* Soma mod n com acumulador de 5 limbs: carry de 2^256 preservado
     * (v11.0 dropava o carry com ~50% de probabilidade — s errado). */
    uint64_t w[5] = {0, 0, 0, 0, 0};
    __uint128_t s = (__uint128_t)a->d[0] + b->d[0];
    w[0] = (uint64_t)s;
    uint64_t c = (uint64_t)(s >> 64);
    for (int i = 1; i < 4; i++) {
        __uint128_t t = (__uint128_t)a->d[i] + b->d[i] + c;
        w[i] = (uint64_t)t;
        c = (uint64_t)(t >> 64);
    }
    w[4] = c;
    int ge = w[4] != 0;
    if (!ge) {
        int cmp = 0;
        for (int i = 3; i >= 0; i--) {
            if (w[i] > fn.d[i]) { cmp = 1; break; }
            if (w[i] < fn.d[i]) { cmp = -1; break; }
        }
        ge = (cmp >= 0);
    }
    if (ge) {
        uint64_t borrow = 0;
        for (int i = 0; i < 4; i++) {
            uint64_t nb = ((__uint128_t)fn.d[i] + borrow > w[i]) ? 1 : 0;
            w[i] = w[i] - fn.d[i] - borrow;
            borrow = nb;
        }
    }
    r->d[0] = w[0]; r->d[1] = w[1]; r->d[2] = w[2]; r->d[3] = w[3];
}

static void sc_mul(u256 *r, const u256 *a, const u256 *b) {
    uint64_t l[8] = {0};
    for (int i = 0; i < 4; i++) {
        __uint128_t c = 0;
        for (int j = 0; j < 4; j++) {
            c += (__uint128_t)a->d[i] * b->d[j] + l[i + j];
            l[i + j] = (uint64_t)c;
            c >>= 64;
        }
        l[i + 4] = (uint64_t)c;
    }
    /* Divisão bit a bit com acumulador de 5 limbs (320 bits): o carry do
     * cruzamento de 2^256 NÃO é descartado (bug corrigido do v11.0). */
    uint64_t rem[5] = {0, 0, 0, 0, 0};
    for (int bit = 511; bit >= 0; bit--) {
        uint64_t carry = 0;
        for (int i = 0; i < 5; i++) {
            uint64_t nxt = rem[i] >> 63;
            rem[i] = (rem[i] << 1) | carry;
            carry = nxt;
        }
        int w = bit >> 6, p = bit & 63;
        uint64_t v = (w < 8) ? l[w] : 0;
        if ((v >> p) & 1) rem[0] |= 1;
        int ge = 0;
        if (rem[4]) {
            ge = 1;
        } else {
            int cmp = 0;
            for (int i = 3; i >= 0; i--) {
                if (rem[i] > fn.d[i]) { cmp = 1; break; }
                if (rem[i] < fn.d[i]) { cmp = -1; break; }
            }
            ge = (cmp >= 0);
        }
        if (ge) {
            u256 rr;
            rr.d[0] = rem[0]; rr.d[1] = rem[1]; rr.d[2] = rem[2]; rr.d[3] = rem[3];
            u256_sub(&rr, &rr, &fn);
            rem[0] = rr.d[0]; rem[1] = rr.d[1]; rem[2] = rr.d[2]; rem[3] = rr.d[3];
            rem[4] = 0;
        }
    }
    r->d[0] = rem[0]; r->d[1] = rem[1]; r->d[2] = rem[2]; r->d[3] = rem[3];
}

static void sc_inv(u256 *r, const u256 *a) {
    u256 exp,base=*a,result;u256_from_hex(&exp,"FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD036413F");u256_one(&result);
    while(!u256_is_zero(&exp)){if(exp.d[0]&1){u256 t;sc_mul(&t,&result,&base);result=t;}u256 t;sc_mul(&t,&base,&base);base=t;u256_shr1(&exp);}*r=result;
}

/* ========== ELLIPTIC CURVE ========== */
typedef struct { u256 x, y; int inf; } ecpt;

static void ec_inf_pt(ecpt *p) { u256_zero(&p->x); u256_zero(&p->y); p->inf=1; }
static void ec_set(ecpt *p, const u256 *x, const u256 *y) { p->x=*x; p->y=*y; p->inf=0; }

static int ec_valid(const ecpt *p) {
    if (p->inf) return 1;
    if (u256_cmp(&p->x,&fp)>=0||u256_cmp(&p->y,&fp)>=0) return 0;
    u256 y2,x3,rhs,seven;fp_mul(&y2,&p->y,&p->y);fp_mul(&x3,&p->x,&p->x);fp_mul(&x3,&x3,&p->x);
    u256_from_hex(&seven,"7");fp_add(&rhs,&x3,&seven);return u256_cmp(&y2,&rhs)==0;
}

static void ec_dbl(ecpt *r, const ecpt *p) {
    if (p->inf) { ec_inf_pt(r); return; }
    u256 x2, three, two, lam, den, inv, lam2, twox, dx, ly, rx, ry;
    u256_from_hex(&three, "3"); u256_from_hex(&two, "2");
    fp_mul(&x2, &p->x, &p->x); fp_mul(&x2, &x2, &three);
    fp_mul(&den, &p->y, &two); fp_inv(&inv, &den); fp_mul(&lam, &x2, &inv);
    fp_mul(&lam2, &lam, &lam); fp_mul(&twox, &p->x, &two);
    fp_sub(&rx, &lam2, &twox);
    fp_sub(&dx, &p->x, &rx); fp_mul(&ly, &lam, &dx);
    fp_sub(&ry, &ly, &p->y);
    r->x = rx; r->y = ry; r->inf = 0;
}

static void ec_add_pt(ecpt *r, const ecpt *a, const ecpt *b) {
    if (a->inf) { *r = *b; return; }
    if (b->inf) { *r = *a; return; }
    if (u256_cmp(&a->x, &b->x) == 0) {
        if (u256_cmp(&a->y, &b->y) == 0) { ec_dbl(r, a); return; }
        ec_inf_pt(r); return;
    }
    u256 dx, dy, inv, lam, lam2, xsum, dx2, ly, rx, ry;
    fp_sub(&dx, &b->x, &a->x); fp_inv(&inv, &dx);
    fp_sub(&dy, &b->y, &a->y); fp_mul(&lam, &dy, &inv);
    fp_mul(&lam2, &lam, &lam);
    fp_add(&xsum, &a->x, &b->x);
    fp_sub(&rx, &lam2, &xsum);
    fp_sub(&dx2, &a->x, &rx); fp_mul(&ly, &lam, &dx2);
    fp_sub(&ry, &ly, &a->y);
    r->x = rx; r->y = ry; r->inf = 0;
}

/* NOTA: ec_mul NÃO é constant-time — leaka bits da chave via timing.
 * Para produção, usar ladder de Montgomery. */
static void ec_mul(ecpt *r, const ecpt *p, const u256 *k) {
    ecpt result,base=*p;ec_inf_pt(&result);
    for(int i=0;i<256;i++){if(u256_bit(k,i))ec_add_pt(&result,&result,&base);ec_dbl(&base,&base);}*r=result;
}

static void ec_gen_mul(ecpt *r, const u256 *k) { ecpt g;ec_set(&g,&fgx,&fgy);ec_mul(r,&g,k); }

static void ec_compress(const ecpt *p, uint8_t out[33]) { out[0]=0x02|(p->y.d[0]&1);u256_to_be(&p->x,out+1); }

static int ec_decompress(ecpt *r, const uint8_t in[33]) {
    u256 x;u256_from_be(&x,in+1);
    if (u256_cmp(&x, &fp) >= 0) return -1;
    int yp=in[0]&1;u256 x3,y2;
    fp_mul(&x3,&x,&x);fp_mul(&x3,&x3,&x);u256 seven;u256_from_hex(&seven,"7");fp_add(&y2,&x3,&seven);
    u256 exp;u256_from_hex(&exp,"3FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFBFFFFF0C");
    u256 base=y2,result;u256_one(&result);
    while(!u256_is_zero(&exp)){if(exp.d[0]&1){u256 t;fp_mul(&t,&result,&base);result=t;}u256 t;fp_mul(&t,&base,&base);base=t;u256_shr1(&exp);}
    if((result.d[0]&1)!=yp)fp_neg(&result,&result);
    ec_set(r,&x,&result);
    return ec_valid(r)?0:-1;
}

/* ========== SCHNORR + VRF ========== */
typedef struct { uint8_t R[33]; uint8_t e[32]; uint8_t s[32]; } SchnorrProof;
typedef struct { uint8_t output[32]; SchnorrProof proof; } VRFOutput;

/* Buffers alocados dinamicamente para evitar stack overflow */
static int schnorr_prove(const u256 *x, const ecpt *P, const uint8_t *msg,
                         size_t mlen, SchnorrProof *proof) {
    u256 k;
    if (u256_random(&k) < 0) return -1;
    while (u256_cmp(&k, &fn) >= 0 || u256_is_zero(&k)) {
        if (u256_random(&k) < 0) return -1;
    }
    ecpt R; ec_gen_mul(&R, &k); ec_compress(&R, proof->R);
    uint8_t Pb[33]; ec_compress(P, Pb);
    size_t buflen = 33 + 33 + mlen;
    uint8_t *buf = malloc(buflen);
    if (!buf) return -1;
    size_t off = 0;
    memcpy(buf+off, proof->R, 33); off += 33;
    memcpy(buf+off, Pb, 33); off += 33;
    memcpy(buf+off, msg, mlen); off += mlen;
    if (sha256(buf, off, proof->e) < 0) { free(buf); return -1; }
    free(buf);
    u256 ev, ex, sv; u256_from_be(&ev, proof->e); sc_mul(&ex, &ev, x); sc_add(&sv, &k, &ex);
    u256_to_be(&sv, proof->s); secure_zero(&k, sizeof(k));
    return 0;
}

static int schnorr_verify(const ecpt *P, const uint8_t *msg, size_t mlen,
                          const SchnorrProof *proof) {
    if (!ec_valid(P)) return 0;
    ecpt R; if (ec_decompress(&R, proof->R) != 0) return 0;
    if (!ec_valid(&R)) return 0;
    /* Range check: s in [1, n-1] */
    u256 sv; u256_from_be(&sv, proof->s);
    if (u256_is_zero(&sv)) return 0;
    if (u256_cmp(&sv, &fn) >= 0) return 0;
    uint8_t Pb[33]; ec_compress(P, Pb);
    size_t buflen = 33 + 33 + mlen;
    uint8_t *buf = malloc(buflen);
    if (!buf) return 0;
    size_t off = 0; memcpy(buf+off, proof->R, 33); off += 33;
    memcpy(buf+off, Pb, 33); off += 33; memcpy(buf+off, msg, mlen); off += mlen;
    uint8_t ec[32]; if (sha256(buf, off, ec) < 0) { free(buf); return 0; }
    free(buf);
    if (memcmp(ec, proof->e, 32) != 0) return 0;
    u256 ev; u256_from_be(&ev, proof->e);
    ecpt sG, eP, neg_eP, Rp; ec_gen_mul(&sG, &sv); ec_mul(&eP, P, &ev);
    fp_neg(&neg_eP.y, &eP.y); neg_eP.x = eP.x; neg_eP.inf = eP.inf;
    ec_add_pt(&Rp, &sG, &neg_eP); if (Rp.inf && R.inf) return 1;
    if (Rp.inf || R.inf) return 0;
    return (u256_cmp(&Rp.x, &R.x) == 0 && u256_cmp(&Rp.y, &R.y) == 0);
}

static int vrf_eval(const u256 *x, const ecpt *P, const uint8_t *msg,
                    size_t mlen, VRFOutput *vrf) {
    if (schnorr_prove(x, P, msg, mlen, &vrf->proof) < 0) return -1;
    uint8_t Pb[33]; ec_compress(P, Pb);
    size_t buflen = 33 + 33 + mlen;
    uint8_t *buf = malloc(buflen);
    if (!buf) return -1;
    size_t off = 0; memcpy(buf+off, Pb, 33); off += 33;
    memcpy(buf+off, vrf->proof.R, 33); off += 33; memcpy(buf+off, msg, mlen); off += mlen;
    int ret = sha256(buf, off, vrf->output);
    free(buf);
    return ret;
}

static int vrf_verify(const ecpt *P, const uint8_t *msg, size_t mlen,
                      const VRFOutput *vrf) {
    if (!schnorr_verify(P, msg, mlen, &vrf->proof)) return 0;
    uint8_t Pb[33]; ec_compress(P, Pb);
    size_t buflen = 33 + 33 + mlen;
    uint8_t *buf = malloc(buflen);
    if (!buf) return 0;
    size_t off = 0;
    memcpy(buf+off, Pb, 33); off += 33;
    memcpy(buf+off, vrf->proof.R, 33); off += 33;
    memcpy(buf+off, msg, mlen); off += mlen;
    uint8_t expected[32]; int ret = sha256(buf, off, expected);
    free(buf);
    if (ret < 0) return 0;
    return memcmp(expected, vrf->output, 32) == 0;
}

/* ========== MANIFOLD ========== */
typedef struct {
    double mu;
    double sigma;
    double A;
} VirtualNeuron;

typedef struct {
    VirtualNeuron neurons[NUM_NEURONS];
} NeuronPopulation;

static void neuron_pop_init(NeuronPopulation *pop) {
    for (int i = 0; i < NUM_NEURONS; i++) {
        pop->neurons[i].mu = (double)i / (double)NUM_NEURONS;
        pop->neurons[i].sigma = 0.08 + 0.12 * ((double)i / (double)NUM_NEURONS);
        pop->neurons[i].A = 1.0;
    }
}

static double neuron_fire(const VirtualNeuron *n, double stimulus) {
    double dx = stimulus - n->mu;
    return n->A * exp(-(dx * dx) / (2.0 * n->sigma * n->sigma));
}

static void population_encode(const NeuronPopulation *pop, double stimulus,
                              double out[NUM_NEURONS]) {
    for (int i = 0; i < NUM_NEURONS; i++)
        out[i] = neuron_fire(&pop->neurons[i], stimulus);
}

typedef struct {
    double latent[MANIFOLD_DIM][STI_HISTORY + STI_PREDICT];
    int history_len;
    int prediction_valid;
    double A[MANIFOLD_DIM][NUM_NEURONS];
    double B[NUM_NEURONS][MANIFOLD_DIM];
    double mean[NUM_NEURONS];
    NeuronPopulation pop;
    double neural_history[STI_HISTORY + STI_PREDICT][NUM_NEURONS];
    int neural_history_len;
    double latent_now[MANIFOLD_DIM];
    double latent_pred[MANIFOLD_DIM];
    double anomaly_score;
    double regression_coeffs[MANIFOLD_DIM][2];
} ManifoldState;

static void manifold_init(ManifoldState *ms) {
    memset(ms, 0, sizeof(ManifoldState));
    neuron_pop_init(&ms->pop);
    ms->history_len = 0;
    ms->prediction_valid = 0;
    ms->neural_history_len = 0;
}

static int simple_pca(const double data[][NUM_NEURONS], int N,
                      double components[NUM_NEURONS][MANIFOLD_DIM],
                      double scores[][MANIFOLD_DIM],
                      double mean[NUM_NEURONS]) {
    if (N < 2) return -1;
    memset(mean, 0, sizeof(double) * NUM_NEURONS);
    for (int i = 0; i < N; i++)
        for (int j = 0; j < NUM_NEURONS; j++)
            mean[j] += data[i][j];
    for (int j = 0; j < NUM_NEURONS; j++) mean[j] /= N;

    double cov[NUM_NEURONS][NUM_NEURONS];
    memset(cov, 0, sizeof(cov));
    for (int i = 0; i < NUM_NEURONS; i++)
        for (int j = i; j < NUM_NEURONS; j++) {
            double s = 0;
            for (int t = 0; t < N; t++)
                s += (data[t][i] - mean[i]) * (data[t][j] - mean[j]);
            cov[i][j] = cov[j][i] = s / (N - 1);
        }
    for (int k = 0; k < MANIFOLD_DIM; k++) {
        double vec[NUM_NEURONS];
        {
            uint8_t seed[32];
            crypto_random_bytes(seed, 32);
            for (int i = 0; i < NUM_NEURONS; i++)
                vec[i] = (seed[i] / 255.0) - 0.5;
        }
        for (int iter = 0; iter < 100; iter++) {
            double new_vec[NUM_NEURONS] = {0};
            for (int i = 0; i < NUM_NEURONS; i++)
                for (int j = 0; j < NUM_NEURONS; j++)
                    new_vec[i] += cov[i][j] * vec[j];
            for (int prev = 0; prev < k; prev++) {
                double dot = 0;
                for (int i = 0; i < NUM_NEURONS; i++)
                    dot += new_vec[i] * components[i][prev];
                for (int i = 0; i < NUM_NEURONS; i++)
                    new_vec[i] -= dot * components[i][prev];
            }
            double mag = 0;
            for (int i = 0; i < NUM_NEURONS; i++) mag += new_vec[i] * new_vec[i];
            mag = sqrt(mag);
            if (mag < 1e-12) break;
            for (int i = 0; i < NUM_NEURONS; i++) vec[i] = new_vec[i] / mag;
        }
        for (int i = 0; i < NUM_NEURONS; i++) components[i][k] = vec[i];
    }
    for (int t = 0; t < N; t++)
        for (int k = 0; k < MANIFOLD_DIM; k++) {
            double s = 0;
            for (int j = 0; j < NUM_NEURONS; j++)
                s += (data[t][j] - mean[j]) * components[j][k];
            scores[t][k] = s;
        }
    return 0;
}

static void fit_linear_regression(const double x[], const double y[], int n,
                                  double *a, double *b) {
    double sum_x = 0, sum_y = 0, sum_xy = 0, sum_x2 = 0;
    for (int i = 0; i < n; i++) {
        sum_x += x[i];
        sum_y += y[i];
        sum_xy += x[i] * y[i];
        sum_x2 += x[i] * x[i];
    }
    double denom = n * sum_x2 - sum_x * sum_x;
    if (fabs(denom) < 1e-12) { *a = 0; *b = sum_y / n; return; }
    *a = (n * sum_xy - sum_x * sum_y) / denom;
    *b = (sum_y * sum_x2 - sum_x * sum_xy) / denom;
}

static int sti_solve(ManifoldState *ms) {
    int N = ms->neural_history_len;
    if (N < STI_HISTORY + STI_PREDICT) return -1;
    double data[STI_HISTORY + STI_PREDICT][NUM_NEURONS];
    for (int t = 0; t < STI_HISTORY + STI_PREDICT; t++)
        for (int j = 0; j < NUM_NEURONS; j++) {
            double val = ms->neural_history[t][j];
            if (t >= TAU_DELAY)
                val += MEMORY_FACTOR * ms->neural_history[t - TAU_DELAY][j];
            data[t][j] = val;
        }
    double components[NUM_NEURONS][MANIFOLD_DIM];
    double scores[STI_HISTORY + STI_PREDICT][MANIFOLD_DIM];
    if (simple_pca(data, STI_HISTORY + STI_PREDICT, components, scores, ms->mean) < 0)
        return -1;
    for (int k = 0; k < MANIFOLD_DIM; k++)
        for (int j = 0; j < NUM_NEURONS; j++)
            ms->A[k][j] = components[j][k];
    for (int t = 0; t < STI_HISTORY + STI_PREDICT; t++)
        for (int k = 0; k < MANIFOLD_DIM; k++)
            ms->latent[k][t] = scores[t][k];
    for (int i = 0; i < NUM_NEURONS; i++)
        for (int k = 0; k < MANIFOLD_DIM; k++)
            ms->B[i][k] = ms->A[k][i];

    /* Regressão linear nos pontos de história (out-of-sample) */
    for (int k = 0; k < MANIFOLD_DIM; k++) {
        double t_vals[STI_HISTORY];
        double y_vals[STI_HISTORY];
        for (int i = 0; i < STI_HISTORY; i++) {
            t_vals[i] = (double)i;
            y_vals[i] = ms->latent[k][i];
        }
        fit_linear_regression(t_vals, y_vals, STI_HISTORY,
                              &ms->regression_coeffs[k][0],
                              &ms->regression_coeffs[k][1]);
    }

    ms->history_len = STI_HISTORY + STI_PREDICT;
    ms->prediction_valid = 1;
    return 0;
}

static int manifold_predict(const ManifoldState *ms, int offset,
                            double predicted_latent[MANIFOLD_DIM]) {
    if (!ms->prediction_valid) return -1;
    if (offset < 1 || offset > STI_PREDICT) return -1;
    for (int k = 0; k < MANIFOLD_DIM; k++) {
        double t = (double)(STI_HISTORY - 1 + offset);
        predicted_latent[k] = ms->regression_coeffs[k][0] * t + ms->regression_coeffs[k][1];
    }
    return 0;
}

static void manifold_reconstruct(const ManifoldState *ms,
                                 const double latent[MANIFOLD_DIM],
                                 double reconstructed[NUM_NEURONS]) {
    for (int j = 0; j < NUM_NEURONS; j++) {
        double s = ms->mean[j];
        for (int k = 0; k < MANIFOLD_DIM; k++)
            s += ms->B[j][k] * latent[k];
        reconstructed[j] = s;
    }
}

static double manifold_anomaly_score(const ManifoldState *ms,
                                     const double actual[NUM_NEURONS]) {
    if (!ms->prediction_valid) return 0.0;
    double latent_now[MANIFOLD_DIM];
    for (int k = 0; k < MANIFOLD_DIM; k++)
        latent_now[k] = ms->latent[k][STI_HISTORY - 1];
    double expected[NUM_NEURONS];
    manifold_reconstruct(ms, latent_now, expected);
    double dist = 0, mag = 0;
    for (int j = 0; j < NUM_NEURONS; j++) {
        double d = actual[j] - expected[j];
        dist += d * d;
        mag += actual[j] * actual[j];
    }
    if (mag < 1e-12) return 0.0;
    return sqrt(dist / mag);
}

/* ========== SERIALIZAÇÃO PORTABLE ========== */
typedef struct __attribute__((packed)) {
    uint32_t version;
    uint64_t timestamp;
    uint64_t cycle;
    uint8_t  prev_hash[32];
    uint8_t  state_hash[32];
    uint8_t  pubkey[33];
    uint8_t  sig_R[33];
    uint8_t  sig_e[32];
    uint8_t  sig_s[32];
    uint8_t  vrf_output[32];
    uint8_t  vrf_R[33];
    uint8_t  vrf_e[32];
    uint8_t  vrf_s[32];
    int32_t  latent_now[MANIFOLD_DIM];
    int32_t  latent_pred[MANIFOLD_DIM];
    int32_t  anomaly_q;
    uint8_t  manifold_valid;
} PackedBlockHeader;

typedef struct __attribute__((packed)) {
    PackedBlockHeader header;
    uint32_t    payload_len;
    uint8_t     payload[MAX_PAYLOAD];
} PackedBlock;

#define WIRE_HDR_SZ ((size_t)sizeof(PackedBlockHeader))

typedef struct {
    uint32_t version;
    uint64_t timestamp;
    uint64_t cycle;
    uint8_t  prev_hash[32];
    uint8_t  state_hash[32];
    uint8_t  pubkey[33];
    SchnorrProof sig;
    VRFOutput  vrf;
    double  latent_now[MANIFOLD_DIM];
    double  latent_pred[MANIFOLD_DIM];
    double  anomaly_score;
    uint8_t manifold_valid;
} BlockHeader;

typedef struct {
    BlockHeader header;
    uint8_t     payload[MAX_PAYLOAD];
    size_t      payload_len;
} Block;

typedef struct {
    u256 private_key;
    ecpt public_key;
} Identity;

typedef struct {
    double   entropy;
    uint64_t cycle_count;
    uint8_t  internal_state[STATE_SZ];
} EngineState;

static EngineState g_state;
static Identity    g_identity;
static int         g_udp_sock = -1;
static int         g_zk_enabled = 1;
static ManifoldState g_manifold;
static volatile uint64_t g_rx_accepted = 0;
static volatile uint64_t g_rx_rejected = 0;

static void pack_u32_be(uint8_t *p, uint32_t v) {
    p[0] = (v >> 24) & 0xFF; p[1] = (v >> 16) & 0xFF;
    p[2] = (v >> 8) & 0xFF; p[3] = v & 0xFF;
}
static void pack_u64_be(uint8_t *p, uint64_t v) {
    p[0] = (v >> 56) & 0xFF; p[1] = (v >> 48) & 0xFF;
    p[2] = (v >> 40) & 0xFF; p[3] = (v >> 32) & 0xFF;
    p[4] = (v >> 24) & 0xFF; p[5] = (v >> 16) & 0xFF;
    p[6] = (v >> 8) & 0xFF; p[7] = v & 0xFF;
}
static uint32_t unpack_u32_be(const uint8_t *p) {
    return ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) |
           ((uint32_t)p[2] << 8) | p[3];
}
static uint64_t unpack_u64_be(const uint8_t *p) {
    return ((uint64_t)p[0] << 56) | ((uint64_t)p[1] << 48) |
           ((uint64_t)p[2] << 40) | ((uint64_t)p[3] << 32) |
           ((uint64_t)p[4] << 24) | ((uint64_t)p[5] << 16) |
           ((uint64_t)p[6] << 8) | p[7];
}
static void pack_i32_be(uint8_t *p, int32_t v) {
    uint32_t u = (uint32_t)v;
    p[0] = (u >> 24) & 0xFF; p[1] = (u >> 16) & 0xFF;
    p[2] = (u >> 8) & 0xFF; p[3] = u & 0xFF;
}
static int32_t unpack_i32_be(const uint8_t *p) {
    return (int32_t)(((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) |
                     ((uint32_t)p[2] << 8) | p[3]);
}
static int32_t dbl_to_q(double v) {
    double x = v * LATENT_SCALE;
    if (x > (double)INT32_MAX) return INT32_MAX;
    if (x < (double)INT32_MIN) return INT32_MIN;
    return (int32_t)x;
}
static double q_to_dbl(int32_t q) { return (double)q / LATENT_SCALE; }

static void pack_block(const Block *blk, PackedBlock *out) {
    memset(out, 0, sizeof(PackedBlock));
    pack_u32_be((uint8_t*)&out->header.version, blk->header.version);
    pack_u64_be((uint8_t*)&out->header.timestamp, blk->header.timestamp);
    pack_u64_be((uint8_t*)&out->header.cycle, blk->header.cycle);
    memcpy(out->header.prev_hash, blk->header.prev_hash, 32);
    memcpy(out->header.state_hash, blk->header.state_hash, 32);
    memcpy(out->header.pubkey, blk->header.pubkey, 33);
    memcpy(out->header.sig_R, blk->header.sig.R, 33);
    memcpy(out->header.sig_e, blk->header.sig.e, 32);
    memcpy(out->header.sig_s, blk->header.sig.s, 32);
    memcpy(out->header.vrf_output, blk->header.vrf.output, 32);
    memcpy(out->header.vrf_R, blk->header.vrf.proof.R, 33);
    memcpy(out->header.vrf_e, blk->header.vrf.proof.e, 32);
    memcpy(out->header.vrf_s, blk->header.vrf.proof.s, 32);
    for (int k = 0; k < MANIFOLD_DIM; k++)
        pack_i32_be((uint8_t*)&out->header.latent_now[k], dbl_to_q(blk->header.latent_now[k]));
    for (int k = 0; k < MANIFOLD_DIM; k++)
        pack_i32_be((uint8_t*)&out->header.latent_pred[k], dbl_to_q(blk->header.latent_pred[k]));
    pack_i32_be((uint8_t*)&out->header.anomaly_q, dbl_to_q(blk->header.anomaly_score));
    out->header.manifold_valid = blk->header.manifold_valid;
    memcpy(out->payload, blk->payload, blk->payload_len);
    pack_u32_be((uint8_t*)&out->payload_len, (uint32_t)blk->payload_len);
}

static void unpack_block(const uint8_t *raw, size_t in_sz, Block *blk) {
    const PackedBlock *in = (const PackedBlock *)raw;
    memset(blk, 0, sizeof(Block));
    blk->header.version = unpack_u32_be((const uint8_t*)&in->header.version);
    blk->header.timestamp = unpack_u64_be((const uint8_t*)&in->header.timestamp);
    blk->header.cycle = unpack_u64_be((const uint8_t*)&in->header.cycle);
    memcpy(blk->header.prev_hash, in->header.prev_hash, 32);
    memcpy(blk->header.state_hash, in->header.state_hash, 32);
    memcpy(blk->header.pubkey, in->header.pubkey, 33);
    memcpy(blk->header.sig.R, in->header.sig_R, 33);
    memcpy(blk->header.sig.e, in->header.sig_e, 32);
    memcpy(blk->header.sig.s, in->header.sig_s, 32);
    memcpy(blk->header.vrf.output, in->header.vrf_output, 32);
    memcpy(blk->header.vrf.proof.R, in->header.vrf_R, 33);
    memcpy(blk->header.vrf.proof.e, in->header.vrf_e, 32);
    memcpy(blk->header.vrf.proof.s, in->header.vrf_s, 32);
    for (int k = 0; k < MANIFOLD_DIM; k++)
        blk->header.latent_now[k] = q_to_dbl(unpack_i32_be((const uint8_t*)&in->header.latent_now[k]));
    for (int k = 0; k < MANIFOLD_DIM; k++)
        blk->header.latent_pred[k] = q_to_dbl(unpack_i32_be((const uint8_t*)&in->header.latent_pred[k]));
    blk->header.anomaly_score = q_to_dbl(unpack_i32_be((const uint8_t*)&in->header.anomaly_q));
    blk->header.manifold_valid = in->header.manifold_valid;
    uint32_t plen = unpack_u32_be((const uint8_t*)&in->payload_len);
    if (plen > MAX_PAYLOAD) plen = MAX_PAYLOAD;
    if (WIRE_HDR_SZ + 4 + plen > in_sz) plen = 0;  /* truncado */
    blk->payload_len = plen;
    if (plen) memcpy(blk->payload, in->payload, plen);
}

/* ========== IDENTITY + STATE ========== */
static int init_identity(void) {
    field_init();
    if (u256_random(&g_identity.private_key) < 0) return -1;
    while (u256_cmp(&g_identity.private_key, &fn) >= 0) {
        if (u256_random(&g_identity.private_key) < 0) return -1;
    }
    ec_gen_mul(&g_identity.public_key, &g_identity.private_key);
    if (mlock(&g_identity, sizeof(g_identity)) != 0) {
        log_msg("⚠️  mlock failed: %s\n", strerror(errno));
    }
    uint8_t pub[33]; ec_compress(&g_identity.public_key, pub);
    log_msg("🔑 Identity initialized\n"); hex_dump("Public key", pub, 33);
    return 0;
}

static void init_state(void) {
    memset(&g_state, 0, sizeof(g_state)); g_state.entropy = 0.5;
    uint8_t seed[32];
    crypto_random_bytes(seed, 32);   /* estrito, sem fallback LCG */
    for (int i = 0; i < STATE_SZ; i++)
        g_state.internal_state[i] = seed[i % 32];
    secure_zero(seed, 32);
    manifold_init(&g_manifold);
}

/* ========== TRANSUBSTANTIATION FUNCIONAL ========== */
static pid_t g_children[MAX_CHILDREN];
static int   g_nchildren = 0;
static int   g_seal_fd = -1;

/* ELF64 x86-64 mínimo: exit(0) — payload executável real para o demo.
   Header 64B + phdr 56B + text 10B = 130 bytes. */
static const uint8_t g_demo_elf[] = {
    0x7F,0x45,0x4C,0x46,0x02,0x01,0x01,0x00, 0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x02,0x00, 0x3E,0x00, 0x01,0x00,0x00,0x00,
    0x78,0x00,0x40,0x00,0x00,0x00,0x00,0x00,
    0x40,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,
    0x40,0x00, 0x38,0x00, 0x01,0x00, 0x00,0x00, 0x00,0x00, 0x00,0x00,
    0x01,0x00,0x00,0x00,
    0x05,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x40,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x40,0x00,0x00,0x00,0x00,0x00,
    0x82,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x82,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x10,0x00,0x00,0x00,0x00,0x00,0x00,
    0xB8,0x3C,0x00,0x00,0x00,
    0x48,0x31,0xFF,
    0x0F,0x05
};

static void reap_children(void) {
    for (int i = 0; i < g_nchildren; i++) {
        if (g_children[i] <= 0) continue;
        int status;
        pid_t r = waitpid(g_children[i], &status, WNOHANG);
        if (r == g_children[i]) {
            if (WIFEXITED(status))
                log_msg("🏛️  Payload[%d] exited status %d\n", g_children[i], WEXITSTATUS(status));
            else if (WIFSIGNALED(status))
                log_msg("🏛️  Payload[%d] killed by signal %d\n", g_children[i], WTERMSIG(status));
            g_children[i] = -1;
        }
    }
    int w = 0;
    for (int i = 0; i < g_nchildren; i++)
        if (g_children[i] != -1) g_children[w++] = g_children[i];
    g_nchildren = w;
}

static void reap_all_children(void) {
    for (int i = 0; i < g_nchildren; i++) {
        if (g_children[i] <= 0) continue;
        int status;
        waitpid(g_children[i], &status, 0);
        g_children[i] = -1;
    }
    g_nchildren = 0;
}

static int transubstantiate(const uint8_t *payload, size_t len) {
    if (len < 4 || memcmp(payload, "\x7F""ELF", 4) != 0) {
        log_msg("⚠️  Transubstantiation skipped: not an ELF payload (%zu bytes)\n", len);
        return -1;
    }
    reap_children();
    int fd = (int)syscall(SYS_memfd_create, "cathedral", MFD_CLOEXEC);
    if (fd < 0) { log_msg("⚠️  memfd_create failed: %s\n", strerror(errno)); return -1; }
    if (write(fd, payload, len) != (ssize_t)len) { close(fd); return -1; }
    if (g_seal_fd >= 0) close(g_seal_fd);
    g_seal_fd = fd;   /* mantém o artefato selado vivo */

    char *argv[] = { "[cathedral-payload]", NULL };
    char *envp[] = { NULL };
    pid_t pid = fork();
    if (pid < 0) { return -1; }
    if (pid == 0) {
        fexecve(fd, argv, envp);
        _exit(127);
    }
    if (g_nchildren < MAX_CHILDREN) g_children[g_nchildren++] = pid;

    uint8_t seal[32]; sha256(payload, len, seal);
    log_msg("🏛️  Transubstantiation: %zu bytes → memfd exec [pid=%d]", len, pid);
    if (g_verbose) {
        fprintf(stderr, " (seal=");
        for (int i = 0; i < 4; i++) fprintf(stderr, "%02x", seal[i]);
        fprintf(stderr, "...)");
    }
    fprintf(stderr, "\n");
    return 0;
}

/* ========== QME + BEKENSTEIN ========== */
static void qme_jump(EngineState *s) {
    if (s->entropy <= QME_THRESH) return;
    double r = 3.9 + (s->entropy - QME_THRESH) * 0.1; if (r > 4.0) r = 4.0;
    double x = s->internal_state[0] / 255.0; if (x <= 0) x = 0.01; if (x >= 1) x = 0.99;
    for (int i = 0; i < 16; i++) {
        x = r * x * (1.0 - x);
        if (x <= 0) x = 0.01; if (x >= 1) x = 0.99;
        s->internal_state[i % STATE_SZ] ^= (uint8_t)(x * 255.0);
    }
    s->entropy *= 0.5;
}

/* Entropia de Shannon normalizada [0,1] sobre o estado interno apenas.
   Alta entropia = estado uniforme/aleatório (bom para crypto);
   colapso de entropia = estado degenerado → breach (reseeding). */
static double bekenstein_ratio(const EngineState *s) {
    double freq[256] = {0};
    for (size_t i = 0; i < STATE_SZ; i++) freq[s->internal_state[i]]++;
    double h = 0;
    for (int i = 0; i < 256; i++)
        if (freq[i] > 0) { double p = freq[i] / STATE_SZ; h -= p * log2(p); }
    return h / 8.0;
}

static int bekenstein_check(const EngineState *s) {
    double r = bekenstein_ratio(s);
    if (r < 0.05) { log_msg("🔥 Bekenstein COLLAPSE: %.4f (state degenerated)\n", r); return 2; }
    if (r < 0.10) { log_msg("⚠️  Bekenstein low-entropy warning: %.4f\n", r); return 1; }
    return 0;
}

/* ========== NETWORK ========== */
static int send_block(const Block *block) {
    if (g_udp_sock < 0) {
        g_udp_sock = socket(AF_INET, SOCK_DGRAM, 0);
        if (g_udp_sock < 0) return -1;
        int t = 1, l = 1;
        setsockopt(g_udp_sock, IPPROTO_IP, IP_MULTICAST_TTL, &t, sizeof(t));
        setsockopt(g_udp_sock, IPPROTO_IP, IP_MULTICAST_LOOP, &l, sizeof(l));
    }
    struct sockaddr_in addr = {.sin_family = AF_INET, .sin_port = htons(UDP_PORT)};
    inet_pton(AF_INET, MULTICAST_IP, &addr.sin_addr);

    PackedBlock pblk;
    pack_block(block, &pblk);
    size_t total = WIRE_HDR_SZ + 4 + block->payload_len;
    return sendto(g_udp_sock, &pblk, total, 0, (struct sockaddr *)&addr, sizeof(addr)) == (ssize_t)total ? 0 : -1;
}

static void *receive_thread(void *arg) {
    (void)arg;
    int recv_sock = socket(AF_INET, SOCK_DGRAM, 0);
    if (recv_sock < 0) { log_msg("❌ Receive socket failed\n"); return NULL; }

    int reuse = 1;
    setsockopt(recv_sock, SOL_SOCKET, SO_REUSEADDR, &reuse, sizeof(reuse));

    /* Timeout no recv para o shutdown não pendurar no pthread_join:
       g_running=0 precisa acordar o loop de recepção. */
    struct timeval rcv_to = { .tv_sec = 0, .tv_usec = 500000 };
    setsockopt(recv_sock, SOL_SOCKET, SO_RCVTIMEO, &rcv_to, sizeof(rcv_to));

    struct sockaddr_in addr = {.sin_family = AF_INET, .sin_port = htons(UDP_PORT)};
    addr.sin_addr.s_addr = INADDR_ANY;
    if (bind(recv_sock, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        log_msg("❌ Receive bind failed: %s\n", strerror(errno));
        close(recv_sock);
        return NULL;
    }

    struct ip_mreq mreq;
    mreq.imr_multiaddr.s_addr = inet_addr(MULTICAST_IP);
    mreq.imr_interface.s_addr = INADDR_ANY;
    setsockopt(recv_sock, IPPROTO_IP, IP_ADD_MEMBERSHIP, &mreq, sizeof(mreq));

    uint8_t self_pub[33];
    ec_compress(&g_identity.public_key, self_pub);
    log_msg("📡 Receiver thread started on %s:%d\n", MULTICAST_IP, UDP_PORT);

    while (g_running) {
        uint8_t rxbuf[sizeof(PackedBlock)];
        memset(rxbuf, 0, sizeof(rxbuf));
        ssize_t n = recv(recv_sock, rxbuf, sizeof(rxbuf), 0);
        if (n < (ssize_t)(WIRE_HDR_SZ + 4)) continue;

        Block blk;
        unpack_block(rxbuf, (size_t)n, &blk);

        /* Skip próprio echo */
        if (memcmp(self_pub, blk.header.pubkey, 33) == 0) continue;

        uint8_t msg[4 + 8 + 8 + 32 + 32];
        memcpy(msg, &blk.header.version, 4);
        memcpy(msg + 4, &blk.header.timestamp, 8);
        memcpy(msg + 12, &blk.header.cycle, 8);
        memcpy(msg + 20, blk.header.prev_hash, 32);
        memcpy(msg + 52, blk.header.state_hash, 32);

        int valid = 1;
        if (g_zk_enabled) {
            ecpt P;
            if (ec_decompress(&P, blk.header.pubkey) != 0) valid = 0;
            else {
                valid = schnorr_verify(&P, msg, 84, &blk.header.sig);
                if (valid) valid = vrf_verify(&P, msg, 84, &blk.header.vrf);
            }
        }

        if (valid) {
            g_rx_accepted++;
            if (g_verbose) {
                log_msg("📥 Accepted block cycle=%llu hash=", (unsigned long long)blk.header.cycle);
                for (int i = 0; i < 4; i++) fprintf(stderr, "%02x", blk.header.state_hash[i]);
                fprintf(stderr, "...\n");
            }
        } else {
            g_rx_rejected++;
            log_msg("🚫 Rejected invalid block cycle=%llu\n", (unsigned long long)blk.header.cycle);
        }
    }

    close(recv_sock);
    return NULL;
}

/* ========== EVOLVE + BLOCK CREATION ========== */
static void evolve_state_manifold(EngineState *s, uint64_t cycle) {
    pthread_mutex_lock(&g_state_lock);

    double stimulus = s->entropy;
    double neural_now[NUM_NEURONS];
    population_encode(&g_manifold.pop, stimulus, neural_now);
    if (g_manifold.neural_history_len >= STI_HISTORY + STI_PREDICT) {
        memmove(g_manifold.neural_history, g_manifold.neural_history + 1,
                sizeof(double) * (STI_HISTORY + STI_PREDICT - 1) * NUM_NEURONS);
        g_manifold.neural_history_len = STI_HISTORY + STI_PREDICT - 1;
    }
    memcpy(g_manifold.neural_history[g_manifold.neural_history_len],
           neural_now, sizeof(double) * NUM_NEURONS);
    g_manifold.neural_history_len++;
    int solved = sti_solve(&g_manifold);
    double anomaly = manifold_anomaly_score(&g_manifold, neural_now);
    if (solved == 0) {
        double pred_latent[MANIFOLD_DIM];
        if (manifold_predict(&g_manifold, 1, pred_latent) == 0) {
            double pred_neural[NUM_NEURONS];
            manifold_reconstruct(&g_manifold, pred_latent, pred_neural);
            double pred_stimulus = 0;
            for (int j = 0; j < NUM_NEURONS; j++)
                pred_stimulus += pred_neural[j];
            pred_stimulus /= NUM_NEURONS;
            double correction = (pred_stimulus - stimulus) * 0.1;
            s->entropy -= correction;
            if (g_verbose)
                log_msg("🧠 Manifold prediction: stimulus %.4f -> %.4f (corr %.3f)\n",
                        stimulus, pred_stimulus, correction);
            for (int k = 0; k < MANIFOLD_DIM; k++) {
                g_manifold.latent_now[k] = g_manifold.latent[k][STI_HISTORY - 1];
                g_manifold.latent_pred[k] = pred_latent[k];
            }
            g_manifold.anomaly_score = anomaly;
        }
    }
    uint8_t seed[32], cycle_b[8];
    memcpy(cycle_b, &cycle, 8);
    hmac_sha256(s->internal_state, 32, cycle_b, 8, seed);
    for (int i = 0; i < STATE_SZ; i++) {
        s->internal_state[i] ^= seed[i % 32];
        s->internal_state[i] = (s->internal_state[i] * 7 + 13) & 0xFF;
    }
    s->entropy += sin(cycle * 0.05) * 0.03 + (seed[0] / 255.0 - 0.5) * 0.1;
    if (s->entropy < 0) s->entropy = 0;
    if (s->entropy > 1) s->entropy = 1;
    s->cycle_count = cycle + 1;
    if (anomaly > 0.3) {
        log_msg("🚨 MANIFOLD ANOMALY: score %.4f (possible state seizure)\n", anomaly);
    } else if (g_verbose && anomaly > 0.1) {
        log_msg("📊 Manifold anomaly: %.4f\n", anomaly);
    }
    secure_zero(seed, 32);

    pthread_mutex_unlock(&g_state_lock);
}

static Block create_block(const EngineState *s, const uint8_t *prev_hash,
                          uint64_t cycle) {
    Block blk = {0};
    blk.header.version = VERSION;
    blk.header.timestamp = (uint64_t)time(NULL);
    blk.header.cycle = cycle;
    if (prev_hash) memcpy(blk.header.prev_hash, prev_hash, 32);
    sha256((const uint8_t *)s, sizeof(EngineState), blk.header.state_hash);
    ec_compress(&g_identity.public_key, blk.header.pubkey);
    uint8_t msg[4 + 8 + 8 + 32 + 32];
    memcpy(msg, &blk.header.version, 4);
    memcpy(msg + 4, &blk.header.timestamp, 8);
    memcpy(msg + 12, &blk.header.cycle, 8);
    memcpy(msg + 20, blk.header.prev_hash, 32);
    memcpy(msg + 52, blk.header.state_hash, 32);
    if (g_zk_enabled) {
        schnorr_prove(&g_identity.private_key, &g_identity.public_key,
                      msg, 84, &blk.header.sig);
        vrf_eval(&g_identity.private_key, &g_identity.public_key,
                 msg, 84, &blk.header.vrf);
    }
    if (g_manifold.prediction_valid) {
        memcpy(blk.header.latent_now, g_manifold.latent_now,
               sizeof(double) * MANIFOLD_DIM);
        memcpy(blk.header.latent_pred, g_manifold.latent_pred,
               sizeof(double) * MANIFOLD_DIM);
        blk.header.anomaly_score = g_manifold.anomaly_score;
        blk.header.manifold_valid = 1;
    }
    memcpy(blk.payload, s, sizeof(EngineState));
    blk.payload_len = sizeof(EngineState);
    return blk;
}

/* Verifica Schnorr + VRF contra o pubkey embutido no bloco (do sender). */
static int verify_block(const Block *blk) {
    if (!g_zk_enabled) return 1;
    uint8_t msg[4 + 8 + 8 + 32 + 32];
    memcpy(msg, &blk->header.version, 4);
    memcpy(msg + 4, &blk->header.timestamp, 8);
    memcpy(msg + 12, &blk->header.cycle, 8);
    memcpy(msg + 20, blk->header.prev_hash, 32);
    memcpy(msg + 52, blk->header.state_hash, 32);
    ecpt P;
    if (ec_decompress(&P, blk->header.pubkey) != 0) return 0;
    if (!schnorr_verify(&P, msg, 84, &blk->header.sig)) return 0;
    if (!vrf_verify(&P, msg, 84, &blk->header.vrf)) return 0;
    return 1;
}

/* ========== SELF TEST ========== */
static int self_test(void) {
    log_msg("🧪 Self-test...\n"); field_init(); int pass = 1;
    uint8_t h[32]; sha256((const uint8_t *)"abc", 3, h);
    const uint8_t exp[] = {0xba,0x78,0x16,0xbf,0x8f,0x01,0xcf,0xea,0x41,0x41,0x40,0xde,0x5d,0xae,0x22,0x23,0xb0,0x03,0x61,0xa3,0x96,0x17,0x7a,0x9c,0xb4,0x10,0xff,0x61,0xf2,0x00,0x15,0xad};
    if (memcmp(h, exp, 32) != 0) { log_msg("❌ SHA-256 fail\n"); pass = 0; }
    u256 one; u256_one(&one); ecpt P; ec_gen_mul(&P, &one);
    if (u256_cmp(&P.x, &fgx) != 0 || u256_cmp(&P.y, &fgy) != 0) { log_msg("❌ EC fail\n"); pass = 0; }
    uint8_t cb[33]; ec_compress(&P, cb); ecpt P2;
    if (ec_decompress(&P2, cb) != 0 || u256_cmp(&P2.x, &P.x) != 0) { log_msg("❌ Decompress fail\n"); pass = 0; }
    u256 tx; u256_from_hex(&tx, "DEADBEEFCAFEBABEDEADBEEFCAFEBABEDEADBEEFCAFEBABEDEADBEEFCAFEBABE");
    if (u256_cmp(&tx, &fn) >= 0) u256_sub(&tx, &tx, &fn); ecpt tP; ec_gen_mul(&tP, &tx);
    uint8_t tm[] = "Cathedral v11 test"; SchnorrProof pr;
    if (schnorr_prove(&tx, &tP, tm, 18, &pr) < 0) { log_msg("❌ Schnorr prove fail\n"); pass = 0; }
    else if (!schnorr_verify(&tP, tm, 18, &pr)) { log_msg("❌ Schnorr verify fail\n"); pass = 0; }
    uint8_t wm[] = "wrong"; if (schnorr_verify(&tP, wm, 5, &pr)) { log_msg("❌ Schnorr accept wrong\n"); pass = 0; }
    /* Rejeição real de s >= n: 0xFF...FF > n */
    {
        SchnorrProof big = pr;
        memset(big.s, 0xFF, 32);
        if (schnorr_verify(&tP, tm, 18, &big)) { log_msg("❌ Schnorr accept s >= n\n"); pass = 0; }
    }
    /* Rejeição de s == 0 */
    {
        SchnorrProof zero = pr;
        memset(zero.s, 0, 32);
        if (schnorr_verify(&tP, tm, 18, &zero)) { log_msg("❌ Schnorr accept s == 0\n"); pass = 0; }
    }
    VRFOutput vrf;
    if (vrf_eval(&tx, &tP, tm, 18, &vrf) < 0) { log_msg("❌ VRF eval fail\n"); pass = 0; }
    else if (!vrf_verify(&tP, tm, 18, &vrf)) { log_msg("❌ VRF verify fail\n"); pass = 0; }
    {
        VRFOutput vrf_bad = vrf;
        vrf_bad.output[0] ^= 1;
        if (vrf_verify(&tP, tm, 18, &vrf_bad)) { log_msg("❌ VRF accept bad output\n"); pass = 0; }
    }

    NeuronPopulation pop; neuron_pop_init(&pop);
    double f = neuron_fire(&pop.neurons[0], pop.neurons[0].mu);
    if (f < 0.9 || f > 1.1) { log_msg("❌ Tuning curve peak fail: %.4f\n", f); pass = 0; }
    double f_edge = neuron_fire(&pop.neurons[0], pop.neurons[0].mu + 3.0 * pop.neurons[0].sigma);
    if (f_edge > 0.1) { log_msg("❌ Tuning curve tail fail: %.4f\n", f_edge); pass = 0; }
    ManifoldState ms; manifold_init(&ms);
    for (int t = 0; t < STI_HISTORY + STI_PREDICT; t++) {
        double stim = 0.5 + 0.2 * sin(t * 0.3);
        population_encode(&ms.pop, stim, ms.neural_history[t]);
        ms.neural_history_len++;
    }
    int solved = sti_solve(&ms);
    if (solved != 0) { log_msg("❌ STI solve fail\n"); pass = 0; }
    else if (!ms.prediction_valid) { log_msg("❌ STI prediction not valid\n"); pass = 0; }
    else {
        double pred[MANIFOLD_DIM];
        if (manifold_predict(&ms, 1, pred) != 0) {
            log_msg("❌ Manifold predict fail\n"); pass = 0;
        } else if (g_verbose) {
            log_msg("🧠 Manifold latent prediction: [");
            for (int k = 0; k < MANIFOLD_DIM; k++)
                fprintf(stderr, "%.4f%s", pred[k], k < MANIFOLD_DIM - 1 ? ", " : "");
            fprintf(stderr, "]\n");
        }
        double recon[NUM_NEURONS];
        double latent_test[MANIFOLD_DIM] = {0};
        manifold_reconstruct(&ms, latent_test, recon);
        double mean_sum = 0;
        for (int j = 0; j < NUM_NEURONS; j++) mean_sum += recon[j];
        if (fabs(mean_sum) < 1e-6) {
            log_msg("❌ Reconstruction mean not restored\n"); pass = 0;
        }
    }

    /* Wire round-trip: packed serialization com doubles quantizados */
    {
        Block a, b; memset(&a, 0, sizeof(a));
        a.header.version = VERSION; a.header.cycle = 42;
        a.header.anomaly_score = 0.123; a.header.manifold_valid = 1;
        for (int k = 0; k < MANIFOLD_DIM; k++) {
            a.header.latent_now[k] = k * 1.5;
            a.header.latent_pred[k] = -k * 0.5;
        }
        memcpy(a.header.sig.R, "RRRR", 4);
        memcpy(a.header.vrf.output, "VVVV", 4);
        memcpy(a.header.pubkey, "PPPP", 4);
        memcpy(a.payload, "cathedral payload", 17);
        a.payload_len = 17;
        PackedBlock pb;
        pack_block(&a, &pb);
        size_t wire_sz = WIRE_HDR_SZ + 4 + a.payload_len;
        unpack_block((const uint8_t *)&pb, wire_sz, &b);
        if (b.header.cycle != 42 ||
            fabs(b.header.anomaly_score - 0.123) > 0.002 ||
            fabs(b.header.latent_pred[1] - (-0.5)) > 0.002 ||
            memcmp(b.header.vrf.output, "VVVV", 4) != 0 ||
            memcmp(b.header.pubkey, "PPPP", 4) != 0 ||
            b.payload_len != 17 || memcmp(b.payload, "cathedral payload", 17) != 0) {
            log_msg("❌ Wire serialize round-trip fail\n"); pass = 0;
        }
    }

    if (pass) log_msg("✅ All tests passed (including manifold + wire)\n");
    else log_msg("❌ Some tests FAILED\n");
    return pass;
}

static void print_banner(void) {
    fprintf(stderr,
        "\n"
        "  ╔═══════════════════════════════════════════════════════════╗\n"
        "  ║   🏛️  CATHEDRAL ENGINE v11.1 — The Manifold (hardened)  ║\n"
        "  ╠═══════════════════════════════════════════════════════════╣\n"
        "  ║   1. Transubstantiation — memfd + exec (ELF real)       ║\n"
        "  ║   2. Signing — Schnorr ZKP (secp256k1) + range check    ║\n"
        "  ║   3. Proclamation — UDP multicast + receiver thread       ║\n"
        "  ║   4. QME Acceleration — Chaotic entropy jumps           ║\n"
        "  ║   5. Bekenstein Guardian — normalized + collapse detect  ║\n"
        "  ║   6. Scripture — Arkhe-Chain + VRF (both verified)      ║\n"
        "  ║   7. Continuum — Abstract Stone Duality                 ║\n"
        "  ║   8. MANIFOLD — Neural trajectory + linear regression   ║\n"
        "  ║   9. Portable Serialization — BE, packed, quantized     ║\n"
        "  ║  10. Thread Safety — Mutex + mlock on private key       ║\n"
        "  ╚═══════════════════════════════════════════════════════════╝\n\n"
    );
}

int main(int argc, char **argv) {
    int once = 0;
    static struct option long_opts[] = {
        {"no-zk", no_argument, 0, 'z'}, {"verbose", no_argument, 0, 'v'},
        {"once", no_argument, 0, 'o'}, {"help", no_argument, 0, 'h'},
        {0, 0, 0, 0}
    };
    int opt;
    while ((opt = getopt_long(argc, argv, "zvoh", long_opts, NULL)) != -1) {
        switch (opt) {
            case 'z': g_zk_enabled = 0; break;
            case 'v': g_verbose = 1; break;
            case 'o': once = 1; break;
            case 'h': printf("Usage: %s [--no-zk] [--verbose] [--once]\n", argv[0]); return 0;
        }
    }
    print_banner();
    signal(SIGINT, sig_handler); signal(SIGTERM, sig_handler);
    if (!self_test()) { log_msg("🛑 Self-test failed\n"); return 1; }
    if (init_identity() < 0) { log_msg("🛑 Identity init failed\n"); return 1; }
    init_state();

    pthread_t recv_tid;
    if (pthread_create(&recv_tid, NULL, receive_thread, NULL) != 0) {
        log_msg("🛑 Failed to start receiver thread\n");
        return 1;
    }

    log_msg("⚙️  ZK: %s | Manifold: %dD (γ=%.2f, hist=%d, pred=%d) | Receiver: ON\n",
            g_zk_enabled ? "ON" : "OFF", MANIFOLD_DIM, MEMORY_FACTOR,
            STI_HISTORY, STI_PREDICT);
    uint8_t prev_hash[32] = {0};
    uint64_t cycle = 0;
    while (g_running) {
        log_msg("━━━ Cycle %llu ━━━\n", (unsigned long long)cycle);
        qme_jump(&g_state);
        int bk = bekenstein_check(&g_state);
        if (bk == 2) {
            log_msg("🛑 Bekenstein collapse — reseeding\n");
            crypto_random_bytes(g_state.internal_state, STATE_SZ);
            g_state.entropy = 0.1;
        }
        evolve_state_manifold(&g_state, cycle);
        Block blk = create_block(&g_state, prev_hash, cycle);
        if (g_zk_enabled && !verify_block(&blk))
            log_msg("❌ Block self-verify FAILED\n");
        else {
            if (send_block(&blk) == 0) {
                log_msg("📡 Block ");
                for (int i = 0; i < 4; i++) fprintf(stderr, "%02x", blk.header.state_hash[i]);
                fprintf(stderr, "...");
                if (blk.header.manifold_valid) {
                    fprintf(stderr, " [manifold:");
                    for (int k = 0; k < MANIFOLD_DIM; k++)
                        fprintf(stderr, " %.3f", blk.header.latent_pred[k]);
                    fprintf(stderr, " anom=%.3f]", blk.header.anomaly_score);
                }
                fprintf(stderr, " [rx: %llu ok / %llu bad]",
                        (unsigned long long)g_rx_accepted, (unsigned long long)g_rx_rejected);
                fprintf(stderr, "\n");
            }
        }
        if (cycle % 10 == 0) {
            transubstantiate(g_demo_elf, sizeof(g_demo_elf));
        }
        memcpy(prev_hash, blk.header.state_hash, 32);
        if (once) break;
        usleep(CYCLE_US); cycle++;
    }

    g_running = 0;
    pthread_join(recv_tid, NULL);
    reap_all_children();
    if (g_seal_fd >= 0) close(g_seal_fd);
    secure_zero(&g_identity, sizeof(g_identity));
    secure_zero(&g_state, sizeof(g_state));
    log_msg("🏛️  Cathedral Engine v11.1 stopped\n");
    return 0;
}
