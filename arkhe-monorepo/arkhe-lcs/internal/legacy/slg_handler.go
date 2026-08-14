// Package legacy implements the 4G legacy interworking (IWF) leg of the
// ARKHE-LCS pipeline.
//
// Fix N2: Diameter (4G) and SBI (5G) are no longer treated as the same flow.
// The SLgHandler consumes only Diameter SLg frames produced by the C DRA (over
// the ring buffer), translates the carried identity, and reuses the same 5G
// SBI clients for UDM/AMF. Responses are written back to the ring for the
// C DRA to return to the MME.
package legacy

import (
	"context"
	"log"
	"strings"

	"arkhe-lcs/internal/hss"
	"arkhe-lcs/internal/ring"
	"arkhe-lcs/internal/sbi"
	"arkhe-lcs/pkg/diameter"
)

// SLgHandler processes Diameter SLg messages from the 4G MME leg.
type SLgHandler struct {
	daemon    *hss.HSSStateDaemon
	udmClient *sbi.NudmSDMClient
	amfClient *sbi.NamfLocationClient
}

// NewSLgHandler returns a handler for the legacy SLg flow.
func NewSLgHandler(d *hss.HSSStateDaemon, udm *sbi.NudmSDMClient, amf *sbi.NamfLocationClient) *SLgHandler {
	return &SLgHandler{
		daemon:    d,
		udmClient: udm,
		amfClient: amf,
	}
}

// ProcessRingBuffer consumes Diameter SLg frames from the shared ring until ctx
// is cancelled.
func (h *SLgHandler) ProcessRingBuffer(ctx context.Context, r *ring.ShmRing) {
	for {
		select {
		case <-ctx.Done():
			return
		default:
			if !h.daemon.AllowRequest() {
				continue
			}

			slot, release := r.Acquire()
			if slot == nil {
				continue
			}

			payload := r.SlotPayload(slot)
			if payload == nil {
				release()
				continue
			}

			// Extract IMSI/SUCI from the Diameter payload.
			rawIdentity, err := diameter.ParseIMSI(payload)
			if err != nil {
				log.Printf("SLg: parser Diameter falhou: %v", err)
				release()
				continue
			}

			var supi string
			if strings.HasPrefix(rawIdentity, "suci-") || !strings.HasPrefix(rawIdentity, "imsi-") {
				s, err := h.udmClient.DeconcealSUCI(ctx, rawIdentity)
				if err != nil {
					log.Printf("SLg: deconceal SUCI falhou: %v", err)
					release()
					continue
				}
				supi = s
			} else {
				supi = "imsi-" + strings.TrimPrefix(rawIdentity, "imsi-")
			}

			// Reuse the 5G SBI clients for UDM and AMF.
			_, err = h.udmClient.GetAMData(ctx, supi, nil)
			if err != nil {
				log.Printf("SLg: UDM falhou: %v", err)
				release()
				continue
			}

			locResp, err := h.amfClient.ProvidePositioningInfo(ctx, supi)
			if err != nil {
				log.Printf("SLg: AMF falhou: %v", err)
				release()
				continue
			}

			log.Printf("SLg: LOCALIZADO SUPI=%s, CellID=%s", supi, locResp.LocationInfo.CellId)

			// Response path: write the SLg answer back to the ring for the
			// C DRA to relay to the MME (SAI).
			_ = r.ProducerWrite([]byte("slg-answer:" + supi + ":" + locResp.LocationInfo.CellId))

			release()
		}
	}
}
