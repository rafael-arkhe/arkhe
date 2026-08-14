package udm

// bridge.go implements the primary/fallback selection: a real UDM transport is
// the source of truth; the local HSS store is only consulted when the primary
// is unreachable or has no profile.

import (
	"context"
	"errors"
	"sync/atomic"
)

// Bridge selects a primary Client and falls back to the local store.
type Bridge struct {
	primary   Client
	fallback  *LocalClient
	fallbacks atomic.Int64
	served    atomic.Int64
}

// NewBridge returns a Bridge with the given primary transport and local store
// fallback.
func NewBridge(primary Client, store *LocalClient) *Bridge {
	return &Bridge{primary: primary, fallback: store}
}

// GetSubscriberData resolves a profile, falling back to the local store.
func (b *Bridge) GetSubscriberData(ctx context.Context, imsi string) (SubscriberData, error) {
	sd, err := b.primary.GetSubscriberData(ctx, imsi)
	if err == nil {
		b.served.Add(1)
		return sd, nil
	}
	// Primary unreachable or unknown profile -> local degraded mode.
	local, lerr := b.fallback.GetSubscriberData(ctx, imsi)
	if lerr != nil {
		return SubscriberData{}, err
	}
	b.fallbacks.Add(1)
	local.Source = "local-fallback"
	return local, nil
}

// AuthenticationInformation resolves vectors, falling back to local synthesis.
func (b *Bridge) AuthenticationInformation(ctx context.Context, imsi, snName string, numVectors int) ([]AuthVector, error) {
	vecs, err := b.primary.AuthenticationInformation(ctx, imsi, snName, numVectors)
	if err == nil {
		b.served.Add(1)
		return vecs, nil
	}
	local, lerr := b.fallback.AuthenticationInformation(ctx, imsi, snName, numVectors)
	if lerr != nil {
		return nil, err
	}
	b.fallbacks.Add(1)
	for i := range local {
		local[i].Source = "local-fallback"
	}
	return local, nil
}

// Name reports the primary transport's name.
func (b *Bridge) Name() string { return b.primary.Name() }

// Fallbacks returns the number of requests served from the local store.
func (b *Bridge) Fallbacks() int64 { return b.fallbacks.Load() }

// Served returns the number of requests served from the primary UDM.
func (b *Bridge) Served() int64 { return b.served.Load() }

// Primary exposes the underlying transport.
func (b *Bridge) Primary() Client { return b.primary }

// IsUnavailable reports whether err indicates the primary is down.
func IsUnavailable(err error) bool {
	return errors.Is(err, ErrUnavailable) || err == nil && false
}
