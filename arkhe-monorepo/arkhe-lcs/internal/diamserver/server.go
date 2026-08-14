// Package diamserver implements the Diameter server side of the ARKHE-LCS
// control plane: it accepts CER/CEA, DWR/DWA, and dispatches S6a (ULR/ULA,
// AIR/AIA) and SLg (PLR/PLA) commands to the udm.Client and ring pipeline.
package diamserver

import (
	"context"
	"errors"
	"io"
	"log"
	"net"
	"sync"
	"sync/atomic"
	"time"

	"arkhe-lcs/internal/ring"
	"arkhe-lcs/internal/udm"
	"arkhe-lcs/pkg/diameter"
	"arkhe-lcs/pkg/throttle"
)

// Handler resolves subscriber data through a udm.Client.
type Handler struct {
	Client   udm.Client
	Ring     *ring.Buffer
	Capacity float64
	Origin   string
	Realm    string

	mu     sync.Mutex
	tps    map[net.Addr]*meter
	served atomic.Int64
}

type meter struct {
	t    time.Time
	cnt  int
	rate float64
}

// NewHandler returns a configured Diameter handler.
func NewHandler(client udm.Client, rb *ring.Buffer, capacity float64, origin, realm string) *Handler {
	return &Handler{
		Client:   client,
		Ring:     rb,
		Capacity: capacity,
		Origin:   origin,
		Realm:    realm,
		tps:      make(map[net.Addr]*meter),
	}
}

// Served returns the number of handled commands.
func (h *Handler) Served() int64 { return h.served.Load() }

// Serve accepts a single Diameter peer connection and answers commands.
func (h *Handler) Serve(conn net.Conn) {
	defer conn.Close()
	_ = conn.SetDeadline(time.Now().Add(2 * time.Minute))

	// CER/CEA handshake.
	if err := h.handshake(conn); err != nil {
		return
	}

	parser := diameter.NewFrameParser()
	for {
		_ = conn.SetDeadline(time.Now().Add(2 * time.Minute))
		msgs, err := parser.Read(conn)
		if err != nil {
			if err == io.EOF {
				return
			}
			if ne, ok := err.(net.Error); ok && ne.Timeout() {
				continue
			}
			return
		}
		for _, m := range msgs {
			h.dispatch(conn, m)
		}
	}
}

func (h *Handler) handshake(conn net.Conn) error {
	parser := diameter.NewFrameParser()
	for {
		_ = conn.SetDeadline(time.Now().Add(15 * time.Second))
		msgs, err := parser.Read(conn)
		if err != nil {
			return err
		}
		for _, m := range msgs {
			if m.Header.CommandCode == diameter.CmdCapabilitiesExchange && m.Header.IsRequest() {
				cea := diameter.NewCEA(m, 2001, h.Origin, h.Realm)
				b, err := cea.Encode()
				if err != nil {
					return err
				}
				_, err = conn.Write(b)
				return err
			}
		}
	}
}

func (h *Handler) dispatch(conn net.Conn, m *diameter.Message) {
	h.served.Add(1)
	h.meter(conn.RemoteAddr()).inc()

	switch {
	case m.Header.CommandCode == diameter.CmdDeviceWatchdog && m.Header.IsRequest():
		h.write(conn, diameter.NewDWA(m))
	case m.Header.CommandCode == diameter.CmdUpdateLocation && m.Header.IsRequest():
		h.handleULR(conn, m)
	case m.Header.CommandCode == diameter.CmdAuthenticationInfo && m.Header.IsRequest():
		h.handleAIR(conn, m)
	case m.Header.CommandCode == diameter.CmdProvideLocation && m.Header.IsRequest():
		h.handlePLR(conn, m)
	default:
		log.Printf("diamserver: unhandled cmd %d", m.Header.CommandCode)
	}
}

func (h *Handler) handleULR(conn net.Conn, m *diameter.Message) {
	imsi := m.GetString(diameter.AVPUserIdentity)
	if imsi == "" {
		log.Printf("diamserver: ULR without IMSI")
		return
	}
	_ = h.Ring.Push(struct{ Command, IMSI string }{Command: "ULR", IMSI: imsi})

	sd, err := h.Client.GetSubscriberData(context.Background(), imsi)
	rc := uint32(5001) // DIAMETER_ERROR_USER_UNKNOWN
	msisdn := ""
	if err == nil {
		rc = 2001
		msisdn = sd.MSISDN
	}
	ula := &diameter.Message{
		Header: diameter.Header{
			Flags:         diameter.FlagProxiable,
			CommandCode:   diameter.CmdUpdateLocation,
			ApplicationID: diameter.AppS6a,
			HopByHop:      m.Header.HopByHop,
			EndToEnd:      m.Header.EndToEnd,
		},
		AVPs: []diameter.AVP{
			diameter.NewAVP(diameter.AVPResultCode, itoa(rc)),
			diameter.NewAVP(diameter.AVPOriginHost, h.Origin),
			diameter.NewAVP(diameter.AVPOriginRealm, h.Realm),
			diameter.NewAVP(1401, msisdn), // MSISDN
		},
	}
	h.write(conn, ula)
}

func (h *Handler) handleAIR(conn net.Conn, m *diameter.Message) {
	imsi := m.GetString(diameter.AVPUserIdentity)
	if imsi == "" {
		return
	}
	snName := m.GetString(1465) // Serving-Network
	_ = h.Ring.Push(struct{ Command, IMSI string }{Command: "AIR", IMSI: imsi})

	vecs, err := h.Client.AuthenticationInformation(context.Background(), imsi, snName, 1)
	rc := uint32(5001)
	var autn, xres, randv string
	if err == nil && len(vecs) > 0 {
		rc = 2001
		autn, xres, randv = vecs[0].Autn, vecs[0].Xres, vecs[0].Rand
	}
	// Note: full AIR/AIA carries structured E-UTRAN-Vector payloads; here we
	// expose the vector fields as diagnostic AVPs to keep the codec simple.
	aia := &diameter.Message{
		Header: diameter.Header{
			Flags:         diameter.FlagProxiable,
			CommandCode:   diameter.CmdAuthenticationInfo,
			ApplicationID: diameter.AppS6a,
			HopByHop:      m.Header.HopByHop,
			EndToEnd:      m.Header.EndToEnd,
		},
		AVPs: []diameter.AVP{
			diameter.NewAVP(diameter.AVPResultCode, itoa(rc)),
			diameter.NewAVP(diameter.AVPOriginHost, h.Origin),
			diameter.NewAVP(diameter.AVPOriginRealm, h.Realm),
			diameter.NewAVP(1413, autn), // Authentication-Info (diag)
			diameter.NewAVP(1411, xres), // E-UTRAN-Vector (diag)
			diameter.NewAVP(1471, randv), // RES-Sync (diag)
		},
	}
	h.write(conn, aia)
}

func (h *Handler) handlePLR(conn net.Conn, m *diameter.Message) {
	msisdn := m.GetString(1401) // MSISDN
	_ = h.Ring.Push(struct{ Command, MSISDN string }{Command: "PLR", MSISDN: msisdn})
	pla := &diameter.Message{
		Header: diameter.Header{
			Flags:         diameter.FlagProxiable,
			CommandCode:   diameter.CmdProvideLocation,
			ApplicationID: diameter.AppSLg,
			HopByHop:      m.Header.HopByHop,
			EndToEnd:      m.Header.EndToEnd,
		},
		AVPs: []diameter.AVP{
			diameter.NewAVP(diameter.AVPResultCode, itoa(2001)),
			diameter.NewAVP(diameter.AVPOriginHost, h.Origin),
			diameter.NewAVP(diameter.AVPOriginRealm, h.Realm),
		},
	}
	h.write(conn, pla)
}

func (h *Handler) write(conn net.Conn, m *diameter.Message) {
	b, err := m.Encode()
	if err != nil {
		log.Printf("diamserver: encode: %v", err)
		return
	}
	_, _ = conn.Write(b)
}

func (h *Handler) meter(addr net.Addr) *meter {
	h.mu.Lock()
	defer h.mu.Unlock()
	m, ok := h.tps[addr]
	if !ok {
		m = &meter{t: time.Now()}
		h.tps[addr] = m
	}
	return m
}

func (m *meter) inc() {
	now := time.Now()
	el := now.Sub(m.t)
	m.cnt++
	if el >= time.Second {
		m.rate = float64(m.cnt) / el.Seconds()
		m.cnt, m.t = 0, now
	}
}

// ThrottleFactor returns the current pipeline throttle factor.
func (h *Handler) ThrottleFactor() float64 {
	h.mu.Lock()
	defer h.mu.Unlock()
	var total float64
	for _, m := range h.tps {
		total += m.rate
	}
	return throttle.Factor(total, h.Capacity)
}

func itoa(v uint32) string {
	if v == 0 {
		return "0"
	}
	var b [12]byte
	i := len(b)
	for v > 0 {
		i--
		b[i] = byte('0' + v%10)
		v /= 10
	}
	return string(b[i:])
}

var errStopped = errors.New("diamserver: stopped")
