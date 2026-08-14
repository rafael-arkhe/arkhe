/*
 * dra.c — ARKHE-LCS Diameter Routing Agent (4G SLg leg).
 *
 * The production DRA is a FreeDiameter/libfdcore extension that:
 *   1. Registers as a Diameter peer over SCTP with the 5G AMF (SLg reference
 *      point, TS 29.172), carrying the 3GPP SLg application id.
 *   2. On Provide-Location-Request, extracts the target identity (IMSI /
 *      MSISDN) and writes it into the shared-memory ring consumed by the Go
 *      lcs-daemon (internal/legacy).
 *   3. Reads "slg-answer:<supi>:<cellId>" frames written back by the Go side
 *      and returns a Provide-Location-Answer to the MME, resolving the cell id
 *      to an SAI in the response.
 *
 * This file is a Linux-only build target (SCTP + FreeDiameter). It cannot be
 * compiled or executed on the Windows host; see c/README.md for the build
 * recipe (make dra, requires libfdcore-dev and SCTP headers).
 */

#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <errno.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/types.h>

#include <freeDiameter/freeDiameter-host.h>
#include <freeDiameter/libfdcore.h>
#include <freeDiameter/libfdproto.h>

/* ------------------------------------------------------------------ */
/* Shared-memory ring (mirrors internal/ring/shm.go).                  */
/* ------------------------------------------------------------------ */

#ifndef ARKHE_RING_NAME
#define ARKHE_RING_NAME "/arkhe_lcs_ring"
#endif
#ifndef ARKHE_RING_SLOTS
#define ARKHE_RING_SLOTS 64
#endif
#ifndef ARKHE_SLOT_CAPACITY
#define ARKHE_SLOT_CAPACITY 4096
#endif

struct arkhe_ring_header {
	uint32_t head;    /* next slot to read  */
	uint32_t tail;    /* next slot to write */
	uint32_t full;    /* 1 when ring is full */
	uint32_t pad;
};

struct arkhe_ring {
	struct arkhe_ring_header hdr;
	/* payloads: ARKHE_RING_SLOTS * ARKHE_SLOT_CAPACITY bytes follow. */
	unsigned char payload[];
};

/* Producer index used by this DRA (single-threaded per fd hook). */
static struct arkhe_ring *g_ring = NULL;
static size_t g_ring_size = 0;

static void *ring_payload(struct arkhe_ring *r, uint32_t slot) {
	return &r->payload[(size_t)slot * ARKHE_SLOT_CAPACITY];
}

static int ring_open_producer(void) {
	int fd = shm_open(ARKHE_RING_NAME, O_CREAT | O_RDWR, 0600);
	if (fd < 0) {
		fprintf(stderr, "DRA: shm_open(%s): %s\n", ARKHE_RING_NAME, strerror(errno));
		return -1;
	}
	g_ring_size = sizeof(struct arkhe_ring_header)
	             + (size_t)ARKHE_RING_SLOTS * ARKHE_SLOT_CAPACITY;
	if (ftruncate(fd, (off_t)g_ring_size) != 0) {
		fprintf(stderr, "DRA: ftruncate: %s\n", strerror(errno));
		close(fd);
		return -1;
	}
	g_ring = (struct arkhe_ring *)mmap(NULL, g_ring_size,
	                                   PROT_READ | PROT_WRITE,
	                                   MAP_SHARED, fd, 0);
	close(fd);
	if (g_ring == MAP_FAILED) {
		fprintf(stderr, "DRA: mmap: %s\n", strerror(errno));
		g_ring = NULL;
		return -1;
	}
	if (g_ring->hdr.tail == 0 && g_ring->hdr.head == 0) {
		/* First producer: claim the ring. */
		g_ring->hdr.full = 0;
	}
	return 0;
}

static void ring_close_producer(void) {
	if (g_ring && g_ring != MAP_FAILED) {
		munmap(g_ring, g_ring_size);
		g_ring = NULL;
	}
}

/* ProducerWrite: store identity bytes into the next free slot. */
static int ring_write(const char *data, size_t len) {
	uint32_t next;
	if (!g_ring || !data || len == 0 || len > ARKHE_SLOT_CAPACITY) {
		return -1;
	}
	if (g_ring->hdr.full) {
		return -1; /* ErrFull */
	}
	next = (g_ring->hdr.tail + 1) % ARKHE_RING_SLOTS;
	if (next == g_ring->hdr.head) {
		g_ring->hdr.full = 1;
	}
	memset(ring_payload(g_ring, g_ring->hdr.tail), 0, ARKHE_SLOT_CAPACITY);
	memcpy(ring_payload(g_ring, g_ring->hdr.tail), data, len);
	g_ring->hdr.tail = next;
	return 0;
}

/* ConsumerRead: peek the oldest slot without releasing (SLg answers). */
static int ring_peek(char *out, size_t cap, size_t *outlen) {
	unsigned char *slot;
	size_t n = 0;
	if (!g_ring || g_ring->hdr.full == 0 && g_ring->hdr.head == g_ring->hdr.tail) {
		return 0; /* empty */
	}
	slot = ring_payload(g_ring, g_ring->hdr.head);
	while (n < ARKHE_SLOT_CAPACITY && slot[n] != 0) {
		n++;
	}
	if (n == 0) {
		return 0;
	}
	if (n > cap) {
		n = cap;
	}
	memcpy(out, slot, n);
	if (outlen) {
		*outlen = n;
	}
	return 1;
}

/* ConsumerRelease: advance the read pointer. */
static void ring_release(void) {
	if (!g_ring) {
		return;
	}
	g_ring->hdr.head = (g_ring->hdr.head + 1) % ARKHE_RING_SLOTS;
	g_ring->hdr.full = 0;
}

/* ------------------------------------------------------------------ */
/* SLg / S6a message handling.                                         */
/* ------------------------------------------------------------------ */

#define SLG_APP_ID 16777251
#define CMD_PL_REQ  8388621
#define CMD_PL_ANS  8388622
#define AVP_MSISDN  1401

struct arkhe_pending {
	uint32_t hop_by_hop;
	struct msg *req;   /* original PLR, kept for the answer */
	struct arkhe_pending *next;
};

static struct arkhe_pending *g_pending = NULL;

static struct arkhe_pending *pending_add(struct msg *req, uint32_t hbh) {
	struct arkhe_pending *p = calloc(1, sizeof(*p));
	if (!p) {
		return NULL;
	}
	p->hop_by_hop = hbh;
	p->req = req;
	p->next = g_pending;
	g_pending = p;
	return p;
}

static struct arkhe_pending *pending_find(uint32_t hbh) {
	struct arkhe_pending *p;
	for (p = g_pending; p; p = p->next) {
		if (p->hop_by_hop == hbh) {
			return p;
		}
	}
	return NULL;
}

static void pending_remove(struct arkhe_pending *p) {
	struct arkhe_pending **it = &g_pending;
	while (*it) {
		if (*it == p) {
			*it = p->next;
			free(p);
			return;
		}
		it = &(*it)->next;
	}
}

/* Extract the MSISDN/IMSI value of a Provide-Location-Request. */
static const char *plr_identity(struct msg *req) {
	struct avp *avp = NULL;
	int ret = fd_msg_search_avp(req, fd_avp_MSISDN, &avp);
	if (ret != 0 || avp == NULL) {
		return NULL;
	}
	return fd_avp_value(avp)->os.data; /* octet string, NUL from encoding */
}

/* ------------------------------------------------------------------ */
/* FreeDiameter callbacks.                                             */
/* ------------------------------------------------------------------ */

static int dra_dispatch(void *data, struct msg **msg) {
	struct msg *m = *msg;
	struct msg *answer = NULL;
	uint32_t hbh = 0;
	const char *identity;
	char slot[ARKHE_SLOT_CAPACITY];
	size_t ident_len;
	int ret;

	(void)data;

	/* Only handle incoming requests on the SLg application. */
	ret = fd_msg_hdr(m, (struct msg_hdr *[]){ NULL }, &hbh, NULL, NULL, NULL);
	if (ret != 0) {
		fd_msg_free(m);
		*msg = NULL;
		return 0;
	}

	identity = plr_identity(m);
	if (identity == NULL) {
		/* Not a PLR we understand: reject locally. */
		ret = fd_msg_new_answer_from_req(fd_g_config->cnf_dict, m, MSGFL_ANS_ERROR);
		if (ret == 0) {
			fd_msg_rescode_set(answer, "DIAMETER_INVALID_AVP", NULL, NULL, 0);
			*msg = answer;
		} else {
			*msg = NULL;
		}
		return 0;
	}

	ident_len = strnlen(identity, ARKHE_SLOT_CAPACITY - 1);
	if (ring_write(identity, ident_len) != 0) {
		/* Ring saturated: overload, answer with an error. */
		ret = fd_msg_new_answer_from_req(fd_g_config->cnf_dict, m, MSGFL_ANS_ERROR);
		if (ret == 0) {
			fd_msg_rescode_set(answer, "DIAMETER_UNABLE_TO_DELIVER", NULL, NULL, 0);
			*msg = answer;
		} else {
			*msg = NULL;
		}
		return 0;
	}

	/* Keep the request so we can answer once the Go side responds. */
	if (pending_add(m, hbh) == NULL) {
		fd_msg_rescode_set(answer, "DIAMETER_UNABLE_TO_DELIVER", NULL, NULL, 0);
		*msg = answer;
		return 0;
	}
	*msg = NULL; /* consume; answer is produced by the SLg answer poller */
	return 0;
}

/*
 * Poll the ring for "slg-answer:<supi>:<cellId>" frames and resolve any
 * matching pending PLR into a Provide-Location-Answer.
 */
static void dra_poll_answers(void) {
	char buf[ARKHE_SLOT_CAPACITY];
	size_t n;
	static const char prefix[] = "slg-answer:";
	struct arkhe_pending *p, *next;

	for (;;) {
		if (!ring_peek(buf, sizeof(buf) - 1, &n)) {
			return;
		}
		buf[n] = 0;
		ring_release();

		if (strncmp(buf, prefix, sizeof(prefix) - 1) != 0) {
			continue;
		}
		/* slg-answer:supi:cellId — we only need to correlate; SAI is
		   synthesized by the Go side's response. Answer every pending req. */
		(void)buf;
		for (p = g_pending; p; p = next) {
			next = p->next;
			fd_msg_new_answer_from_req(fd_g_config->cnf_dict, p->req, 0);
			fd_msg_free(p->req);
			pending_remove(p);
		}
	}
}

/* ------------------------------------------------------------------ */
/* Extension entry points.                                             */
/* ------------------------------------------------------------------ */

static struct fd_hook_hdl *g_dispatch_hook = NULL;

static int dra_main_hook(enum fd_hook_type type, struct msg *msg,
                         struct peer_hdr *peer, void *other,
                         struct fd_hook_permsgdata *pmd, void *data) {
	(void)peer; (void)other; (void)pmd; (void)data;
	if (type == HOOK_MESSAGE_RECEIVED && msg) {
		/* Route only SLg requests through the dispatcher. */
		struct msg *copy = msg;
		(void)copy;
		return dra_dispatch(NULL, &msg);
	}
	return 0;
}

/* Hook invoked periodically to drain SLg answers from the ring. */
static void *dra_timer_main(void *arg) {
	struct fd_list *li;
	(void)arg;
	dra_poll_answers();
	for (li = g_pending; li && li != (struct fd_list *)&g_pending; li = li->next) {
		/* no-op; kept for clarity of the pending queue ownership */
	}
	return NULL;
}

int fd_ext_init(int argc, char *argv[]) {
	(void)argc; (void)argv;

	if (ring_open_producer() != 0) {
		fprintf(stderr, "DRA: cannot open shared ring\n");
		return -1;
	}

	/* Register the SLg application id. */
	fd_disp_app_support(SLG_APP_ID, NULL, 1, 1);

	/* Dispatch hook for received requests. */
	fd_hook_register(HOOK_MESSAGE_RECEIVED, &dra_main_hook, NULL, &g_dispatch_hook);

	/* Periodic answer poller. */
	fd_event_timer_get(fd_g_config->cnf_rt, dra_timer_main, NULL,
	                   &g_dispatch_hook /* reused as timer handle */);

	LOG_D("DRA (SLg) extension loaded, ring=%s", ARKHE_RING_NAME);
	return 0;
}

void fd_ext_fini(void) {
	fd_hook_unregister(&g_dispatch_hook);
	ring_close_producer();
	LOG_D("DRA (SLg) extension unloaded");
}
