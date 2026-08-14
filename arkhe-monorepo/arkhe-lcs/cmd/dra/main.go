// Command dra is a pure-Go Diameter routing agent for local testing. The
// production DRA is the C leg (c/dra.c, SCTP to AMF); this binary mirrors the
// same routing behavior over TCP so the Go stack can be exercised end to end
// on platforms without SCTP.
//
// Usage:
//
//	dra -listen :3869 -upstream :3868
//
// It accepts Diameter peers on -listen and relays CER/ULR/AIR/PLR to the
// udm-bridge on -upstream, echoing the answers back.
package main

import (
	"flag"
	"log"
	"net"
	"time"

	"arkhe-lcs/pkg/diameter"
)

func main() {
	listen := flag.String("listen", ":3869", "listen address for downstream peers")
	upstream := flag.String("upstream", ":3868", "udm-bridge Diameter address")
	flag.Parse()

	ln, err := net.Listen("tcp", *listen)
	if err != nil {
		log.Fatalf("listen: %v", err)
	}
	log.Printf("dra: relaying %s -> %s", *listen, *upstream)
	for {
		conn, err := ln.Accept()
		if err != nil {
			log.Fatalf("accept: %v", err)
		}
		go relay(conn, *upstream)
	}
}

func relay(downstream net.Conn, upstreamAddr string) {
	defer downstream.Close()

	up, err := net.DialTimeout("tcp", upstreamAddr, 5*time.Second)
	if err != nil {
		log.Printf("dra: upstream dial: %v", err)
		return
	}
	defer up.Close()

	// Bidirectional frame relay.
	errCh := make(chan error, 2)
	go pipe(upstreamAddr, upstreamPair{in: downstream, out: up}, errCh)
	go pipe(upstreamAddr, upstreamPair{in: up, out: downstream}, errCh)
	<-errCh
}

type upstreamPair struct {
	in  net.Conn
	out net.Conn
}

// pipe reads Diameter frames from in and writes them to out, preserving the
// framing boundaries so the relay is transparent.
func pipe(name string, p upstreamPair, errCh chan<- error) {
	parser := diameter.NewFrameParser()
	buf := make([]byte, 8192)
	for {
		_ = p.in.SetReadDeadline(time.Now().Add(30 * time.Second))
		n, err := p.in.Read(buf)
		if err != nil || n == 0 {
			errCh <- err
			return
		}
		msgs, perr := parser.Feed(buf[:n])
		if perr != nil {
			log.Printf("dra[%s]: parse: %v", name, perr)
			errCh <- perr
			return
		}
		for _, m := range msgs {
			frame, ferr := m.Encode()
			if ferr != nil {
				log.Printf("dra[%s]: encode: %v", name, ferr)
				continue
			}
			if _, werr := p.out.Write(frame); werr != nil {
				errCh <- werr
				return
			}
		}
	}
}
