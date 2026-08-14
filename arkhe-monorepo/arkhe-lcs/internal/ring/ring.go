// Package ring provides a bounded, thread-safe ring buffer used by the
// ARKHE-LCS pipeline for buffering Diameter request events.
//
// Fix R2: access is guarded by a mutex and every index operation is bounds
// checked against the fixed capacity, so a full or empty buffer can never
// wrap into an out-of-range index.
package ring

import (
	"errors"
	"sync"
)

// ErrFull is returned when Push is called on a full buffer.
var ErrFull = errors.New("ring: buffer full")

// ErrEmpty is returned when Pop is called on an empty buffer.
var ErrEmpty = errors.New("ring: buffer empty")

// Buffer is a bounded FIFO ring buffer.
type Buffer struct {
	mu    sync.Mutex
	data  []any
	head  int // next read position
	tail  int // next write position
	count int
}

// New returns a ring buffer with the given fixed capacity.
func New(capacity int) (*Buffer, error) {
	if capacity <= 0 {
		return nil, errors.New("ring: capacity must be > 0")
	}
	return &Buffer{data: make([]any, capacity)}, nil
}

// Push appends an item, returning ErrFull if the buffer is at capacity.
func (b *Buffer) Push(item any) error {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.count == len(b.data) {
		return ErrFull
	}
	b.data[b.tail] = item
	b.tail = (b.tail + 1) % len(b.data)
	b.count++
	return nil
}

// Pop removes and returns the oldest item, returning ErrEmpty if empty.
func (b *Buffer) Pop() (any, error) {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.count == 0 {
		return nil, ErrEmpty
	}
	item := b.data[b.head]
	b.data[b.head] = nil
	b.head = (b.head + 1) % len(b.data)
	b.count--
	return item, nil
}

// Len returns the current number of buffered items.
func (b *Buffer) Len() int {
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.count
}

// Cap returns the fixed capacity.
func (b *Buffer) Cap() int { return len(b.data) }

// Reset clears the buffer.
func (b *Buffer) Reset() {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.head, b.tail, b.count = 0, 0, 0
	for i := range b.data {
		b.data[i] = nil
	}
}

// Drain removes all items, returning them in FIFO order.
func (b *Buffer) Drain() []any {
	b.mu.Lock()
	defer b.mu.Unlock()
	out := make([]any, 0, b.count)
	for b.count > 0 {
		out = append(out, b.data[b.head])
		b.data[b.head] = nil
		b.head = (b.head + 1) % len(b.data)
		b.count--
	}
	return out
}
