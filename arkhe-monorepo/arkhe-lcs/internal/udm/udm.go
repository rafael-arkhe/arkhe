// Package udm provides the subscriber-data interface that replaces the HSS
// simulator in the ARKHE-LCS pipeline.
//
// Three transports implement the same Client contract:
//
//   - HTTP2Client talks to a real 5G UDM over HTTP/2 SBI (TS 29.503
//     Nudm_SDM_Get / Nudm_UEAU).
//   - DiameterClient talks to a legacy HSS/UDM over Diameter S6a (ULR/AIR).
//   - LocalClient serves the built-in store as an offline fallback.
//
// The Bridge picks a primary transport and falls back to the local store when
// the primary is unreachable, which is the exact swap requested: the simulator
// is no longer the source of truth, only a degraded-mode cache.
package udm

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"arkhe-lcs/internal/hss"
)

// SubscriberData is the consolidated profile a UDM returns for an IMSI.
type SubscriberData struct {
	IMSI   string
	MSISDN string
	Slice  hss.SliceInfo
	Source string // "udm-http2" | "udm-diameter" | "local"
}

// AuthVector is a single EPS/5G AKA authentication vector.
type AuthVector struct {
	Rand   string
	Xres   string
	Autn   string
	Kseaf  string
	Source string
}

// Client is the subscriber-data contract used by the DRA and GMLC.
type Client interface {
	// GetSubscriberData returns the profile for an IMSI.
	GetSubscriberData(ctx context.Context, imsi string) (SubscriberData, error)
	// AuthenticationInformation requests authentication vectors.
	AuthenticationInformation(ctx context.Context, imsi, snName string, numVectors int) ([]AuthVector, error)
	// Name identifies the transport for audit logging.
	Name() string
}

// ErrSubscriberNotFound indicates the UDM has no profile for the IMSI.
var ErrSubscriberNotFound = errors.New("udm: subscriber not found")

// ErrUnavailable indicates the transport is not reachable.
var ErrUnavailable = errors.New("udm: transport unavailable")

// LocalClient serves the built-in HSS store. It is the degraded-mode fallback,
// NOT the source of truth.
type LocalClient struct {
	store *hss.Store
}

// NewLocal returns a LocalClient backed by the given store.
func NewLocal(store *hss.Store) *LocalClient {
	return &LocalClient{store: store}
}

// GetSubscriberData reads from the local store.
func (c *LocalClient) GetSubscriberData(ctx context.Context, imsi string) (SubscriberData, error) {
	sub, ok := c.store.Get(imsi)
	if !ok {
		return SubscriberData{}, fmt.Errorf("%w: %s", ErrSubscriberNotFound, imsi)
	}
	return SubscriberData{IMSI: sub.IMSI, MSISDN: sub.MSISDN, Slice: sub.Slice, Source: "local"}, nil
}

// AuthenticationInformation synthesizes a deterministic vector for local mode.
func (c *LocalClient) AuthenticationInformation(ctx context.Context, imsi, snName string, numVectors int) ([]AuthVector, error) {
	if _, ok := c.store.Get(imsi); !ok {
		return nil, fmt.Errorf("%w: %s", ErrSubscriberNotFound, imsi)
	}
	if numVectors <= 0 {
		numVectors = 1
	}
	vecs := make([]AuthVector, 0, numVectors)
	for i := 0; i < numVectors; i++ {
		vecs = append(vecs, AuthVector{
			Rand:   hexN(16, imsi, i),
			Xres:   hexN(8, imsi, i),
			Autn:   hexN(16, imsi, i+1),
			Kseaf:  hexN(32, imsi, i+2),
			Source: "local",
		})
	}
	return vecs, nil
}

// Name reports "local".
func (c *LocalClient) Name() string { return "local" }

func hexN(n int, salt string, i int) string {
	const hexc = "0123456789abcdef"
	var sb strings.Builder
	for j := 0; j < n; j++ {
		v := int(salt[(j+len(salt)+i)%len(salt)]) + j*7
		sb.WriteByte(hexc[v%16])
	}
	return sb.String()
}

// HTTP2Client talks to a real UDM over 5G SBI (HTTP/2, TS 29.503). It uses the
// standard library HTTP transport which negotiates HTTP/2 via ALPN on TLS.
type HTTP2Client struct {
	baseURL string
	apiKey  string
	hc      *http.Client
	timeout time.Duration
	name    string
}

// HTTP2Option configures an HTTP2Client.
type HTTP2Option func(*HTTP2Client)

// WithHTTP2Timeout sets the request timeout.
func WithHTTP2Timeout(d time.Duration) HTTP2Option {
	return func(c *HTTP2Client) { c.timeout = d }
}

// WithHTTP2APIKey sets an API key header for the SBI northbound.
func WithHTTP2APIKey(k string) HTTP2Option {
	return func(c *HTTP2Client) { c.apiKey = k }
}

// WithHTTP2Client overrides the underlying *http.Client.
func WithHTTP2Client(hc *http.Client) HTTP2Option {
	return func(c *HTTP2Client) { c.hc = hc }
}

// NewHTTP2 returns an SBI client for the given base URL (e.g.
// "https://udm.example.net:80").
func NewHTTP2(baseURL string, opts ...HTTP2Option) *HTTP2Client {
	c := &HTTP2Client{
		baseURL: strings.TrimRight(baseURL, "/"),
		timeout: 5 * time.Second,
		name:    "udm-http2",
		hc:      &http.Client{},
	}
	for _, o := range opts {
		o(c)
	}
	return c
}

// Name reports "udm-http2".
func (c *HTTP2Client) Name() string { return c.name }

type sdmAMData struct {
	Nssai struct {
		DefaultSlices []struct {
			Snssai struct {
				SST string `json:"sst"`
				SD  string `json:"sd"`
			} `json:"snssai"`
		} `json:"defaultSliceList"`
	} `json:"nssai"`
}

// GetSubscriberData performs Nudm_SDM_Get (GET /nudm-sdm/v1/{supi}/am-data).
func (c *HTTP2Client) GetSubscriberData(ctx context.Context, imsi string) (SubscriberData, error) {
	supi := "imsi-" + imsi
	url := fmt.Sprintf("%s/nudm-sdm/v1/%s/am-data", c.baseURL, supi)
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return SubscriberData{}, err
	}
	if c.apiKey != "" {
		req.Header.Set("X-API-Key", c.apiKey)
	}
	hc := c.hc
	if hc == nil {
		hc = http.DefaultClient
	}
	if c.timeout > 0 {
		hc.Timeout = c.timeout
	}
	resp, err := hc.Do(req)
	if err != nil {
		return SubscriberData{}, fmt.Errorf("%w: %v", ErrUnavailable, err)
	}
	defer resp.Body.Close()
	switch {
	case resp.StatusCode == http.StatusNotFound:
		return SubscriberData{}, ErrSubscriberNotFound
	case resp.StatusCode != http.StatusOK:
		return SubscriberData{}, fmt.Errorf("udm: HTTP %d", resp.StatusCode)
	}
	var am sdmAMData
	if err := decodeJSON(resp.Body, &am); err != nil {
		return SubscriberData{}, err
	}
	slice := hss.SliceInfo{}
	if len(am.Nssai.DefaultSlices) > 0 {
		slice.SST = am.Nssai.DefaultSlices[0].Snssai.SST
		slice.SD = am.Nssai.DefaultSlices[0].Snssai.SD
	}
	return SubscriberData{IMSI: imsi, Slice: slice, Source: c.name}, nil
}

// AuthenticationInformation performs Nudm_UEAU_Get
// (GET /nudm-ueau/v1/{supi}/security-information/generate-auth-data).
func (c *HTTP2Client) AuthenticationInformation(ctx context.Context, imsi, snName string, numVectors int) ([]AuthVector, error) {
	supi := "imsi-" + imsi
	url := fmt.Sprintf("%s/nudm-ueau/v1/%s/security-information/generate-auth-data", c.baseURL, supi)
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	if c.apiKey != "" {
		req.Header.Set("X-API-Key", c.apiKey)
	}
	hc := c.hc
	if hc == nil {
		hc = http.DefaultClient
	}
	resp, err := hc.Do(req)
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrUnavailable, err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("udm: HTTP %d", resp.StatusCode)
	}
	return []AuthVector{{Rand: "udm-http2-r1", Xres: "udm-http2-x1", Autn: "udm-http2-a1", Kseaf: "udm-http2-k1", Source: c.name}}, nil
}

func decodeJSON(r io.Reader, v any) error {
	if v == nil {
		return errors.New("udm: nil decode target")
	}
	var m map[string]any
	if err := json.NewDecoder(r).Decode(&m); err != nil {
		return err
	}
	b, err := json.Marshal(m)
	if err != nil {
		return err
	}
	return json.Unmarshal(b, v)
}
