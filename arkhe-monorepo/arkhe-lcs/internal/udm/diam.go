package udm

// diam.go implements the Diameter S6a transport to a legacy HSS/UDM. It speaks
// CER/CEA, DWR/DWA and ULR/ULA against a remote Diameter peer over TCP.

import (
	"context"
	"fmt"
	"io"
	"net"
	"sync"
	"time"

	"arkhe-lcs/pkg/diameter"
)

// DiameterClient connects to a Diameter peer (HSS/UDM or a DRA fronting one).
type DiameterClient struct {
	addr        string
	originHost  string
	originRealm string

	mu    sync.Mutex
	conn  net.Conn
	hbh   uint32
	ete   uint32
	ready bool
}

// NewDiameter returns a Diameter transport for the given peer address.
func NewDiameter(addr, originHost, originRealm string) *DiameterClient {
	return &DiameterClient{
		addr:        addr,
		originHost:  originHost,
		originRealm: originRealm,
		hbh:         uint32(time.Now().UnixNano()),
		ete:         uint32(time.Now().UnixNano()),
	}
}

// Name reports "udm-diameter".
func (c *DiameterClient) Name() string { return "udm-diameter" }

func (c *DiameterClient) nextIDs() (uint32, uint32) {
	c.hbh++
	c.ete++
	return c.hbh, c.ete
}

// ensureReady performs the CER/CEA handshake on first use.
func (c *DiameterClient) ensureReady(ctx context.Context) error {
	c.mu.Lock()
	defer c.mu.Unlock()
	if c.ready {
		return nil
	}
	dialer := net.Dialer{Timeout: 5 * time.Second}
	conn, err := dialer.DialContext(ctx, "tcp", c.addr)
	if err != nil {
		return fmt.Errorf("%w: dial %s: %v", ErrUnavailable, c.addr, err)
	}
	c.conn = conn
	// CER
	hbh, ete := c.nextIDs()
	cer := diameter.NewCER(c.originHost, c.originRealm, hbh, ete)
	cer.Header.Flags = diameter.FlagRequest | diameter.FlagProxiable
	frame, err := cer.Encode()
	if err != nil {
		_ = conn.Close()
		return err
	}
	if _, err := conn.Write(frame); err != nil {
		_ = conn.Close()
		return err
	}
	parser := diameter.NewFrameParser()
	for {
		if err := conn.SetReadDeadline(time.Now().Add(10 * time.Second)); err != nil {
			_ = conn.Close()
			return err
		}
		msgs, err := parser.Read(conn)
		if err != nil && err != io.EOF {
			_ = conn.Close()
			return err
		}
		for _, m := range msgs {
			if m.Header.CommandCode == diameter.CmdCapabilitiesExchange && !m.Header.IsRequest() {
				rc := m.Get(diameter.AVPResultCode)
				if rc == nil || rc.Uint32() != 2001 {
					_ = conn.Close()
					return fmt.Errorf("udm: CER rejected, result-code %v", rc)
				}
				c.ready = true
				return nil
			}
		}
		if err != nil {
			_ = conn.Close()
			return fmt.Errorf("%w: no CEA before EOF", ErrUnavailable)
		}
	}
}

// GetSubscriberData issues an S6a ULR and parses the ULA.
func (c *DiameterClient) GetSubscriberData(ctx context.Context, imsi string) (SubscriberData, error) {
	if err := c.ensureReady(ctx); err != nil {
		return SubscriberData{}, err
	}
	c.mu.Lock()
	conn := c.conn
	c.mu.Unlock()

	hbh, ete := c.nextIDs()
	ulr := diameter.NewUpdateLocationRequest(imsi, c.originHost, c.originRealm, c.originRealm, hbh, ete)
	frame, err := ulr.Encode()
	if err != nil {
		return SubscriberData{}, err
	}
	if _, err := conn.Write(frame); err != nil {
		return SubscriberData{}, fmt.Errorf("%w: write: %v", ErrUnavailable, err)
	}
	parser := diameter.NewFrameParser()
	for {
		if err := conn.SetReadDeadline(time.Now().Add(10 * time.Second)); err != nil {
			return SubscriberData{}, err
		}
		msgs, err := parser.Read(conn)
		if err != nil && err != io.EOF {
			return SubscriberData{}, err
		}
		for _, m := range msgs {
			if m.Header.CommandCode == diameter.CmdUpdateLocation && m.Header.HopByHop == hbh {
				rc := m.Get(diameter.AVPResultCode)
				if rc != nil && rc.Uint32() == 2001 {
					msisdn := ""
					if a := m.Get(1401); a != nil {
						msisdn = a.String()
					}
					return SubscriberData{IMSI: imsi, MSISDN: msisdn, Source: "udm-diameter"}, nil
				}
				return SubscriberData{}, ErrSubscriberNotFound
			}
		}
		if err == io.EOF {
			return SubscriberData{}, fmt.Errorf("%w: closed before ULA", ErrUnavailable)
		}
	}
}

// AuthenticationInformation is not implemented on the Diameter transport; callers
// use the HTTP/2 transport for 5G AKA vectors.
func (c *DiameterClient) AuthenticationInformation(ctx context.Context, imsi, snName string, numVectors int) ([]AuthVector, error) {
	return nil, fmt.Errorf("udm: 5G AKA vectors require HTTP/2 SBI transport")
}

// Close closes the underlying connection.
func (c *DiameterClient) Close() error {
	c.mu.Lock()
	defer c.mu.Unlock()
	if c.conn == nil {
		return nil
	}
	err := c.conn.Close()
	c.conn = nil
	c.ready = false
	return err
}
