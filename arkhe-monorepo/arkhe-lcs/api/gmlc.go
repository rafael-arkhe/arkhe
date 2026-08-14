package api

// gmlc.go implements the 5G SBI GMLC gateway (v0.7.0): a single
// POST /nlmf-lcs/v1/{ueContextId}/provide-location endpoint served over
// HTTP/2 + mTLS (fix N3), resolving SUCI via Nudm_UECM, checking the
// subscriber via Nudm_SDM, and requesting positioning via NAmf_Location.

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strings"

	"arkhe-lcs/internal/hss"
	"arkhe-lcs/internal/lmf"
	"arkhe-lcs/internal/ring"
	"arkhe-lcs/internal/sbi"
	"arkhe-lcs/internal/sms"
)

// GMLCGateway is the northbound SBI gateway.
type GMLCGateway struct {
	daemon    *hss.HSSStateDaemon
	lmf       *lmf.PositioningEngine
	smsRouter *sms.SMSRouter
	ring      *ring.ShmRing

	udmClient *sbi.NudmSDMClient
	amfClient *sbi.NamfLocationClient
}

// NewGMLCGateway returns the GMLC gateway wired to the real UDM and AMF.
func NewGMLCGateway(
	d *hss.HSSStateDaemon,
	l *lmf.PositioningEngine,
	s *sms.SMSRouter,
	r *ring.ShmRing,
	udm *sbi.NudmSDMClient,
	amf *sbi.NamfLocationClient,
) *GMLCGateway {
	return &GMLCGateway{
		daemon:    d,
		lmf:       l,
		smsRouter: s,
		ring:      r,
		udmClient: udm,
		amfClient: amf,
	}
}

// ServeTLS starts the HTTP/2 + mTLS server (N3).
func (g *GMLCGateway) ServeTLS(addr, certFile, keyFile string) error {
	tlsConfig, err := sbi.ServerTLSConfig()
	if err != nil {
		return err
	}
	server := &http.Server{
		Addr:      addr,
		Handler:   g,
		TLSConfig: tlsConfig,
	}
	return server.ListenAndServeTLS(certFile, keyFile)
}

type provideLocationRequest struct {
	Supi         string `json:"supi"`
	LocationType string `json:"locationType"`
}

// ServeHTTP implements the single 5G SBI endpoint.
func (g *GMLCGateway) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req provideLocationRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, "Invalid JSON", http.StatusBadRequest)
		return
	}
	if req.Supi == "" {
		http.Error(w, "Missing supi", http.StatusBadRequest)
		return
	}

	ctx := r.Context()

	// 1. SUCI deconcealment via Nudm_UECM.
	supi := req.Supi
	if !strings.HasPrefix(supi, "imsi-") && !strings.HasPrefix(supi, "nai-") {
		deconcealed, err := g.udmClient.DeconcealSUCI(ctx, supi)
		if err != nil {
			http.Error(w, fmt.Sprintf("Deconcealment failed: %v", err), http.StatusBadRequest)
			return
		}
		supi = deconcealed
	}

	// 2. Subscriber check via Nudm_SDM.
	amData, err := g.udmClient.GetAMData(ctx, supi, nil)
	if err != nil || amData == nil {
		http.Error(w, "UDM subscription check failed", http.StatusNotFound)
		return
	}

	// 3. Positioning via NAmf_Location.
	locResp, err := g.amfClient.ProvidePositioningInfo(ctx, supi)
	if err != nil {
		http.Error(w, fmt.Sprintf("AMF location failed: %v", err), http.StatusInternalServerError)
		return
	}

	// 4. Response with daemon health telemetry.
	resp := map[string]interface{}{
		"supi":      supi,
		"cellId":    locResp.LocationInfo.CellId,
		"tac":       locResp.LocationInfo.Tac,
		"plmnId":    locResp.LocationInfo.PlmnId,
		"age":       locResp.LocationInfo.Age,
		"ccdi":      g.daemon.CurrentLoad(),
		"integrity": g.daemon.CurrentIntegrity(),
	}
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(resp)
}
