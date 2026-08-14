// Command udm-bridge replaces the HSS simulator with an interface to a real
// UDM (HTTP/2 SBI TS 29.503, or Diameter S6a), falling back to the local HSS
// store only when the UDM is unreachable.
//
// It exposes:
//   - a Diameter server (S6a ULR/AIR, SLg PLR) on :3868 for the DRA/AMF leg
//   - a GMLC northbound HTTP API on :8080
//
// Usage:
//
//	udm-bridge -listen :3868 -gmlc :8080 -udm http2://udm.example.net \
//	           -hss subscribers.json
package main

import (
	"context"
	"encoding/json"
	"flag"
	"log"
	"net"
	"net/http"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"arkhe-lcs/api"
	"arkhe-lcs/internal/diamserver"
	"arkhe-lcs/internal/hss"
	"arkhe-lcs/internal/ring"
	"arkhe-lcs/internal/udm"
)

func main() {
	listen := flag.String("listen", ":3868", "Diameter listen address (TCP)")
	gmlcAddr := flag.String("gmlc", ":8080", "GMLC HTTP API listen address")
	udmURL := flag.String("udm", "", "real UDM SBI base URL, e.g. https://udm.example.net or diameter://hss.example.net:3868")
	hssFile := flag.String("hss", "", "JSON file seeding the local HSS store (fallback)")
	capacity := flag.Float64("capacity", 1000, "pipeline capacity in TPS for throttling")
	ringSize := flag.Int("ring", 4096, "event ring buffer capacity")
	flag.Parse()

	store := hss.NewStore()
	if *hssFile != "" {
		if err := seedStore(store, *hssFile); err != nil {
			log.Fatalf("seed hss: %v", err)
		}
	}
	store.Seed(sampleSubscribers()...)

	primary := newPrimary(*udmURL)
	local := udm.NewLocal(store)
	var client udm.Client
	if primary != nil {
		client = udm.NewBridge(primary, local)
		log.Printf("udm-bridge: primary transport %s (fallback local)", primary.Name())
	} else {
		client = local
		log.Printf("udm-bridge: WARNING no real UDM configured; serving local only")
	}

	rb, err := ring.New(*ringSize)
	if err != nil {
		log.Fatalf("ring: %v", err)
	}

	// Diameter server for the DRA/AMF leg.
	handler := diamserver.NewHandler(client, rb, *capacity, "arkhe-lcs.udm", "arkhe")
	ln, err := net.Listen("tcp", *listen)
	if err != nil {
		log.Fatalf("listen: %v", err)
	}
	defer ln.Close()
	go func() {
		log.Printf("udm-bridge: Diameter server on %s", ln.Addr())
		for {
			conn, err := ln.Accept()
			if err != nil {
				return
			}
			go handler.Serve(conn)
		}
	}()

	// GMLC northbound.
	gm := api.NewGMLCServer(client)
	mux := http.NewServeMux()
	mux.HandleFunc("GET /v1/subscriber/{imsi}", gm.SubscriberHandler)
	mux.HandleFunc("GET /v1/vectors/{imsi}", gm.VectorHandler)
	srv := &http.Server{Addr: *gmlcAddr, Handler: mux, ReadHeaderTimeout: 10 * time.Second}
	go func() {
		log.Printf("udm-bridge: GMLC API on %s", *gmlcAddr)
		if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatalf("gmlc: %v", err)
		}
	}()

	// Tick reporting TPS / throttle.
	go func() {
		for {
			time.Sleep(5 * time.Second)
			log.Printf("udm-bridge: served=%d throttle=%.2f", handler.Served(), handler.ThrottleFactor())
		}
	}()

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	<-ctx.Done()
	_ = srv.Shutdown(context.Background())
	log.Printf("udm-bridge: stopped")
}

// newPrimary builds the real-UDM transport from the -udm flag, or nil.
func newPrimary(url string) udm.Client {
	switch {
	case url == "":
		return nil
	case strings.HasPrefix(url, "diameter://"):
		addr := strings.TrimPrefix(url, "diameter://")
		return udm.NewDiameter(addr, "arkhe-lcs.dra", "arkhe")
	default:
		return udm.NewHTTP2(url)
	}
}

func seedStore(store *hss.Store, path string) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	var subs []hss.Subscriber
	if err := json.Unmarshal(data, &subs); err != nil {
		return err
	}
	store.Seed(subs...)
	return nil
}

func sampleSubscribers() []hss.Subscriber {
	return []hss.Subscriber{
		{IMSI: "001010000000001", MSISDN: "5511999990001", Slice: hss.SliceInfo{SST: "1", SD: "010203"}},
		{IMSI: "001010000000002", MSISDN: "5511999990002", Slice: hss.SliceInfo{SST: "1", SD: "010204"}},
		{IMSI: "001010000000003", MSISDN: "5511999990003", Slice: hss.SliceInfo{SST: "2", SD: "010205"}},
	}
}
