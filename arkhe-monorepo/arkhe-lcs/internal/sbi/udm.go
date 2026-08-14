package sbi

// udm.go implements the UDM SBI clients per TS 29.503:
//   - Nudm_UECM DeconcealSUCI (F2: path-escaped SUCI; N5: NoBody)
//   - Nudm_SDM GetAMData (F3: query-escaped plmn-id; F6: checked json.Marshal)

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"strings"
	"time"
)

// NudmSDMClient talks to a real UDM over SBI.
type NudmSDMClient struct {
	httpClient *http.Client
	baseURL    string
	nrf        *NrfClient
}

// NewNudmSDMClient returns an SBI client for the given UDM base URL.
func NewNudmSDMClient(baseURL string, nrfClient *NrfClient) *NudmSDMClient {
	return &NudmSDMClient{
		httpClient: &http.Client{
			Transport: &http.Transport{TLSClientConfig: GetTLSConfig()},
			Timeout:   50 * time.Millisecond,
		},
		baseURL: baseURL,
		nrf:     nrfClient,
	}
}

// DeconcealSUCI resolves a SUPI from a SUCI via Nudm_UECM (F2, N5).
func (c *NudmSDMClient) DeconcealSUCI(ctx context.Context, suci string) (string, error) {
	token, err := c.nrf.GetToken(ctx, "nudm-uecm")
	if err != nil {
		return "", err
	}

	// F2: PathEscape for unsafe characters in the SUCI.
	safeSUCI := url.PathEscape(suci)
	urlStr := fmt.Sprintf("%s/nudm-uecm/v1/%s/deconceal-suci", c.baseURL, safeSUCI)

	// N5: NoBody em vez de "{}".
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, urlStr, http.NoBody)
	if err != nil {
		return "", fmt.Errorf("UDM request build failed: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+token)
	req.Header.Set("3gpp-Sbi-Originator", "arkhe-lcs-gmlc")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("UDM deconceal returned %d", resp.StatusCode)
	}

	if supi := resp.Header.Get("3gpp-Sbi-Supi"); supi != "" {
		return supi, nil
	}
	var res struct {
		Supi string `json:"supi"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&res); err != nil {
		return "", err
	}
	return res.Supi, nil
}

// GetAMData fetches access & mobility subscription data (Nudm_SDM_Get).
func (c *NudmSDMClient) GetAMData(ctx context.Context, supi string, plmnId *PlmnId) (*AccessAndMobilitySubscriptionData, error) {
	if !strings.HasPrefix(supi, "imsi-") {
		supi = "imsi-" + supi
	}

	token, err := c.nrf.GetToken(ctx, "nudm-sdm")
	if err != nil {
		return nil, err
	}

	// F3: build the URL through url.Parse and set the query via url.Values.
	u, err := url.Parse(fmt.Sprintf("%s/nudm-sdm/v2/%s/am-data", c.baseURL, supi))
	if err != nil {
		return nil, fmt.Errorf("falha ao parsear URL: %w", err)
	}

	if plmnId != nil {
		// F6: json.Marshal error is checked.
		plmnJSON, err := json.Marshal(plmnId)
		if err != nil {
			return nil, fmt.Errorf("falha ao serializar PLMN ID: %w", err)
		}
		q := u.Query()
		q.Set("plmn-id", string(plmnJSON))
		u.RawQuery = q.Encode()
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, u.String(), nil)
	if err != nil {
		return nil, fmt.Errorf("UDM request build failed: %w", err)
	}
	req.Header.Set("Accept", "application/json")
	req.Header.Set("Authorization", "Bearer "+token)
	req.Header.Set("3gpp-Sbi-Originator", "arkhe-lcs-gmlc")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	var amData AccessAndMobilitySubscriptionData
	if err := json.NewDecoder(resp.Body).Decode(&amData); err != nil {
		return nil, err
	}
	return &amData, nil
}
