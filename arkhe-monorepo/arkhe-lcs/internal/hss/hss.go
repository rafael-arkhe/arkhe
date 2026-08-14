// Package hss implements the local subscriber store that backs the ARKHE-LCS
// pipeline when no real UDM is reachable, plus a transactions-per-second meter
// (fix R5).
package hss

import (
	"context"
	"sync"
	"sync/atomic"
	"time"
)

// Subscriber is the minimal S6a subscriber profile needed by the DRA/AMF leg.
type Subscriber struct {
	IMSI   string
	MSISDN string
	Slice  SliceInfo
}

// SliceInfo is a 5G network slice selection descriptor.
type SliceInfo struct {
	SST string // slice/service type, e.g. "1"
	SD  string // slice differentiator, e.g. "010203"
}

// Store is an in-memory HSS subscriber store.
type Store struct {
	mu     sync.RWMutex
	byIMSI map[string]Subscriber
}

// NewStore returns an empty subscriber store.
func NewStore() *Store {
	return &Store{byIMSI: make(map[string]Subscriber)}
}

// Put upserts a subscriber.
func (s *Store) Put(sub Subscriber) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.byIMSI[sub.IMSI] = sub
}

// Get returns a subscriber by IMSI.
func (s *Store) Get(imsi string) (Subscriber, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	sub, ok := s.byIMSI[imsi]
	return sub, ok
}

// Count returns the number of stored subscribers.
func (s *Store) Count() int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return len(s.byIMSI)
}

// Seed populates the store with the sample subscribers used by the local DRA.
func (s *Store) Seed(subs ...Subscriber) {
	for _, sub := range subs {
		s.Put(sub)
	}
}

// HSSStateDaemon is the load-aware admission gate for the LCS pipeline.
// AllowRequest returns false (and therefore sheds load) when the sustained
// transaction rate crosses the throttle ratio of the configured capacity.
type HSSStateDaemon struct {
	maxTPS        float64
	throttleRatio float64
	window        time.Duration
	events        []time.Time
	mu            sync.Mutex
	admitted      atomic.Uint64
	rejected      atomic.Uint64
}

// NewHSSStateDaemon returns a daemon that admits up to maxTPS transactions,
// rejecting requests once the sustained rate exceeds throttleRatio*maxTPS.
func NewHSSStateDaemon(maxTPS, throttleRatio float64) *HSSStateDaemon {
	if maxTPS <= 0 {
		maxTPS = 1
	}
	if throttleRatio <= 0 || throttleRatio > 1 {
		throttleRatio = 0.8
	}
	return &HSSStateDaemon{
		maxTPS:        maxTPS,
		throttleRatio: throttleRatio,
		window:        time.Second,
	}
}

// AllowRequest admits or sheds a request based on the current windowed rate.
func (d *HSSStateDaemon) AllowRequest() bool {
	now := time.Now()
	d.mu.Lock()
	d.events = append(d.events, now)
	cut := now.Add(-d.window)
	keep := 0
	for i, t := range d.events {
		if !t.Before(cut) {
			keep = i
			break
		}
		if i == len(d.events)-1 {
			keep = i
		}
	}
	d.events = d.events[keep:]
	rate := float64(len(d.events)) / d.window.Seconds()
	d.mu.Unlock()

	if rate > d.maxTPS*d.throttleRatio {
		d.rejected.Add(1)
		return false
	}
	d.admitted.Add(1)
	return true
}

// Admitted returns the total number of admitted requests.
func (d *HSSStateDaemon) Admitted() uint64 { return d.admitted.Load() }

// Rejected returns the total number of rejected (shed) requests.
func (d *HSSStateDaemon) Rejected() uint64 { return d.rejected.Load() }

// MaxTPS returns the configured capacity.
func (d *HSSStateDaemon) MaxTPS() float64 { return d.maxTPS }

// ThrottleRatio returns the configured admission ratio.
func (d *HSSStateDaemon) ThrottleRatio() float64 { return d.throttleRatio }

// CurrentLoad returns the current windowed transaction rate.
func (d *HSSStateDaemon) CurrentLoad() float64 {
	now := time.Now()
	d.mu.Lock()
	defer d.mu.Unlock()
	cut := now.Add(-d.window)
	n := 0
	for _, t := range d.events {
		if !t.Before(cut) {
			n++
		}
	}
	return float64(n) / d.window.Seconds()
}

// CurrentIntegrity returns a [0,1] health score: 1.0 while nothing is being
// shed, decaying toward 0 as rejection dominates.
func (d *HSSStateDaemon) CurrentIntegrity() float64 {
	adm := d.admitted.Load()
	rej := d.rejected.Load()
	total := adm + rej
	if total == 0 {
		return 1
	}
	return float64(adm) / float64(total)
}

// TPSMeter measures transactions per second over a sliding window.
//
// Fix R5: the meter uses a monotonic sliding window instead of a cumulative
// counter, so the rate is correct after idle periods and across window wraps.
type TPSMeter struct {
	mu     sync.Mutex
	daemon *HSSStateDaemon
	window time.Duration
	times  []time.Time
}

// NewTPSMeter returns a meter over the given window, driven by the daemon's
// transaction counter.
func NewTPSMeter(daemon *HSSStateDaemon, window time.Duration) *TPSMeter {
	if window <= 0 {
		window = time.Second
	}
	return &TPSMeter{daemon: daemon, window: window}
}

// Tick records one transaction at the current time.
func (m *TPSMeter) Tick() {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.times = append(m.times, time.Now())
}

// Run periodically trims the sliding window while ctx is active.
func (m *TPSMeter) Run(ctx context.Context) {
	ticker := time.NewTicker(m.window / 4)
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case now := <-ticker.C:
			m.trim(now)
		}
	}
}

// Rate returns transactions per second measured at time now.
func (m *TPSMeter) Rate(now time.Time) float64 {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.trim(now)
	return float64(len(m.times)) / m.window.Seconds()
}

// Inc records one transaction at time now.
func (m *TPSMeter) Inc(now time.Time) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.times = append(m.times, now)
	m.trim(now)
}

func (m *TPSMeter) trim(now time.Time) {
	cut := now.Add(-m.window)
	keep := len(m.times)
	for i, t := range m.times {
		if !t.Before(cut) {
			keep = i
			break
		}
	}
	m.times = m.times[keep:]
}
