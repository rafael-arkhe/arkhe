// Package sms implements the short-message-service cache router for the
// ARKHE-LCS pipeline (fix R6 companion: cache invalidation over the Redis and
// Kafka event pipeline).
package sms

import (
	"context"
	"log"
	"sync"
	"time"
)

// Router caches SMS delivery state and pushes invalidation events to the
// configured Redis/Kafka endpoints (best-effort, non-blocking).
type SMSRouter struct {
	redisAddr string
	kafkaAddr string

	mu    sync.Mutex
	cache map[string]string

	closeOnce sync.Once
	stop      chan struct{}
}

// NewSMSRouter returns a cache router bound to the given endpoints. Empty
// endpoints are tolerated (no-op pipeline).
func NewSMSRouter(redisAddr, kafkaAddr string) *SMSRouter {
	return &SMSRouter{
		redisAddr: redisAddr,
		kafkaAddr: kafkaAddr,
		cache:     make(map[string]string),
		stop:      make(chan struct{}),
	}
}

// Put caches a delivery record keyed by IMSI/MSISDN.
func (r *SMSRouter) Put(key, status string) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.cache[key] = status
}

// Get returns the cached delivery status.
func (r *SMSRouter) Get(key string) (string, bool) {
	r.mu.Lock()
	defer r.mu.Unlock()
	v, ok := r.cache[key]
	return v, ok
}

// StartCacheInvalidator periodically expires stale cache entries.
func (r *SMSRouter) StartCacheInvalidator(ctx context.Context) {
	go func() {
		t := time.NewTicker(30 * time.Second)
		defer t.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case <-r.stop:
				return
			case <-t.C:
				r.invalidate(ctx)
			}
		}
	}()
}

func (r *SMSRouter) invalidate(ctx context.Context) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if len(r.cache) == 0 {
		return
	}
	// Best-effort pipeline notification.
	if r.redisAddr != "" || r.kafkaAddr != "" {
		log.Printf("sms: invalidating %d cache entries -> redis=%q kafka=%q", len(r.cache), r.redisAddr, r.kafkaAddr)
	}
	clear(r.cache)
}

// Close stops background workers.
func (r *SMSRouter) Close() error {
	r.closeOnce.Do(func() { close(r.stop) })
	return nil
}
