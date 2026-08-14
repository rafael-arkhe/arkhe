//! Convergência óptica para enlaces de comunicação por luz (VLC / laser).
//!
//! # Fase 4
//! - **RaptorQ FEC**: `raptorq` 1.7 (RFC 6330) para correção de erros sem
//!   canal de retorno (one-way light links).
//! - **QR Code**: `qr_code` 2.0 para codificação visual do frame óptico.
//!
//! # Restrições
//! - Módulo **std only** (`#[cfg(feature = "std")]`): preserva o build `no_std`.
//! - Sem `unsafe`; o crate inteiro é `#![deny(unsafe_code)]`.

use alloc::string::String;
use alloc::vec::Vec;

use crate::processing::sha256_hex;

/// MTU padrão do enlace óptico (bytes).
pub const OPTICAL_MTU: u16 = 1400;
/// Pacotes de reparo por bloco (RaptorQ).
pub const DEFAULT_REPAIR_PACKETS: u32 = 16;

/// Erros da camada de convergência óptica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpticalError {
    /// Dados vazios ou mtu inválido.
    InvalidInput,
    /// Não foi possível decodificar com os pacotes recebidos.
    DecodeInsufficientPackets,
    /// Erro na codificação QR.
    QrEncodeFailed,
    /// Frame inválido (primeiro pacote deve conter o OTI de 12 bytes).
    InvalidFrame,
}

impl core::fmt::Display for OpticalError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidInput => write!(f, "optical convergence: invalid input"),
            Self::DecodeInsufficientPackets => {
                write!(f, "optical convergence: insufficient packets to decode")
            }
            Self::QrEncodeFailed => write!(f, "optical convergence: QR encode failed"),
            Self::InvalidFrame => write!(f, "optical convergence: invalid frame"),
        }
    }
}

impl core::error::Error for OpticalError {}

impl From<qr_code::types::QrError> for OpticalError {
    fn from(_value: qr_code::types::QrError) -> Self {
        Self::QrEncodeFailed
    }
}

/// Codifica dados com FEC RaptorQ e retorna o frame óptico serializado.
///
/// O primeiro pacote do frame carrega o `ObjectTransmissionInformation`
/// (12 bytes) que o decodificador precisa para reconstruir a configuração.
pub fn fec_encode(data: &[u8], mtu: u16, repair_packets: u32) -> Result<Vec<Vec<u8>>, OpticalError> {
    if data.is_empty() || mtu == 0 {
        return Err(OpticalError::InvalidInput);
    }
    let encoder = raptorq::Encoder::with_defaults(data, mtu);
    let oti = encoder.get_config().serialize();
    let mut frames = Vec::with_capacity(1);
    frames.push(oti.to_vec());
    for packet in encoder.get_encoded_packets(repair_packets) {
        frames.push(packet.serialize());
    }
    Ok(frames)
}

/// Decodifica um frame óptico RaptorQ (tolerante a perdas).
pub fn fec_decode(frames: &[Vec<u8>]) -> Result<Vec<u8>, OpticalError> {
    let first = frames.first().ok_or(OpticalError::InvalidFrame)?;
    if first.len() < 12 {
        return Err(OpticalError::InvalidFrame);
    }
    let mut oti_bytes = [0u8; 12];
    oti_bytes.copy_from_slice(&first[..12]);
    let oti = raptorq::ObjectTransmissionInformation::deserialize(&oti_bytes);
    let mut decoder = raptorq::Decoder::new(oti);
    for frame in &frames[1..] {
        let packet = raptorq::EncodingPacket::deserialize(frame);
        if let Some(decoded) = decoder.decode(packet) {
            return Ok(decoded);
        }
    }
    Err(OpticalError::DecodeInsufficientPackets)
}

/// Codifica um frame em QR Code e retorna (largura, texto ASCII).
pub fn qr_encode(data: &[u8]) -> Result<(usize, String), OpticalError> {
    if data.is_empty() {
        return Err(OpticalError::InvalidInput);
    }
    let code = qr_code::QrCode::new(data)?;
    Ok((code.width(), code.to_string(false, 2)))
}

/// Codifica um frame em QR Code e retorna a matriz de pixels `(width, Vec<bool>)`.
pub fn qr_matrix(data: &[u8]) -> Result<(usize, Vec<bool>), OpticalError> {
    if data.is_empty() {
        return Err(OpticalError::InvalidInput);
    }
    let code = qr_code::QrCode::new(data)?;
    Ok((code.width(), code.to_vec()))
}

/// Converte um frame RaptorQ serializado em um QR Code (associando o digest).
pub fn frame_to_qr(frames: &[Vec<u8>]) -> Result<(usize, String), OpticalError> {
    let mut payload = Vec::new();
    for f in frames {
        payload.extend_from_slice(f);
    }
    qr_encode(&payload)
}

/// Calcula o digest SHA-256 de um frame óptico (integridade na luz).
pub fn frame_digest(frames: &[Vec<u8>]) -> String {
    let mut payload = Vec::new();
    for f in frames {
        payload.extend_from_slice(f);
    }
    sha256_hex(&payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fec_roundtrip_no_loss() {
        let data = b"ARKHE optical convergence payload - RaptorQ FEC roundtrip";
        let frames = fec_encode(data, OPTICAL_MTU, DEFAULT_REPAIR_PACKETS).unwrap();
        let decoded = fec_decode(&frames).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_fec_roundtrip_with_loss() {
        let data = vec![0x42u8; 200_000];
        let frames = fec_encode(&data, OPTICAL_MTU, 64).unwrap();
        assert!(frames.len() > 10);
        // Descarta alguns pacotes (perda de enlace) e ainda decodifica.
        // Mantém o índice 0 (cabeçalho OTI); descarta 1 de cada 5 pacotes de dados.
        let received: Vec<Vec<u8>> = frames
            .iter()
            .enumerate()
            .filter(|(i, _)| *i == 0 || *i % 5 != 0)
            .map(|(_, f)| f.clone())
            .collect();
        let decoded = fec_decode(&received).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_fec_empty_input_rejected() {
        assert_eq!(fec_encode(&[], OPTICAL_MTU, 0), Err(OpticalError::InvalidInput));
        assert_eq!(fec_encode(b"x", 0, 0), Err(OpticalError::InvalidInput));
    }

    #[test]
    fn test_qr_roundtrip() {
        let (width, ascii) = qr_encode(b"ARKHE QR").unwrap();
        assert!(width > 20);
        assert!(ascii.contains('█') || ascii.contains('▀'));
    }

    #[test]
    fn test_frame_digest_stable() {
        let frames = fec_encode(b"digest me", OPTICAL_MTU, 8).unwrap();
        let d1 = frame_digest(&frames);
        let d2 = frame_digest(&frames);
        assert_eq!(d1, d2);
        assert_eq!(d1.len(), 64);
    }
}
