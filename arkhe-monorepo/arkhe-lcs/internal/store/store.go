// Package store defines the event pipeline used by the ARKHE-LCS control plane
// to buffer and transport Diameter/LCS events (fix R6).
//
// The package ships an in-memory pipeline and a file-backed append-only
// pipeline (Loopseal-2: append-only, tamper-evident audit trail). Redis and
// Kafka adapters can be added behind the same Pipeline interface without
// changing callers.
package store

import (
	"bufio"
	"errors"
	"os"
	"sync"
	"time"
)

// Event is a single audit/transport record.
type Event struct {
	Seq   uint64    `json:"seq"`
	Time  time.Time `json:"time"`
	Kind  string    `json:"kind"`
	Key   string    `json:"key"`
	Value string    `json:"value"`
}

// Pipeline transports events to downstream consumers (Redis, Kafka, files).
type Pipeline interface {
	// Publish appends one event.
	Publish(Event) error
	// Close releases resources.
	Close() error
}

// MemoryPipeline is a bounded in-memory pipeline.
type MemoryPipeline struct {
	mu  sync.Mutex
	buf *buffer
}

// NewMemory returns an in-memory pipeline with the given capacity.
func NewMemory(capacity int) *MemoryPipeline {
	return &MemoryPipeline{buf: &buffer{cap: capacity}}
}

// Publish appends an event, dropping the oldest when full.
func (p *MemoryPipeline) Publish(e Event) error {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.buf.push(e)
	return nil
}

// Close is a no-op for the in-memory pipeline.
func (p *MemoryPipeline) Close() error { return nil }

// Len returns the current number of buffered events.
func (p *MemoryPipeline) Len() int {
	p.mu.Lock()
	defer p.mu.Unlock()
	return p.buf.len()
}

type buffer struct {
	cap  int
	data []Event
}

func (b *buffer) push(e Event) {
	b.data = append(b.data, e)
	if len(b.data) > b.cap {
		b.data = b.data[len(b.data)-b.cap:]
	}
}

func (b *buffer) len() int { return len(b.data) }

// FilePipeline is an append-only JSON-lines audit trail.
type FilePipeline struct {
	mu   sync.Mutex
	w    *bufio.Writer
	f    *os.File
	seq  uint64
	path string
}

// NewFile returns an append-only pipeline writing JSON lines to path.
func NewFile(path string) (*FilePipeline, error) {
	f, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o600)
	if err != nil {
		return nil, err
	}
	return &FilePipeline{w: bufio.NewWriter(f), f: f, path: path}, nil
}

// Publish appends one JSON line and flushes the buffer.
func (p *FilePipeline) Publish(e Event) error {
	if p.w == nil {
		return errors.New("store: pipeline closed")
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	p.seq++
	e.Seq = p.seq
	line, err := encodeLine(e)
	if err != nil {
		return err
	}
	if _, err := p.w.Write(line); err != nil {
		return err
	}
	return p.w.Flush()
}

// Close flushes and closes the audit trail.
func (p *FilePipeline) Close() error {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.w == nil {
		return nil
	}
	err := p.w.Flush()
	cerr := p.f.Close()
	p.w, p.f = nil, nil
	if err != nil {
		return err
	}
	return cerr
}
