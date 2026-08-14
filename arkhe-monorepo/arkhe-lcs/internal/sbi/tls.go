// Package sbi implements the 5G Service-Based Interface (SBI) clients used by
// the ARKHE-LCS GMLC over HTTP/2 + mTLS: NRF OAuth2 (TS 29.510), UDM
// (TS 29.503) and AMF NAmf_Location (TS 29.518).
package sbi

import (
	"crypto/tls"
	"crypto/x509"
	"errors"
	"fmt"
	"os"
	"sync"
)

var (
	tlsOnce sync.Once
	tlsConf *tls.Config
	tlsErr  error
)

// GetTLSConfig returns the process-wide SBI client TLS configuration, with
// mTLS applied when InitMTLS was called successfully.
func GetTLSConfig() *tls.Config {
	tlsOnce.Do(func() {
		tlsConf = &tls.Config{
			MinVersion: tls.VersionTLS12,
		}
	})
	return tlsConf
}

// InitMTLS loads the client certificate, key and CA bundle and configures the
// process-wide SBI TLS config for mutual TLS. It is idempotent; the first
// successful call wins.
func InitMTLS(certFile, keyFile, caFile string) error {
	var initErr error
	tlsOnce.Do(func() {
		if certFile == "" || keyFile == "" || caFile == "" {
			tlsErr = errors.New("sbi: cert/key/ca all required for mTLS")
			return
		}
		cert, err := tls.LoadX509KeyPair(certFile, keyFile)
		if err != nil {
			tlsErr = fmt.Errorf("sbi: load keypair: %w", err)
			return
		}
		pool := x509.NewCertPool()
		pem, err := os.ReadFile(caFile)
		if err != nil {
			tlsErr = fmt.Errorf("sbi: read CA: %w", err)
			return
		}
		if !pool.AppendCertsFromPEM(pem) {
			tlsErr = errors.New("sbi: no CA certs appended")
			return
		}
		tlsConf = &tls.Config{
			MinVersion:   tls.VersionTLS12,
			Certificates: []tls.Certificate{cert},
			RootCAs:      pool,
		}
	})
	if tlsConf == nil {
		return tlsErr
	}
	return initErr
}

// ServerTLSConfig builds the server-side TLS config forcing HTTP/2 via ALPN.
func ServerTLSConfig() (*tls.Config, error) {
	client := GetTLSConfig()
	if client == nil || len(client.Certificates) == 0 {
		return nil, errors.New("sbi: mTLS not initialized (no server certificate)")
	}
	return &tls.Config{
		Certificates: client.Certificates,
		NextProtos:   []string{"h2"},
		MinVersion:   tls.VersionTLS12,
	}, nil
}
