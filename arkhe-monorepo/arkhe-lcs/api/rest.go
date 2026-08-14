// Package api implements the ARKHE-LCS northbound HTTP surface: the GMLC
// gateway (v0.7.0, HTTP/2 + mTLS) and the auxiliary REST API used by the
// udm-bridge Diameter leg.
//
// Fix R1: no handler trusts the local simulator; every profile/location
// request is routed through the real UDM/AMF SBI clients.
package api

import (
	"context"
	"encoding/json"
	"net/http"
	"strconv"
	"time"

	"arkhe-lcs/internal/udm"
)

// GMLCServer is the auxiliary REST API (subscriber / vectors) used by the
// udm-bridge Diameter leg.
type GMLCServer struct {
	client udm.Client
}

// NewGMLCServer returns an auxiliary REST handler over the given UDM client.
func NewGMLCServer(client udm.Client) *GMLCServer {
	return &GMLCServer{client: client}
}

type subscriberResponse struct {
	IMSI    string `json:"imsi"`
	MSISDN  string `json:"msisdn,omitempty"`
	SST     string `json:"sst,omitempty"`
	SD      string `json:"sd,omitempty"`
	Source  string `json:"source"`
	Latency string `json:"latency"`
}

// SubscriberHandler handles GET /v1/subscriber/{imsi}.
func (s *GMLCServer) SubscriberHandler(w http.ResponseWriter, r *http.Request) {
	imsi := r.PathValue("imsi")
	if imsi == "" {
		writeErr(w, http.StatusBadRequest, "imsi required")
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()
	start := time.Now()
	sd, err := s.client.GetSubscriberData(ctx, imsi)
	if err != nil {
		writeErr(w, http.StatusNotFound, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, subscriberResponse{
		IMSI:    sd.IMSI,
		MSISDN:  sd.MSISDN,
		SST:     sd.Slice.SST,
		SD:      sd.Slice.SD,
		Source:  sd.Source,
		Latency: time.Since(start).String(),
	})
}

// VectorHandler handles GET /v1/vectors/{imsi}?snName=...&n=...
func (s *GMLCServer) VectorHandler(w http.ResponseWriter, r *http.Request) {
	imsi := r.PathValue("imsi")
	if imsi == "" {
		writeErr(w, http.StatusBadRequest, "imsi required")
		return
	}
	snName := r.URL.Query().Get("snName")
	n := 1
	if q := r.URL.Query().Get("n"); q != "" {
		v, err := strconv.Atoi(q)
		if err != nil {
			writeErr(w, http.StatusBadRequest, "invalid n")
			return
		}
		n = v
	}
	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()
	vecs, err := s.client.AuthenticationInformation(ctx, imsi, snName, n)
	if err != nil {
		writeErr(w, http.StatusNotFound, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, vecs)
}

func writeJSON(w http.ResponseWriter, code int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(code)
	_ = json.NewEncoder(w).Encode(v)
}

func writeErr(w http.ResponseWriter, code int, msg string) {
	writeJSON(w, code, map[string]string{"error": msg})
}
