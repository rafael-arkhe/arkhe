//! Módulo de descoberta de nós DTN via varredura espectral e beacon BPv7.
//!
//! # Correções aplicadas
//! - **P1 FIX**: BPv7 é unidirecional — não usa handshake TCP-like SYN/ACK.
//!   Em vez disso, envia bundles de beacon com metadados do nó.
//! - Beacons são bundles com payload mínimo (EID + capabilities).
//! - Varredura espectral escaneia faixas de frequência por sinais de portadora.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

#[cfg(feature = "std")]
use crate::bpv7;

/// Endpoint ID (EID) BPv7 conforme RFC 9171.
/// Formato: `dtn://node-name.dsn/` ou `ipn:node_number.service_number`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Eid {
    pub scheme: EidScheme,
    pub node: String,
    pub service: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EidScheme {
    Dtn,  // dtn://
    Ipn,  // ipn:
}

impl Eid {
    /// Cria EID DTN.
    pub fn dtn(node: &str) -> Self {
        Self {
            scheme: EidScheme::Dtn,
            node: node.to_string(),
            service: None,
        }
    }

    /// Cria EID IPN.
    pub fn ipn(node: u32, service: u32) -> Self {
        Self {
            scheme: EidScheme::Ipn,
            node: node.to_string(),
            service: Some(service),
        }
    }

    /// Serializa para string.
    pub fn as_str(&self) -> String {
        match self.scheme {
            EidScheme::Dtn => format!("dtn://{}/", self.node),
            EidScheme::Ipn => format!("ipn:{}.{}", self.node, self.service.unwrap_or(0)),
        }
    }

    /// Faz parse de um EID IPN a partir de string (`ipn:node.service`).
    pub fn ipn_from_str(s: &str) -> Option<Self> {
        let rest = s.strip_prefix("ipn:")?;
        let mut parts = rest.split('.');
        let node = parts.next()?.parse::<u32>().ok()?;
        let service = match parts.next() {
            Some(v) => Some(v.parse::<u32>().ok()?),
            None => Some(0),
        };
        Some(Eid {
            scheme: EidScheme::Ipn,
            node: node.to_string(),
            service,
        })
    }
}

impl fmt::Display for Eid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Capabilities de um nó DTN (serviços anunciados).
#[derive(Debug, Clone, Default)]
pub struct NodeCapabilities {
    /// Suporta custódia de bundles.
    pub custody: bool,
    /// Suporta fragmentação.
    pub fragmentation: bool,
    /// Largura de banda máxima (bytes/segundo).
    pub max_bandwidth_bps: u64,
    /// Tamanho máximo de bundle (bytes).
    pub max_bundle_size: u64,
    /// Versão BP suportada.
    pub bp_version: u8,
}

/// Beacon BPv7 — bundle de anúncio de presença.
///
/// Conforme RFC 9171, um beacon é um bundle especial com:
/// - Source: EID do nó emissor
/// - Destination: EID de broadcast (ex: `dtn://broadcast.dsn/`)
/// - Payload: metadados do nó (capabilities, timestamp)
#[derive(Debug, Clone)]
pub struct Beacon {
    pub source: Eid,
    pub timestamp_ms: u64,
    pub capabilities: NodeCapabilities,
    /// Frequência de transmissão do beacon (Hz).
    pub tx_frequency_hz: u64,
    /// Potência de transmissão (dBm).
    pub tx_power_dbm: i16,
}

/// Resultado de varredura espectral em uma faixa de frequência.
#[derive(Debug, Clone)]
pub struct SpectralScanResult {
    /// Frequência central (Hz).
    pub frequency_hz: u64,
    /// Largura de banda analisada (Hz).
    pub bandwidth_hz: u64,
    /// Potência detectada (dBm).
    pub power_dbm: f64,
    /// SNR estimado (dB).
    pub snr_db: f64,
    /// Probabilidade de ser sinal de portadora (0..1).
    pub confidence: f64,
}

/// Serviço de descoberta de nós.
pub struct DiscoveryService {
    /// Frequências a escanear (Hz).
    pub scan_frequencies: Vec<u64>,
    /// Largura de banda de cada canal (Hz).
    pub channel_bandwidth_hz: u64,
    /// Threshold de SNR para considerar sinal válido (dB).
    pub snr_threshold_db: f64,
    /// EID do nó local.
    pub local_eid: Eid,
    /// Capabilities do nó local.
    pub local_capabilities: NodeCapabilities,
    /// EID de destino dos beacons BPv7 (broadcast).
    #[cfg(feature = "std")]
    pub beacon_destination: String,
}

impl DiscoveryService {
    /// Cria serviço de descoberta com configuração padrão.
    pub fn new(local_eid: Eid) -> Self {
        Self {
            scan_frequencies: Vec::new(),
            channel_bandwidth_hz: 1_000_000, // 1 MHz
            snr_threshold_db: 6.0,
            local_eid,
            local_capabilities: NodeCapabilities {
                custody: true,
                fragmentation: true,
                max_bandwidth_bps: 1_000_000,
                max_bundle_size: 1_000_000,
                bp_version: 7,
            },
            #[cfg(feature = "std")]
            beacon_destination: "dtn://broadcast.dsn/".to_string(),
        }
    }

    /// Adiciona frequência de escaneamento.
    pub fn add_frequency(&mut self, freq_hz: u64) {
        self.scan_frequencies.push(freq_hz);
    }

    /// Simula varredura espectral em uma frequência.
    ///
    /// Em hardware real, isto usaria SDR (Software Defined Radio).
    /// Aqui é uma simulação com ruído gaussiano.
    pub fn scan_frequency(&self, freq_hz: u64) -> SpectralScanResult {
        // Simulação: sinal presente em frequências específicas
        let (power_dbm, snr_db, confidence) = if self.is_known_beacon_frequency(freq_hz) {
            (-80.0, 12.0, 0.95)
        } else {
            // Ruído de fundo
            let noise_dbm = -120.0 + rand::random::<f64>() * 20.0;
            let snr = noise_dbm + 120.0;
            (noise_dbm, snr, 0.0)
        };

        SpectralScanResult {
            frequency_hz: freq_hz,
            bandwidth_hz: self.channel_bandwidth_hz,
            power_dbm,
            snr_db,
            confidence,
        }
    }

    /// Varredura completa de todas as frequências configuradas.
    pub fn full_scan(&self) -> Vec<SpectralScanResult> {
        self.scan_frequencies
            .iter()
            .map(|&f| self.scan_frequency(f))
            .collect()
    }

    /// Detecta beacons em resultados de varredura.
    pub fn detect_beacons(&self, results: &[SpectralScanResult]) -> Vec<u64> {
        results
            .iter()
            .filter(|r| r.snr_db >= self.snr_threshold_db && r.confidence > 0.5)
            .map(|r| r.frequency_hz)
            .collect()
    }

    /// Cria um beacon BPv7 para transmissão.
    ///
    /// Correção P1: Não há SYN/ACK em BPv7. O beacon É o mecanismo de descoberta.
    pub fn create_beacon(&self, timestamp_ms: u64) -> Beacon {
        Beacon {
            source: self.local_eid.clone(),
            timestamp_ms,
            capabilities: self.local_capabilities.clone(),
            tx_frequency_hz: 2_400_000_000, // 2.4 GHz (band ISM)
            tx_power_dbm: 20,
        }
    }

    /// Processa um beacon recebido e extrai informações do nó.
    pub fn process_beacon(&self, beacon: &Beacon) -> DiscoveredNode {
        DiscoveredNode {
            eid: beacon.source.clone(),
            capabilities: beacon.capabilities.clone(),
            last_seen_ms: beacon.timestamp_ms,
            tx_frequency_hz: beacon.tx_frequency_hz,
        }
    }

    /// Fase 1 — Cria um beacon BPv7 real, assinado com BIB-HMAC-SHA2 (RFC 9173).
    ///
    /// O payload do bundle codifica as capabilities do nó em JSON. A integridade
    /// é garantida pelo BPSec (`hardy-bpv7`), não por campo simulado.
    #[cfg(feature = "std")]
    pub fn create_bpv7_beacon(
        &self,
        timestamp_ms: u64,
        key: &hardy_bpv7::bpsec::key::Key,
    ) -> Result<Vec<u8>, bpv7::Error> {
        use serde::Serialize;

        #[derive(Serialize)]
        struct BeaconMeta<'a> {
            timestamp_ms: u64,
            node: &'a str,
            custody: bool,
            fragmentation: bool,
            max_bandwidth_bps: u64,
            max_bundle_size: u64,
            bp_version: u8,
        }

        let meta = BeaconMeta {
            timestamp_ms,
            node: &self.local_eid.node,
            custody: self.local_capabilities.custody,
            fragmentation: self.local_capabilities.fragmentation,
            max_bandwidth_bps: self.local_capabilities.max_bandwidth_bps,
            max_bundle_size: self.local_capabilities.max_bundle_size,
            bp_version: self.local_capabilities.bp_version,
        };
        let payload = serde_json::to_vec(&meta).map_err(|e| bpv7::Error::Message(e.to_string()))?;

        let source = self.local_eid.as_str();
        bpv7::sign_beacon(&source, &self.beacon_destination, &payload, key)
            .map(|s| s.wire)
    }

    /// Fase 1 — Processa um beacon BPv7 recebido, verificando o BIB.
    ///
    /// Exige que o bloco de payload esteja protegido por um BIB (`bib_present`),
    /// evitando beacons adulterados ou não assinados.
    #[cfg(feature = "std")]
    pub fn process_bpv7_beacon(
        &self,
        wire: &[u8],
        keys: &hardy_bpv7::bpsec::key::KeySet,
    ) -> Result<DiscoveredNode, bpv7::Error> {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct BeaconMeta {
            timestamp_ms: u64,
            #[serde(default)]
            node: String,
            #[serde(default)]
            custody: bool,
            #[serde(default)]
            fragmentation: bool,
            #[serde(default)]
            max_bandwidth_bps: u64,
            #[serde(default)]
            max_bundle_size: u64,
            #[serde(default)]
            bp_version: u8,
        }

        let verified = bpv7::verify_beacon(wire, keys)?;
        if !verified.bib_present {
            return Err(bpv7::Error::Message(
                "beacon sem BIB — integridade não comprovada".to_string(),
            ));
        }

        let meta: BeaconMeta = serde_json::from_slice(&verified.payload)
            .map_err(|e| bpv7::Error::Message(e.to_string()))?;

        // O EID de origem vem do bloco primário (assinado via IPPT), não do payload.
        let eid = match verified.source.strip_prefix("dtn://").map(|n| n.trim_end_matches('/')) {
            Some(node) if !node.is_empty() => Eid::dtn(node),
            _ => Eid::ipn_from_str(&verified.source).unwrap_or(Eid::dtn(&meta.node)),
        };

        Ok(DiscoveredNode {
            eid,
            capabilities: NodeCapabilities {
                custody: meta.custody,
                fragmentation: meta.fragmentation,
                max_bandwidth_bps: meta.max_bandwidth_bps,
                max_bundle_size: meta.max_bundle_size,
                bp_version: meta.bp_version,
            },
            last_seen_ms: meta.timestamp_ms,
            tx_frequency_hz: 0,
        })
    }

    fn is_known_beacon_frequency(&self, freq_hz: u64) -> bool {
        // Frequências conhecidas de beacon (ex: bandas ISM, bandas de deep space)
        const BEACON_FREQS: &[u64] = &[
            2_400_000_000, // 2.4 GHz
            5_800_000_000, // 5.8 GHz
            8_400_000_000, // 8.4 GHz (X-band, NASA DSN)
            32_000_000_000, // 32 GHz (Ka-band)
        ];
        BEACON_FREQS.contains(&freq_hz)
    }
}

/// Nó descoberto na rede.
#[derive(Debug, Clone)]
pub struct DiscoveredNode {
    pub eid: Eid,
    pub capabilities: NodeCapabilities,
    pub last_seen_ms: u64,
    pub tx_frequency_hz: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eid_dtn() {
        let eid = Eid::dtn("node-alpha");
        assert_eq!(eid.as_str(), "dtn://node-alpha/");
        assert_eq!(eid.scheme, EidScheme::Dtn);
    }

    #[test]
    fn test_eid_ipn() {
        let eid = Eid::ipn(42, 7);
        assert_eq!(eid.as_str(), "ipn:42.7");
        assert_eq!(eid.scheme, EidScheme::Ipn);
    }

    #[test]
    fn test_beacon_creation() {
        let svc = DiscoveryService::new(Eid::dtn("local"));
        let beacon = svc.create_beacon(1_000_000);
        assert_eq!(beacon.source.as_str(), "dtn://local/");
        assert!(beacon.capabilities.custody);
        assert_eq!(beacon.tx_frequency_hz, 2_400_000_000);
    }

    #[test]
    fn test_spectral_scan_known_freq() {
        let svc = DiscoveryService::new(Eid::dtn("local"));
        let result = svc.scan_frequency(2_400_000_000);
        assert!(result.confidence > 0.9);
        assert!(result.snr_db >= 6.0);
    }

    #[test]
    fn test_spectral_scan_noise() {
        let svc = DiscoveryService::new(Eid::dtn("local"));
        let result = svc.scan_frequency(1_000_000_000); // Frequência desconhecida
        assert!(result.confidence < 0.5);
    }

    #[test]
    fn test_detect_beacons() {
        let svc = DiscoveryService::new(Eid::dtn("local"));
        let results = vec![
            svc.scan_frequency(2_400_000_000),
            svc.scan_frequency(1_000_000_000),
        ];
        let detected = svc.detect_beacons(&results);
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0], 2_400_000_000);
    }

    #[test]
    fn test_process_beacon() {
        let svc = DiscoveryService::new(Eid::dtn("local"));
        let beacon = Beacon {
            source: Eid::dtn("remote-node"),
            timestamp_ms: 5_000,
            capabilities: NodeCapabilities {
                custody: false,
                fragmentation: true,
                max_bandwidth_bps: 500_000,
                max_bundle_size: 500_000,
                bp_version: 7,
            },
            tx_frequency_hz: 8_400_000_000,
            tx_power_dbm: 30,
        };
        let node = svc.process_beacon(&beacon);
        assert_eq!(node.eid.as_str(), "dtn://remote-node/");
        assert!(!node.capabilities.custody);
        assert_eq!(node.last_seen_ms, 5_000);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_ipn_from_str() {
        let eid = Eid::ipn_from_str("ipn:42.7").unwrap();
        assert_eq!(eid.as_str(), "ipn:42.7");
        assert!(Eid::ipn_from_str("dtn://x/").is_none());
        assert!(Eid::ipn_from_str("ipn:notanumber.7").is_none());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_bpv7_beacon_roundtrip() {
        use crate::bpv7::hmac_sha2_key;
        use hardy_bpv7::bpsec::key::KeySet;

        let svc = DiscoveryService::new(Eid::dtn("node-7"));
        let key = hmac_sha2_key("dtn://broadcast.dsn/", b"arkhe-test-secret-32bytes!!");
        let keys = KeySet::new(vec![key.clone()]);

        let wire = svc.create_bpv7_beacon(42_000, &key).unwrap();
        assert!(!wire.is_empty());

        let node = svc.process_bpv7_beacon(&wire, &keys).unwrap();
        assert_eq!(node.eid.as_str(), "dtn://node-7/");
        assert!(node.capabilities.custody);
        assert_eq!(node.capabilities.bp_version, 7);
        assert_eq!(node.last_seen_ms, 42_000);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_bpv7_beacon_tamper_detected() {
        use crate::bpv7::hmac_sha2_key;
        use hardy_bpv7::bpsec::key::KeySet;

        let svc = DiscoveryService::new(Eid::dtn("node-7"));
        let key = hmac_sha2_key("dtn://broadcast.dsn/", b"arkhe-test-secret-32bytes!!");
        let keys = KeySet::new(vec![key.clone()]);

        let mut wire = svc.create_bpv7_beacon(42_000, &key).unwrap();
        let mid = wire.len() / 2;
        wire[mid] ^= 0x55;
        assert!(svc.process_bpv7_beacon(&wire, &keys).is_err());
    }
}
