package udm

import (
	"context"
	"errors"
	"testing"

	"arkhe-lcs/internal/hss"
)

type failingClient struct{}

func (failingClient) GetSubscriberData(ctx context.Context, imsi string) (SubscriberData, error) {
	return SubscriberData{}, ErrUnavailable
}
func (failingClient) AuthenticationInformation(ctx context.Context, imsi, snName string, n int) ([]AuthVector, error) {
	return nil, ErrUnavailable
}
func (failingClient) Name() string { return "failing" }

func TestBridgeFallsBackToLocal(t *testing.T) {
	store := hss.NewStore()
	store.Seed(hss.Subscriber{IMSI: "001010000000001", MSISDN: "5511999990001", Slice: hss.SliceInfo{SST: "1", SD: "010203"}})

	b := NewBridge(failingClient{}, NewLocal(store))
	sd, err := b.GetSubscriberData(context.Background(), "001010000000001")
	if err != nil {
		t.Fatalf("GetSubscriberData: %v", err)
	}
	if sd.Source != "local-fallback" {
		t.Errorf("source: %v", sd.Source)
	}
	if sd.MSISDN != "5511999990001" {
		t.Errorf("msisdn: %v", sd.MSISDN)
	}
	if b.Fallbacks() != 1 {
		t.Errorf("fallbacks: %d", b.Fallbacks())
	}
}

func TestBridgePrimaryServed(t *testing.T) {
	store := hss.NewStore()
	store.Seed(hss.Subscriber{IMSI: "001010000000001"})
	ok := NewLocal(store)
	b := NewBridge(ok, NewLocal(store))
	sd, err := b.GetSubscriberData(context.Background(), "001010000000001")
	if err != nil {
		t.Fatalf("GetSubscriberData: %v", err)
	}
	if sd.Source != "local" {
		t.Errorf("source: %v", sd.Source)
	}
	if b.Served() != 1 {
		t.Errorf("served: %d", b.Served())
	}
}

func TestLocalAuthVectorsDeterministic(t *testing.T) {
	store := hss.NewStore()
	store.Seed(hss.Subscriber{IMSI: "001010000000001"})
	c := NewLocal(store)
	v1, err := c.AuthenticationInformation(context.Background(), "001010000000001", "5g:mnc001.mcc001", 2)
	if err != nil {
		t.Fatalf("vectors: %v", err)
	}
	if len(v1) != 2 {
		t.Fatalf("expected 2 vectors, got %d", len(v1))
	}
	if v1[0].Rand == "" || v1[0].Autn == "" {
		t.Error("vector fields empty")
	}
	if _, err := c.GetSubscriberData(context.Background(), "missing"); !errors.Is(err, ErrSubscriberNotFound) {
		t.Errorf("expected ErrSubscriberNotFound, got %v", err)
	}
}
