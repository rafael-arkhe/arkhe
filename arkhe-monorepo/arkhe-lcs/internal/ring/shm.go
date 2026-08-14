package ring

// shm.go implements the shared-memory ring used by the lcs-daemon consumer
// loop. On this platform (Windows) shared memory is emulated with a named,
// process-wide map; the Slot abstraction is preserved so the Linux port can
// swap in a real mmap-backed ring without touching callers.

import (
	"errors"
	"sync"
)

// ShmRing is a fixed-capacity ring of payload slots addressed by name, so
// producer (C DRA) and consumer (Go daemon) can share it.
type ShmRing struct {
	name  string
	slots [][]byte
	// head points at the next slot to read; tail at the next slot to write.
	head, tail int
	full       bool
	mu         sync.Mutex
}

var (
	shmMu       sync.Mutex
	shmRegistry = map[string]*ShmRing{}
)

// slotCapacity is the maximum payload size per slot.
const slotCapacity = 4096

// OpenShmRing opens (or creates) a named shared ring. If create is true and
// the ring does not exist, it is created with the default slot count.
func OpenShmRing(name string, create bool) (*ShmRing, error) {
	if name == "" {
		return nil, errors.New("ring: empty shm name")
	}
	shmMu.Lock()
	defer shmMu.Unlock()
	if r, ok := shmRegistry[name]; ok {
		return r, nil
	}
	if !create {
		return nil, errors.New("ring: shm " + name + " not found")
	}
	r := &ShmRing{name: name}
	for i := 0; i < 64; i++ {
		r.slots = append(r.slots, make([]byte, slotCapacity))
	}
	shmRegistry[name] = r
	return r, nil
}

// Close removes the named ring from the registry.
func (r *ShmRing) Close() error {
	shmMu.Lock()
	defer shmMu.Unlock()
	delete(shmRegistry, r.name)
	return nil
}

// ProducerWrite appends a payload into the next writable slot (used by the
// C DRA producer leg in tests; the real producer writes to the shared region).
func (r *ShmRing) ProducerWrite(payload []byte) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.full {
		return ErrFull
	}
	if len(payload) > slotCapacity {
		return errors.New("ring: payload exceeds slot capacity")
	}
	copy(r.slots[r.tail], payload)
	r.tail = (r.tail + 1) % len(r.slots)
	r.full = r.tail == r.head
	return nil
}

// Acquire returns a slot reference for reading, or nil when the ring is empty.
// The returned release function must be called once the payload is consumed.
func (r *ShmRing) Acquire() (slot []byte, release func()) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if !r.full && r.head == r.tail {
		return nil, func() {}
	}
	s := r.slots[r.head]
	done := false
	return s, func() {
		if done {
			return
		}
		done = true
		r.mu.Lock()
		defer r.mu.Unlock()
		r.head = (r.head + 1) % len(r.slots)
		r.full = false
	}
}

// SlotPayload returns the payload bytes referenced by a slot, trimmed of
// trailing NUL padding.
func (r *ShmRing) SlotPayload(slot []byte) []byte {
	if slot == nil {
		return nil
	}
	end := len(slot)
	for end > 0 && slot[end-1] == 0 {
		end--
	}
	return slot[:end]
}

// Len returns the number of unread slots.
func (r *ShmRing) Len() int {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.full {
		return len(r.slots)
	}
	n := r.tail - r.head
	if n < 0 {
		n += len(r.slots)
	}
	return n
}
