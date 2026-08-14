//! Fase 1 — Integração com `hardy-bpv7` (RFC 9171) + BPSec (RFC 9173).
//!
//! Substitui os beacons sintéticos por bundles BPv7 reais:
//! - Construção via [`Builder`] (`Builder::new` / `with_payload` /
//!   `CreationTimestamp::now`).
//! - Integridade via BIB-HMAC-SHA2 (`signer::Signer` / `Context::HMAC_SHA2`).
//! - Verificação na recepção via `ParsedBundle::parse_with_keys`, que falha
//!   na presença da chave se a MAC não conferir (RFC 9173 §3.8).
//!
//! Este módulo só existe com a feature `std` (`hardy-bpv7/std`); em `no_std`
//! o crate mantém o comportamento de beacon simulado.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use hardy_bpv7::{
    block,
    bpsec::{key, rfc9173::ScopeFlags, signer},
    builder::Builder,
    bundle::{self, ParsedBundle},
    creation_timestamp::CreationTimestamp,
    eid::Eid,
};

/// Erro de integração BPv7/BPSec.
#[derive(Debug)]
pub enum Error {
    /// EID inválido (falha no `FromStr`).
    Eid(hardy_bpv7::eid::Error),
    /// Falha na construção do bundle.
    Builder(hardy_bpv7::builder::Error),
    /// Falha na serialização/parse do bundle.
    Bundle(hardy_bpv7::Error),
    /// Falha na assinatura BPSec.
    Signer(hardy_bpv7::bpsec::signer::Error),
    /// Falha BPSec de baixo nível.
    Bpsec(hardy_bpv7::bpsec::Error),
    /// Bundle sem bloco de payload.
    NoPayloadBlock,
    /// Erro de aplicação.
    Message(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Eid(e) => write!(f, "EID inválido: {e}"),
            Error::Builder(e) => write!(f, "builder: {e}"),
            Error::Bundle(e) => write!(f, "bundle: {e}"),
            Error::Signer(e) => write!(f, "signer: {e}"),
            Error::Bpsec(e) => write!(f, "bpsec: {e}"),
            Error::NoPayloadBlock => write!(f, "bundle sem bloco de payload"),
            Error::Message(m) => write!(f, "{m}"),
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Error::Eid(e) => Some(e),
            Error::Builder(e) => Some(e),
            Error::Bundle(e) => Some(e),
            Error::Signer(e) => Some(e),
            Error::Bpsec(e) => Some(e),
            _ => None,
        }
    }
}

impl From<hardy_bpv7::eid::Error> for Error {
    fn from(e: hardy_bpv7::eid::Error) -> Self {
        Error::Eid(e)
    }
}

impl From<hardy_bpv7::builder::Error> for Error {
    fn from(e: hardy_bpv7::builder::Error) -> Self {
        Error::Builder(e)
    }
}

impl From<hardy_bpv7::Error> for Error {
    fn from(e: hardy_bpv7::Error) -> Self {
        Error::Bundle(e)
    }
}

impl From<hardy_bpv7::bpsec::signer::Error> for Error {
    fn from(e: hardy_bpv7::bpsec::signer::Error) -> Self {
        Error::Signer(e)
    }
}

impl From<hardy_bpv7::bpsec::Error> for Error {
    fn from(e: hardy_bpv7::bpsec::Error) -> Self {
        Error::Bpsec(e)
    }
}

/// Bundle beacon BPv7 assinado — bundle estruturado + bytes CBOR.
pub struct SignedBeacon {
    /// Bundle BPv7 resultante (contém EIDs, blocos e cobertura BIB).
    pub bundle: bundle::Bundle,
    /// Bytes CBOR serializados (prontos para transmissão).
    pub wire: Vec<u8>,
}

/// Beacon recebido, com payload extraído e estado de integridade BIB.
pub struct VerifiedBeacon {
    /// EID de origem (do bloco primário).
    pub source: String,
    /// EID de destino (do bloco primário).
    pub destination: String,
    /// Conteúdo do bloco de payload (decifrado, se BCB presente).
    pub payload: Vec<u8>,
    /// `true` se o bloco de payload carrega um BIB verificado.
    pub bib_present: bool,
}

/// Cria uma chave HMAC-SHA2 (JWK `oct`, HS256) para BPSec BIB.
pub fn hmac_sha2_key(kid: &str, secret: &[u8]) -> key::Key {
    key::Key {
        key_type: key::Type::OctetSequence {
            key: secret.to_vec().into_boxed_slice(),
        },
        key_algorithm: Some(key::KeyAlgorithm::HS256),
        operations: Some(
            [key::Operation::Sign, key::Operation::Verify]
                .into_iter()
                .collect(),
        ),
        id: Some(kid.to_string()),
        key_use: Some(key::Use::Signature),
        ..Default::default()
    }
}

/// Constrói um bundle BPv7 com payload e assina o bloco de payload com
/// BIB-HMAC-SHA2, retornando os bytes CBOR para transmissão.
pub fn sign_beacon(
    source: &str,
    destination: &str,
    payload: &[u8],
    key: &key::Key,
) -> Result<SignedBeacon, Error> {
    let source_eid: Eid = source.parse()?;
    let dest_eid: Eid = destination.parse()?;

    let (bundle, bytes) = Builder::new(source_eid.clone(), dest_eid)
        .with_payload(payload.into())
        .build(CreationTimestamp::now())?;

    let payload_block = find_payload_block(&bundle)?;

    let wire = signer::Signer::new(&bundle, &bytes)
        .sign_block(
            payload_block,
            signer::Context::HMAC_SHA2(ScopeFlags::default()),
            source_eid,
            key,
        )
        .map_err(|(_, e)| e)?
        .rebuild()?;

    Ok(SignedBeacon {
        bundle,
        wire: wire.to_vec(),
    })
}

/// Parseia e verifica um bundle recebido.
///
/// `ParsedBundle::parse_with_keys` valida o BIB durante o parse: com a chave
/// correta no `KeySet`, um payload adulterado falha; com chave ausente a
/// verificação é pulada (NoKey) e `bib_present` indica a ausência de BIB.
pub fn verify_beacon(wire: &[u8], keys: &key::KeySet) -> Result<VerifiedBeacon, Error> {
    let parsed = ParsedBundle::parse_with_keys(wire, keys)?;

    let payload_block = find_payload_block(&parsed.bundle)?;

    let bib_present = matches!(
        parsed.bundle.blocks.get(&payload_block).map(|b| &b.bib),
        Some(block::BibCoverage::Some(_))
    );

    let payload = match parsed.bundle.block_data(payload_block, wire, keys)? {
        block::Payload::Borrowed(d) => d.to_vec(),
        block::Payload::Decrypted(z) => z.to_vec(),
    };

    Ok(VerifiedBeacon {
        source: parsed.bundle.id.source.to_string(),
        destination: parsed.bundle.destination.to_string(),
        payload,
        bib_present,
    })
}

/// Localiza o número do bloco de payload (tipo `Payload`) no bundle.
fn find_payload_block(bundle: &bundle::Bundle) -> Result<u64, Error> {
    bundle
        .blocks
        .iter()
        .find(|(_, b)| b.block_type == block::Type::Payload)
        .map(|(n, _)| *n)
        .ok_or(Error::NoPayloadBlock)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> (key::Key, key::KeySet) {
        let k = hmac_sha2_key("ipn:2.1", b"arkhe-secret-key-for-tests-32b!!");
        let set = key::KeySet::new(vec![k.clone()]);
        (k, set)
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let (key, keys) = test_key();
        let payload = b"ARKHE-BEACON-v1\x00capabilities:{\"fec\":true}";

        let signed = sign_beacon("ipn:1.2", "ipn:2.1", payload, &key).unwrap();
        assert!(!signed.wire.is_empty());
        assert!(signed.bundle.destination.to_string().starts_with("ipn:2.1"));

        let verified = verify_beacon(&signed.wire, &keys).unwrap();
        assert_eq!(verified.source, "ipn:1.2");
        assert_eq!(verified.destination, "ipn:2.1");
        assert_eq!(verified.payload, payload);
        assert!(verified.bib_present);
    }

    #[test]
    fn tampered_payload_fails_with_key() {
        let (key, keys) = test_key();
        let signed = sign_beacon("ipn:1.2", "ipn:2.1", b"ARKHE-BEACON", &key).unwrap();

        let mut tampered = signed.wire;
        // Corrompe o meio do payload (região não-CBOR da wire) e re-testa.
        let n = tampered.len();
        if n > 8 {
            tampered[n / 2] ^= 0xff;
        }
        assert!(verify_beacon(&tampered, &keys).is_err());
    }

    #[test]
    fn no_key_skips_verification_but_reports_no_bib() {
        // Sem chave (KeySet vazio): parse OK (NoKey), payload lido, bib_present true
        // porque o BIB ainda está declarado — a integridade não foi checada.
        let (key, _) = test_key();
        let signed = sign_beacon("ipn:1.2", "ipn:2.1", b"ARKHE-BEACON", &key).unwrap();
        let empty = key::KeySet::EMPTY;
        let verified = verify_beacon(&signed.wire, &empty).unwrap();
        assert!(verified.bib_present);
        assert_eq!(verified.payload, b"ARKHE-BEACON");
    }

    #[test]
    fn wrong_key_fails_verification() {
        let (key, _) = test_key();
        let wrong = hmac_sha2_key("ipn:9.9", b"wrong-key-wrong-key-wrong-key");
        let keys = key::KeySet::new(vec![wrong]);
        let signed = sign_beacon("ipn:1.2", "ipn:2.1", b"ARKHE-BEACON", &key).unwrap();
        assert!(verify_beacon(&signed.wire, &keys).is_err());
    }
}
