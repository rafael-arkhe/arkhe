package sbi

// nrf.go implements the NRF OAuth2 token client (TS 29.510).
//
// Fix N1: the auth request body is built via a typed struct (no raw string
// concatenation), and the token TTL is guarded against underflow.

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"sync"
	"time"
)

// authRequest is the typed NRF token request body (N1).
type authRequest struct {
	NfInstanceID string `json:"nfInstanceId"`
	GrantType    string `json:"grant_type"`
	NfType       string `json:"nfType"`
	Scope        string `json:"scope"`
}

type tokenResponse struct {
	AccessToken string `json:"access_token"`
	ExpiresIn   int    `json:"expires_in"`
}

// NrfClient is a thread-safe OAuth2 client-credentials client for the NRF.
type NrfClient struct {
	httpClient   *http.Client
	baseURL      string
	nfInstanceID string
	nfType       string
	clientSecret string

	mu          sync.RWMutex
	accessToken string
	expiresAt   time.Time
}

// NewNrfClient builds an NRF OAuth2 client (F1: clientSecret included).
func NewNrfClient(baseURL, nfInstanceID, nfType, clientSecret string) *NrfClient {
	return &NrfClient{
		httpClient: &http.Client{
			Transport: &http.Transport{TLSClientConfig: GetTLSConfig()},
			Timeout:   2 * time.Second,
		},
		baseURL:      baseURL,
		nfInstanceID: nfInstanceID,
		nfType:       nfType,
		clientSecret: clientSecret,
	}
}

// GetToken returns a cached access token, refreshing when expired or absent.
func (c *NrfClient) GetToken(ctx context.Context, scope string) (string, error) {
	c.mu.RLock()
	if c.accessToken != "" && time.Now().Before(c.expiresAt) {
		token := c.accessToken
		c.mu.RUnlock()
		return token, nil
	}
	c.mu.RUnlock()
	return c.refreshToken(ctx, scope)
}

func (c *NrfClient) refreshToken(ctx context.Context, scope string) (string, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	if c.accessToken != "" && time.Now().Before(c.expiresAt) {
		return c.accessToken, nil
	}

	// N1: JSON seguro via struct, não concatenação de strings.
	reqBody := authRequest{
		NfInstanceID: c.nfInstanceID,
		GrantType:    "client_credentials",
		NfType:       c.nfType,
		Scope:        scope,
	}
	bodyBytes, err := json.Marshal(reqBody)
	if err != nil {
		return "", fmt.Errorf("falha ao serializar auth request: %w", err)
	}

	url := fmt.Sprintf("%s/oauth2/token", c.baseURL)
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(bodyBytes))
	if err != nil {
		return "", fmt.Errorf("NRF request build failed: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "application/json")

	if c.clientSecret != "" {
		req.SetBasicAuth(c.nfInstanceID, c.clientSecret)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return "", fmt.Errorf("NRF request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("NRF returned %d", resp.StatusCode)
	}

	var tr tokenResponse
	if err := json.NewDecoder(resp.Body).Decode(&tr); err != nil {
		return "", err
	}

	// N1: TTL seguro sem underflow.
	expirySec := time.Duration(tr.ExpiresIn) * time.Second
	buffer := 60 * time.Second
	if expirySec <= buffer {
		buffer = 0
	}
	c.expiresAt = time.Now().Add(expirySec - buffer)
	c.accessToken = tr.AccessToken

	return c.accessToken, nil
}
