// Package safety implements the mission-safety kernel (COM layer) that watches
// the LCS daemon health and sheds load when the pipeline degrades.
package safety

import (
	"context"
	"log"
	"time"
)

// MissionSafetyKernel monitors the LCS pipeline and enforces safe operation.
type MissionSafetyKernel struct {
	daemon loadGate
}

type loadGate interface {
	Admitted() uint64
	Rejected() uint64
	MaxTPS() float64
	ThrottleRatio() float64
}

// NewMissionSafetyKernel returns a safety kernel over the given load gate.
func NewMissionSafetyKernel(d loadGate) *MissionSafetyKernel {
	return &MissionSafetyKernel{daemon: d}
}

// RunHealthCheck periodically verifies the pipeline is within safe operating
// bounds, logging a warning when rejection is occurring.
func (k *MissionSafetyKernel) RunHealthCheck(ctx context.Context, interval time.Duration) {
	if interval <= 0 {
		interval = 5 * time.Second
	}
	t := time.NewTicker(interval)
	defer t.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case <-t.C:
			k.check()
		}
	}
}

func (k *MissionSafetyKernel) check() {
	if k.daemon == nil {
		return
	}
	rej := k.daemon.Rejected()
	adm := k.daemon.Admitted()
	if rej > 0 && (rej > adm*3 || rej > 1000) {
		log.Printf("safety: pipeline shedding load (admitted=%d rejected=%d) at %.0f TPS / %.0f ratio",
			adm, rej, k.daemon.MaxTPS(), k.daemon.ThrottleRatio())
	}
}
