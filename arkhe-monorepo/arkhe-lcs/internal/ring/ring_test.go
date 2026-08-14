package ring

import "testing"

func TestRingFIFO(t *testing.T) {
	r, err := New(3)
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	for _, v := range []int{1, 2, 3} {
		if err := r.Push(v); err != nil {
			t.Fatalf("Push %d: %v", v, err)
		}
	}
	if err := r.Push(4); err != ErrFull {
		t.Fatalf("expected ErrFull, got %v", err)
	}
	for i, want := range []int{1, 2, 3} {
		v, err := r.Pop()
		if err != nil {
			t.Fatalf("Pop %d: %v", i, err)
		}
		if v != want {
			t.Fatalf("Pop %d: got %v want %v", i, v, want)
		}
	}
	if _, err := r.Pop(); err != ErrEmpty {
		t.Fatalf("expected ErrEmpty, got %v", err)
	}
}

// Fix R2: wrapping past capacity must not corrupt indices.
func TestRingWrapBounds(t *testing.T) {
	r, _ := New(2)
	for i := 0; i < 100; i++ {
		if err := r.Push(i); err != nil {
			t.Fatalf("Push %d: %v", i, err)
		}
		v, err := r.Pop()
		if err != nil {
			t.Fatalf("Pop %d: %v", i, err)
		}
		if v != i {
			t.Fatalf("cycle %d: got %v", i, v)
		}
	}
}

func TestShmRingProducerConsumer(t *testing.T) {
	r, err := OpenShmRing("test-lcs", true)
	if err != nil {
		t.Fatalf("OpenShmRing: %v", err)
	}
	defer r.Close()

	if err := r.ProducerWrite([]byte("imsi-001010000000001")); err != nil {
		t.Fatalf("ProducerWrite: %v", err)
	}
	slot, release := r.Acquire()
	if slot == nil {
		t.Fatalf("expected a slot")
	}
	if got := string(r.SlotPayload(slot)); got != "imsi-001010000000001" {
		t.Fatalf("payload: got %q", got)
	}
	release()
	if slot, _ := r.Acquire(); slot != nil {
		t.Fatalf("expected empty ring after release")
	}
}
