package hss

import (
	"context"
	"testing"
	"time"
)

func TestStoreCRUD(t *testing.T) {
	s := NewStore()
	s.Seed(Subscriber{IMSI: "001010000000001", MSISDN: "5511999990001", Slice: SliceInfo{SST: "1", SD: "010203"}})
	if s.Count() != 1 {
		t.Fatalf("count: %d", s.Count())
	}
	sub, ok := s.Get("001010000000001")
	if !ok {
		t.Fatal("expected subscriber")
	}
	if sub.MSISDN != "5511999990001" {
		t.Errorf("msisdn: %q", sub.MSISDN)
	}
	if _, ok := s.Get("nope"); ok {
		t.Error("unexpected subscriber")
	}
}

func TestTPSMeterWindowed(t *testing.T) {
	d := NewHSSStateDaemon(1000, 0.8)
	m := NewTPSMeter(d, time.Second)
	now := time.Now()
	for i := 0; i < 100; i++ {
		m.Inc(now.Add(time.Duration(i) * time.Millisecond))
	}
	// 100 events spread over ~100ms, window 1s -> ~100 TPS.
	if rate := m.Rate(now.Add(500 * time.Millisecond)); rate < 90 || rate > 120 {
		t.Errorf("rate: %v (want ~100 within window)", rate)
	}
	// Events all fall outside the window after 2s -> 0.
	if rate := m.Rate(now.Add(2 * time.Second)); rate != 0 {
		t.Errorf("rate after expiry: %v", rate)
	}
}

func TestDaemonThrottle(t *testing.T) {
	d := NewHSSStateDaemon(100, 0.5)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	m := NewTPSMeter(d, time.Second)
	go m.Run(ctx)

	// 200 requests at 100/s capacity * 0.5 ratio -> heavy rejection expected.
	var admitted, rejected int
	for i := 0; i < 200; i++ {
		if d.AllowRequest() {
			admitted++
		} else {
			rejected++
		}
		time.Sleep(5 * time.Millisecond)
	}
	if rejected == 0 {
		t.Fatalf("expected some rejections (admitted=%d rejected=%d)", admitted, rejected)
	}
	if d.CurrentIntegrity() < 0 || d.CurrentIntegrity() > 1 {
		t.Errorf("integrity out of range: %v", d.CurrentIntegrity())
	}
}
