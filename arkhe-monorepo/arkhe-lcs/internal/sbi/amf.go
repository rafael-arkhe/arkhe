package sbi

// amf.go implements the NAmf_Location client (TS 29.518).
//
// Fix F4: reqLocType is mandatory in the ProvidePositioningInfo payload.
// Fix N4: the ueContextId is path-escaped.
// Fix N6: the constructor exists.

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"time"
)

// NamfLocationClient talks to the AMF NAmf_Location service.
type NamfLocationClient struct {
	httpClient *http.Client
	baseURL    string
	nrf        *NrfClient
}

// NewNamfLocationClient returns a client for the given AMF base URL (N6).
func NewNamfLocationClient(baseURL string, nrfClient *NrfClient) *NamfLocationClient {
	return &NamfLocationClient{
		httpClient: &http.Client{
			Transport: &http.Transport{TLSClientConfig: GetTLSConfig()},
			Timeout:   200 * time.Millisecond,
		},
		baseURL: baseURL,
		nrf:     nrfClient,
	}
}

// ProvidePositioningInfo requests the UE location from the AMF (F4, N4).
func (c *NamfLocationClient) ProvidePositioningInfo(ctx context.Context, ueContextId string) (*NamfLocResp, error) {
	token, err := c.nrf.GetToken(ctx, "namf-loc")
	if err != nil {
		return nil, fmt.Errorf("NRF token failed: %w", err)
	}

	// N4: PathEscape do ueContextId.
	safeUE := url.PathEscape(ueContextId)

	// F4: reqLocType is mandatory in the payload.
	reqBody := struct {
		ReqLocType string `json:"reqLocType"`
	}{
		ReqLocType: "CURRENT_LOCATION",
	}
	bodyBytes, err := json.Marshal(reqBody)
	if err != nil {
		return nil, fmt.Errorf("falha ao serializar payload: %w", err)
	}

	urlStr := fmt.Sprintf("%s/namf-loc/v1/%s/provide-pos-info", c.baseURL, safeUE)

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, urlStr, bytes.NewReader(bodyBytes))
	if err != nil {
		return nil, fmt.Errorf("AMF request build failed: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "application/json")
	req.Header.Set("Authorization", "Bearer "+token)
	req.Header.Set("3gpp-Sbi-Originator", "arkhe-lcs-gmlc")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusAccepted {
		return nil, fmt.Errorf("AMF returned %d", resp.StatusCode)
	}

	var locResp NamfLocResp
	if err := json.NewDecoder(resp.Body).Decode(&locResp); err != nil {
		return nil, err
	}
	return &locResp, nil
}
