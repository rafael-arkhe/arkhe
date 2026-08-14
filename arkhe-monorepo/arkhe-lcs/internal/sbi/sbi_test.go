package sbi

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestNrfTokenCached(t *testing.T) {
	var calls int
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		calls++
		if r.Method != http.MethodPost {
			t.Errorf("method: %v", r.Method)
		}
		if ct := r.Header.Get("Content-Type"); ct != "application/json" {
			t.Errorf("content-type: %v", ct)
		}
		var body map[string]string
		_ = json.NewDecoder(r.Body).Decode(&body)
		if body["grant_type"] != "client_credentials" {
			t.Errorf("grant_type: %v", body["grant_type"])
		}
		if body["scope"] != "nudm-sdm" {
			t.Errorf("scope: %v", body["scope"])
		}
		_ = json.NewEncoder(w).Encode(map[string]any{
			"access_token": "tok-abc",
			"expires_in":   3600,
		})
	}))
	defer ts.Close()

	c := NewNrfClient(ts.URL, "nf-1", "gmlc", "secret")
	tok1, err := c.GetToken(context.Background(), "nudm-sdm")
	if err != nil {
		t.Fatalf("GetToken: %v", err)
	}
	if tok1 != "tok-abc" {
		t.Errorf("token: %v", tok1)
	}
	// Second call must be served from cache.
	tok2, err := c.GetToken(context.Background(), "nudm-sdm")
	if err != nil {
		t.Fatalf("GetToken cached: %v", err)
	}
	if tok2 != tok1 {
		t.Errorf("cached token mismatch")
	}
	if calls != 1 {
		t.Errorf("expected 1 token call, got %d", calls)
	}
}

func TestUdmDeconcealSUCI(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !strings.Contains(r.URL.Path, "deconceal-suci") {
			t.Errorf("path: %v", r.URL.Path)
		}
		if r.Method != http.MethodPost {
			t.Errorf("method: %v", r.Method)
		}
		if r.Body == nil {
			t.Errorf("expected non-nil (NoBody) body")
		}
		// N5: body must be empty.
		var buf [16]byte
		n, _ := r.Body.Read(buf[:])
		if n != 0 {
			t.Errorf("N5: expected NoBody, read %d bytes", n)
		}
		w.Header().Set("3gpp-Sbi-Supi", "imsi-001010000000001")
		_, _ = w.Write([]byte("{}"))
	}))
	defer ts.Close()

	// NRF stub.
	nrf := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"access_token": "t", "expires_in": 3600})
	}))
	defer nrf.Close()

	c := NewNudmSDMClient(ts.URL, NewNrfClient(nrf.URL, "nf-1", "gmlc", ""))
	supi, err := c.DeconcealSUCI(context.Background(), "suci-0-310-150-0-0-1234567890")
	if err != nil {
		t.Fatalf("DeconcealSUCI: %v", err)
	}
	if supi != "imsi-001010000000001" {
		t.Errorf("supi: %v", supi)
	}
}

func TestUdmGetAMDataPathEscape(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !strings.HasPrefix(r.URL.Path, "/nudm-sdm/v2/imsi-001010000000001/am-data") {
			t.Errorf("path: %v", r.URL.Path)
		}
		_ = json.NewEncoder(w).Encode(map[string]any{
			"gpsis": []string{"msisdn-5511999990001"},
			"nssai": []map[string]any{{"sst": 1, "sd": "010203"}},
		})
	}))
	defer ts.Close()
	nrf := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"access_token": "t", "expires_in": 3600})
	}))
	defer nrf.Close()

	c := NewNudmSDMClient(ts.URL, NewNrfClient(nrf.URL, "nf-1", "gmlc", ""))
	am, err := c.GetAMData(context.Background(), "001010000000001", &PlmnId{Mcc: "310", Mnc: "150"})
	if err != nil {
		t.Fatalf("GetAMData: %v", err)
	}
	if am == nil || len(am.Gpsis) != 1 {
		t.Fatalf("amData: %+v", am)
	}
}

func TestAmfProvidePositioningInfo(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			t.Errorf("method: %v", r.Method)
		}
		var body map[string]string
		_ = json.NewDecoder(r.Body).Decode(&body)
		// F4: reqLocType mandatory.
		if body["reqLocType"] != "CURRENT_LOCATION" {
			t.Errorf("F4: missing reqLocType, got %v", body)
		}
		if !strings.Contains(r.URL.Path, "provide-pos-info") {
			t.Errorf("path: %v", r.URL.Path)
		}
		_ = json.NewEncoder(w).Encode(map[string]any{
			"locationInfo": map[string]any{
				"cellId": "460-00-1234",
				"tac":    "1a2b",
				"plmnId": map[string]string{"mcc": "310", "mnc": "150"},
				"ageOfLocationInfo": 12,
			},
		})
	}))
	defer ts.Close()
	nrf := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"access_token": "t", "expires_in": 3600})
	}))
	defer nrf.Close()

	c := NewNamfLocationClient(ts.URL, NewNrfClient(nrf.URL, "nf-1", "gmlc", ""))
	loc, err := c.ProvidePositioningInfo(context.Background(), "imsi-001010000000001")
	if err != nil {
		t.Fatalf("ProvidePositioningInfo: %v", err)
	}
	if loc.LocationInfo.CellId != "460-00-1234" {
		t.Errorf("cellId: %v", loc.LocationInfo.CellId)
	}
}
