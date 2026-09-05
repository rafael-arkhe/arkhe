//! Framing G1 — SOF + LEN + PAYLOAD + CRC16 + EOF, com rejeição de pacotes
//! corrompidos.

/// CRC16‑XMODEM.
pub fn crc16_xmodem(data: &[u8]) -> u16 {
    let mut crc: u16 = 0x0000;
    for &b in data {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

pub const SOF: u8 = 0xAA;
pub const EOF: u8 = 0xBB;

/// Framing G1: `[SOF][LEN:u16 BE][PAYLOAD][CRC16 BE][EOF]`.
pub fn frame(payload: &[u8]) -> Vec<u8> {
    if payload.len() > u16::MAX as usize {
        panic!("payload too large for G1 framing");
    }
    let mut out = Vec::with_capacity(payload.len() + 6);
    out.push(SOF);
    out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(&crc16_xmodem(payload).to_be_bytes());
    out.push(EOF);
    out
}

/// Unframing G1 com verificação de CRC e delimitação por SOF/EOF.
pub fn unframe(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 6 {
        return None;
    }

    let mut i = 0;
    while i < data.len() - 1 && data[i] != SOF {
        i += 1;
    }
    if i >= data.len() - 1 {
        return None;
    }

    // i aponta para SOF
    let len_bytes = i + 2 + 1;
    if len_bytes > data.len() {
        return None;
    }

    let length = u16::from_be_bytes([data[i + 1], data[i + 2]]) as usize;
    let payload_start = i + 3;
    let payload_end = payload_start + length;
    let crc_start = payload_end;
    let eof_idx = crc_start + 2;

    if eof_idx + 1 > data.len() {
        return None;
    }

    let payload = &data[payload_start..payload_end];
    let crc_recv = u16::from_be_bytes([data[crc_start], data[crc_start + 1]]);
    let crc_calc = crc16_xmodem(payload);

    if crc_recv != crc_calc {
        return None;
    }

    if data[eof_idx] != EOF {
        return None;
    }

    Some(payload.to_vec())
}

/// Framer Z1T de uso geral.
#[derive(Debug, Clone, Default)]
pub struct Z1TFramer;

impl Z1TFramer {
    pub fn new() -> Self {
        Self
    }

    pub fn frame(&self, payload: &[u8]) -> Vec<u8> {
        frame(payload)
    }

    pub fn unframe(&self, data: &[u8]) -> Option<Vec<u8>> {
        unframe(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_unframe_roundtrip() {
        let payload = b"Z1T_HANDSHAKE";
        let framed = frame(payload);
        assert_eq!(unframe(&framed).unwrap(), payload);
    }

    #[test]
    fn test_corrupted_payload_rejected() {
        let payload = b"Z1T_EVIDENCE";
        let mut framed = frame(payload);
        let mid = framed.len() / 2;
        framed[mid] ^= 0xFF;
        assert!(unframe(&framed).is_none());
    }

    #[test]
    fn test_corrupted_crc_rejected() {
        let payload = b"Z1T_INFER";
        let mut framed = frame(payload);
        let n = framed.len();
        framed[n - 3] ^= 0x01;
        assert!(unframe(&framed).is_none());
    }

    #[test]
    fn test_prefix_garbage_skipped() {
        let payload = b"payload";
        let framed = frame(payload);
        let mut junk = vec![0x00, 0x11, 0x22];
        junk.extend_from_slice(&framed);
        assert_eq!(unframe(&junk).unwrap(), payload);
    }
}