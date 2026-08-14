// Command lcs-daemon is the ARKHE-LCS control-plane daemon (v0.7.0). It
// runs two independent flows:
//
//  1. Legacy 4G leg (N2): the SLgHandler consumes Diameter SLg frames written
//     by the C DRA into the shared ring and resolves them via the 5G SBI
//     clients.
//  2. 5G SBI leg (N3): the GMLC gateway exposes POST provide-location over
//     HTTP/2 + mTLS, deconcealing SUCI via the real UDM and locating UEs via
//     the real AMF. OAuth2 tokens are minted by the NRF.
package main

import (
	"context"
	"flag"
	"log"
	"os"
	"os/signal"
	"sync"
	"syscall"
	"time"

	"arkhe-lcs/api"
	"arkhe-lcs/internal/hss"
	"arkhe-lcs/internal/legacy"
	"arkhe-lcs/internal/lmf"
	"arkhe-lcs/internal/ring"
	"arkhe-lcs/internal/safety"
	"arkhe-lcs/internal/sbi"
	"arkhe-lcs/internal/sms"
)

var (
	shmName       = flag.String("shm", "/arkhe_lcs_ring", "Shared memory name")
	redisAddr     = flag.String("redis", "localhost:6379", "Redis address")
	kafkaAddr     = flag.String("kafka", "localhost:9092", "Kafka broker")
	maxTPS        = flag.Float64("max-tps", 20000, "Maximum TPS")
	throttleRatio = flag.Float64("throttle-ratio", 0.8, "Reject above this ratio")

	// 5G SBI
	nrfURL       = flag.String("nrf-url", "https://nrf.5gcore.local:8000", "NRF URL")
	udmURL       = flag.String("udm-url", "https://udm.5gcore.local:8000", "UDM URL")
	amfURL       = flag.String("amf-url", "https://amf.5gcore.local:8000", "AMF URL")
	nfInstanceID = flag.String("nf-instance-id", "", "NF Instance ID (UUID)")
	nfType       = flag.String("nf-type", "gmlc", "NF Type")
	clientSecret = flag.String("client-secret", "", "OAuth2 client secret")

	// mTLS
	certFile = flag.String("cert", "/etc/arkhe/certs/gmlc.crt", "mTLS certificate")
	keyFile  = flag.String("key", "/etc/arkhe/certs/gmlc.key", "mTLS key")
	caFile   = flag.String("ca", "/etc/arkhe/certs/ca.crt", "5G CA root")

	// GMLC HTTP/2
	gmlcAddr = flag.String("gmlc-addr", ":8443", "GMLC HTTP/2 address")
)

func main() {
	flag.Parse()

	if *nfInstanceID == "" {
		log.Fatal("nf-instance-id é obrigatório")
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// ================================================================
	// 1. mTLS (N3)
	// ================================================================
	if err := sbi.InitMTLS(*certFile, *keyFile, *caFile); err != nil {
		log.Fatalf("Falha mTLS: %v", err)
	}

	// ================================================================
	// 2. Ring Buffer
	// ================================================================
	r, err := ring.OpenShmRing(*shmName, true)
	if err != nil {
		log.Fatalf("Falha ring: %v", err)
	}
	defer r.Close()

	// ================================================================
	// 3. Clientes 5G SBI
	// ================================================================
	nrfClient := sbi.NewNrfClient(*nrfURL, *nfInstanceID, *nfType, *clientSecret)
	udmClient := sbi.NewNudmSDMClient(*udmURL, nrfClient)
	amfClient := sbi.NewNamfLocationClient(*amfURL, nrfClient)

	// ================================================================
	// 4. Componentes Core
	// ================================================================
	daemon := hss.NewHSSStateDaemon(*maxTPS, *throttleRatio)
	tpsMeter := hss.NewTPSMeter(daemon, time.Second)
	go tpsMeter.Run(ctx)

	smsRouter := sms.NewSMSRouter(*redisAddr, *kafkaAddr)
	defer smsRouter.Close()
	go smsRouter.StartCacheInvalidator(ctx)

	lmfEngine := lmf.NewPositioningEngine(lmf.NewMemoryCellDB())
	safetyKernel := safety.NewMissionSafetyKernel(daemon)
	go safetyKernel.RunHealthCheck(ctx, 5*time.Second)

	// ================================================================
	// 5. GMLC Gateway (5G SBI) — N3: HTTP/2 + mTLS
	// ================================================================
	gmlc := api.NewGMLCGateway(daemon, lmfEngine, smsRouter, r, udmClient, amfClient)
	go func() {
		log.Printf("GMLC SBI ouvindo em %s (HTTP/2 + mTLS)", *gmlcAddr)
		if err := gmlc.ServeTLS(*gmlcAddr, *certFile, *keyFile); err != nil {
			log.Printf("GMLC HTTP/2 erro: %v", err)
		}
	}()

	// ================================================================
	// 6. Legado 4G Handler (SLg) — N2: processa Diameter do Ring Buffer
	// ================================================================
	slgHandler := legacy.NewSLgHandler(daemon, udmClient, amfClient)
	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		slgHandler.ProcessRingBuffer(ctx, r)
	}()

	// ================================================================
	// 7. Shutdown Graceful
	// ================================================================
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, os.Interrupt, syscall.SIGTERM)
	<-sigCh

	log.Println("Shutting down...")
	cancel()
	wg.Wait()
	log.Println("Encerrado.")
}
