//! Minimal canonical CBOR codec for the PoTT receipt wire format.
//!
//! The PoTT receipt is a canonical CBOR map (RFC 8949, big-endian integers)
//! with the integer keys specified in the paper's Appendix A:
//!
//! ```text
//! 0: h     bstr (32) — payload digest H(P) (Bitcoin-native where applicable)
//! 1: ν     bstr (16) — per-message nonce (unique per payload instance)
//! 2: node  bstr (32) — relay NodeID (BIP-340 x-only public key)
//! 3: tin   uint (64) — ingress time, TAI seconds (CCSDS CUC, epoch 1958-01-01)
//! 4: tout  uint (64) — egress time, TAI seconds
//! 5: prev  bstr (32) — previ = H(R(i−1) \ s(i−1)) anti-splice binding
//! 6: sig   bstr (64) — BIP-340 Schnorr over the canonical CBOR of keys 0–5
//! ```
//!
//! Canonical encoding decisions:
//!
//! * Timestamps are **always** 8-byte unsigned integers (`0x1B`) because the
//!   paper mandates *64-bit* TAI seconds; a smaller encoding is rejected as
//!   non-canonical. This matches the normative test vector exactly.
//! * Byte-string headers, map headers and keys use the shortest definite-length
//!   form; indefinite-length strings/maps are rejected.
//! * Maps MUST have exactly 7 entries in strictly ascending key order; unknown,
//!   duplicate or out-of-order keys are rejected ("Implementations MUST reject
//!   non-canonical encodings and unknown keys").
//! * Trailing bytes are rejected.

use alloc::vec::Vec;

/// Digest of the payload (32 bytes).
pub type H32 = [u8; 32];
/// Per-message nonce (16 bytes).
pub type Nonce16 = [u8; 16];
/// Relay NodeID — BIP-340 x-only public key (32 bytes).
pub type NodeId = [u8; 32];
/// BIP-340 Schnorr signature (64 bytes).
pub type Sig64 = [u8; 64];
/// TAI second (64-bit, CUC coarse).
pub type TAI = u64;

/// Normative CBOR test vector from the paper's Appendix A: one canonical
/// 7-entry receipt (map header `A7` included; the two timestamps are written
/// as contiguous 8-byte hex, so tokens are NOT all single bytes). The signature
/// field is a parser-checkable dummy.
pub const APPENDIX_A_HEX: &str = concat!(
    "A7 00 58 20 83 a0 12 ac 61 2c 83 f6 89 17 73 87 35 34 65 fb 96 13 56 e8 1b cd 8a da",
    " 4b a0 d6 57 da 1c 26 85",
    " 01 50 22 19 c6 46 c0 c3 53 d1 87 ef b2 ca b9 ef 61 5b",
    " 02 58 20 d4 06 3a ea 17 03 81 ce ca f4 d4 3b 1e 8d d3 2e c1 34 9f ac 78 ed c0 75",
    " ce 08 fb 36 4d 60 40 43",
    " 03 1B 0000000065B9B8A0",
    " 04 1B 0000000065B9BD40",
    " 05 58 20 2c 77 0e 00 80 83 e6 2a fd 13 76 98 ce 19 6d b6 5c b4 06 eb 2b 4c 50 6c",
    " b6 fa 0c 54 6f 95 d8 55",
    " 06 58 40 db d5 95 30 45 c5 b1 31 a2 5e ca bd 6f 2d 78 6b 28 7e e1 da 3a e2 84 5b",
    " 27 89 b5 1c cd c3 82 ef 83 68 e0 36 50 87 9c 71 75 5b 7f da 46 6b 44 a7 32 18 f6",
    " 82 06 25 e9 59 2f cc b3 a6 13 3b 92 b2",
);

/// Decoded wire fields for one PoTT receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireReceipt {
    /// Payload digest `H(P)`.
    pub h: H32,
    /// Per-message nonce.
    pub nu: Nonce16,
    /// Relay identifier (x-only public key).
    pub node: NodeId,
    /// Ingress time (TAI seconds).
    pub tin: TAI,
    /// Egress time (TAI seconds).
    pub tout: TAI,
    /// Anti-splice binding to the previous hop's receipt (sans signature).
    pub prev: H32,
    /// BIP-340 Schnorr signature over keys 0–5.
    pub sig: Sig64,
}

/// Wire codec errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    /// Input shorter than the enclosing item requires.
    Truncated,
    /// Bytes remain after the receipt map.
    TrailingData,
    /// Not a definite-length CBOR map.
    NotMap,
    /// Map has a different number of entries than 7.
    UnexpectedMapSize,
    /// Map key is not an unsigned integer.
    KeyNotUint,
    /// Key outside `0..=6`.
    UnknownKey,
    /// Same key encoded twice.
    DuplicateKey,
    /// Keys not in strictly ascending order.
    UnorderedKey,
    /// Value major type does not match the field schema.
    WrongType,
    /// Byte string does not have the exact expected length.
    BadLength,
    /// Not a minimal/canonical encoding (e.g. 2-byte timestamp).
    NonCanonical,
    /// Indefinite-length item encountered where definite is required.
    IndefiniteLength,
}

fn push_head(out: &mut Vec<u8>, major: u8, len: u64) {
    out.push((major << 5) | if len < 24 { len as u8 } else { 24 });
    match len {
        0..=23 => {}
        24..=0xff => out.push(len as u8),
        0x100..=0xffff => out.extend_from_slice(&(len as u16).to_be_bytes()),
        0x1_0000..=0xffff_ffff => out.extend_from_slice(&(len as u32).to_be_bytes()),
        _ => out.extend_from_slice(&len.to_be_bytes()),
    }
}

fn push_uint8_key(out: &mut Vec<u8>, key: u8) {
    // Keys 0..=6 are single-byte unsigned integers (major type 0, ai = key).
    out.push(key);
}

fn push_u64(out: &mut Vec<u8>, v: u64) {
    // Timestamps: always 8-byte 64-bit TAI seconds (0x1B).
    out.push(0x1b);
    out.extend_from_slice(&v.to_be_bytes());
}

fn push_bstr(out: &mut Vec<u8>, bytes: &[u8]) {
    push_head(out, 2, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

fn push_map(out: &mut Vec<u8>, entries: u64) {
    push_head(out, 5, entries);
}

/// Encodes the full receipt (keys 0–6) as canonical CBOR.
#[must_use]
pub fn encode_receipt(w: &WireReceipt) -> Vec<u8> {
    encode_parts(&w.h, &w.nu, &w.node, w.tin, w.tout, &w.prev, &w.sig)
}

/// Encodes the signing payload (keys 0–5, no `sig` field).
///
/// This is the byte range `Mi = encode(h ‖ ν ‖ NodeID ‖ tin ‖ tout ‖ prev)`
/// covered by each relay's Schnorr signature and by the anti-splice hash:
/// `previ = H(R(i−1) \ s(i−1)) = SHA-256(encode_signing_payload(R(i−1)))`.
#[must_use]
pub fn encode_signing_payload(
    h: &H32,
    nu: &Nonce16,
    node: &NodeId,
    tin: TAI,
    tout: TAI,
    prev: &H32,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(6 + h.len() + nu.len() + node.len() + 8 + 8 + prev.len());
    push_map(&mut out, 6);
    push_uint8_key(&mut out, 0);
    push_bstr(&mut out, h);
    push_uint8_key(&mut out, 1);
    push_bstr(&mut out, nu);
    push_uint8_key(&mut out, 2);
    push_bstr(&mut out, node);
    push_uint8_key(&mut out, 3);
    push_u64(&mut out, tin);
    push_uint8_key(&mut out, 4);
    push_u64(&mut out, tout);
    push_uint8_key(&mut out, 5);
    push_bstr(&mut out, prev);
    out
}

fn encode_parts(
    h: &H32,
    nu: &Nonce16,
    node: &NodeId,
    tin: TAI,
    tout: TAI,
    prev: &H32,
    sig: &Sig64,
) -> Vec<u8> {
    // The signing payload declares a 6-entry map (keys 0–5); the full receipt
    // promotes that header to 7 entries (keys 0–6) and appends the sig value.
    let mut out = encode_signing_payload(h, nu, node, tin, tout, prev);
    out[0] = 0xa7;
    out.push(6);
    push_bstr(&mut out, sig);
    out
}

/// Decodes exactly one canonical receipt from `bytes`.
/// Rejects non-canonical encodings and trailing data.
pub fn decode_receipt(bytes: &[u8]) -> Result<WireReceipt, WireError> {
    let mut d = Decoder { buf: bytes, pos: 0 };
    let msg = d.read_map_len()?;
    if msg != 7 {
        return Err(WireError::UnexpectedMapSize);
    }
    let mut w = WireReceipt {
        h: [0u8; 32],
        nu: [0u8; 16],
        node: [0u8; 32],
        tin: 0,
        tout: 0,
        prev: [0u8; 32],
        sig: [0u8; 64],
    };
    let mut last_key: Option<u8> = None;
    for _ in 0..7 {
        let key = d.read_uint_key()?;
        if Some(key) <= last_key {
            return Err(if Some(key) == last_key {
                WireError::DuplicateKey
            } else {
                WireError::UnorderedKey
            });
        }
        last_key = Some(key);
        match key {
            0 => d.read_bstr_exact(32, &mut w.h)?,
            1 => d.read_bstr_exact(16, &mut w.nu)?,
            2 => d.read_bstr_exact(32, &mut w.node)?,
            3 => w.tin = d.read_tai_seconds()?,
            4 => w.tout = d.read_tai_seconds()?,
            5 => d.read_bstr_exact(32, &mut w.prev)?,
            6 => d.read_bstr_exact(64, &mut w.sig)?,
            _ => return Err(WireError::UnknownKey),
        }
    }
    if d.pos != bytes.len() {
        return Err(WireError::TrailingData);
    }
    Ok(w)
}

struct Decoder<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Decoder<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        let end = self.pos.checked_add(n).ok_or(WireError::Truncated)?;
        if end > self.buf.len() {
            return Err(WireError::Truncated);
        }
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn read_head(&mut self) -> Result<(u8, u8), WireError> {
        let b = *self.take(1)?.first().ok_or(WireError::Truncated)?;
        let major = b >> 5;
        let ai = b & 0x1f;
        if ai == 31 {
            return Err(WireError::IndefiniteLength);
        }
        Ok((major, ai))
    }

    fn read_head_len(&mut self) -> Result<(u8, u64), WireError> {
        let (major, ai) = self.read_head()?;
        let len = match ai {
            0..=23 => u64::from(ai),
            24 => {
                let v = u64::from(*self.take(1)?.first().ok_or(WireError::Truncated)?);
                if v < 24 {
                    // Not the shortest form.
                    return Err(WireError::NonCanonical);
                }
                v
            }
            25 => {
                let b = self.take(2)?;
                let v = u64::from(u16::from_be_bytes([b[0], b[1]]));
                if v < 0x100 {
                    return Err(WireError::NonCanonical);
                }
                v
            }
            26 => {
                let b = self.take(4)?;
                let v = u64::from(u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
                if v < 0x1_0000 {
                    return Err(WireError::NonCanonical);
                }
                v
            }
            27 => {
                let b = self.take(8)?;
                u64::from_be_bytes([
                    b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
                ])
            }
            _ => return Err(WireError::IndefiniteLength),
        };
        Ok((major, len))
    }

    fn read_map_len(&mut self) -> Result<u64, WireError> {
        let (major, len) = self.read_head_len()?;
        if major != 5 {
            return Err(WireError::NotMap);
        }
        Ok(len)
    }

    /// Reads the map key: an unsigned integer header that must be the shortest
    /// single-byte form for `0..=23` (canonical ordering keys).
    fn read_uint_key(&mut self) -> Result<u8, WireError> {
        let (major, len) = self.read_head_len()?;
        if major != 0 {
            return Err(WireError::KeyNotUint);
        }
        u8::try_from(len).map_err(|_| WireError::UnknownKey)
    }

    /// Reads a 64-bit TAI timestamp value. MUST be 8-byte (`0x1B`).
    fn read_tai_seconds(&mut self) -> Result<TAI, WireError> {
        let (major, ai) = self.read_head()?;
        if major != 0 {
            return Err(WireError::WrongType);
        }
        if ai != 27 {
            return Err(WireError::NonCanonical);
        }
        let b = self.take(8)?;
        Ok(u64::from_be_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    fn read_bstr_exact(&mut self, len: usize, out: &mut [u8]) -> Result<(), WireError> {
        let (major, l) = self.read_head_len()?;
        if major != 2 {
            return Err(WireError::WrongType);
        }
        if l != len as u64 {
            return Err(WireError::BadLength);
        }
        let b = self.take(len)?;
        out.copy_from_slice(b);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn bytes_from_hex() -> Vec<u8> {
        let compact: alloc::string::String = APPENDIX_A_HEX.split_whitespace().collect();
        compact
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let s = core::str::from_utf8(pair).expect("ascii hex");
                u8::from_str_radix(s, 16).expect("hex byte")
            })
            .collect()
    }

    pub fn test_vector_fields() -> WireReceipt {
        WireReceipt {
            h: [
                0x83, 0xa0, 0x12, 0xac, 0x61, 0x2c, 0x83, 0xf6, 0x89, 0x17, 0x73, 0x87, 0x35,
                0x34, 0x65, 0xfb, 0x96, 0x13, 0x56, 0xe8, 0x1b, 0xcd, 0x8a, 0xda, 0x4b, 0xa0,
                0xd6, 0x57, 0xda, 0x1c, 0x26, 0x85,
            ],
            nu: [
                0x22, 0x19, 0xc6, 0x46, 0xc0, 0xc3, 0x53, 0xd1, 0x87, 0xef, 0xb2, 0xca, 0xb9,
                0xef, 0x61, 0x5b,
            ],
            node: [
                0xd4, 0x06, 0x3a, 0xea, 0x17, 0x03, 0x81, 0xce, 0xca, 0xf4, 0xd4, 0x3b, 0x1e,
                0x8d, 0xd3, 0x2e, 0xc1, 0x34, 0x9f, 0xac, 0x78, 0xed, 0xc0, 0x75, 0xce, 0x08,
                0xfb, 0x36, 0x4d, 0x60, 0x40, 0x43,
            ],
            tin: 0x65B9B8A0,
            tout: 0x65B9BD40,
            prev: [
                0x2c, 0x77, 0x0e, 0x00, 0x80, 0x83, 0xe6, 0x2a, 0xfd, 0x13, 0x76, 0x98, 0xce,
                0x19, 0x6d, 0xb6, 0x5c, 0xb4, 0x06, 0xeb, 0x2b, 0x4c, 0x50, 0x6c, 0xb6, 0xfa,
                0x0c, 0x54, 0x6f, 0x95, 0xd8, 0x55,
            ],
            sig: [
                0xdb, 0xd5, 0x95, 0x30, 0x45, 0xc5, 0xb1, 0x31, 0xa2, 0x5e, 0xca, 0xbd, 0x6f,
                0x2d, 0x78, 0x6b, 0x28, 0x7e, 0xe1, 0xda, 0x3a, 0xe2, 0x84, 0x5b, 0x27, 0x89,
                0xb5, 0x1c, 0xcd, 0xc3, 0x82, 0xef, 0x83, 0x68, 0xe0, 0x36, 0x50, 0x87, 0x9c,
                0x71, 0x75, 0x5b, 0x7f, 0xda, 0x46, 0x6b, 0x44, 0xa7, 0x32, 0x18, 0xf6, 0x82,
                0x06, 0x25, 0xe9, 0x59, 0x2f, 0xcc, 0xb3, 0xa6, 0x13, 0x3b, 0x92, 0xb2,
            ],
        }
    }

    #[test]
    fn appendix_a_hex_matches_fields() {
        let fields = test_vector_fields();
        let bytes = bytes_from_hex();
        let dec = decode_receipt(&bytes).expect("normative vector must decode");
        assert_eq!(dec, fields);
    }

    #[test]
    fn encoder_reproduces_test_vector() {
        let fields = test_vector_fields();
        let encoded = encode_receipt(&fields);
        assert_eq!(encoded, bytes_from_hex());
    }

    /// Deterministic byte layout of the fixed Appendix-A vector:
    /// `A7 | (00 58 20 h32) | (01 50 nu16) | (02 58 20 node32) |
    /// (03 1B tin8) | (04 1B tout8) | (05 58 20 prev32) | (06 58 40 sig64)`.
    const K0: usize = 1; // h key
    const K3: usize = 89; // tin key
    const K6: usize = 144; // sig key
    const LEN_TOTAL: usize = 211; // 1 + 35 + 18 + 35 + 10 + 10 + 35 + 67

    #[test]
    fn fixed_layout_holds() {
        let bytes = bytes_from_hex();
        assert_eq!(bytes.len(), LEN_TOTAL);
        assert_eq!(bytes[K0], 0x00);
        assert_eq!(bytes[K3], 0x03);
        assert_eq!(bytes[K3 + 1], 0x1b);
        assert_eq!(bytes[K6], 0x06);
        assert_eq!(bytes[K6 + 1], 0x58);
        assert_eq!(bytes[LEN_TOTAL - 1], 0xb2);
    }

    #[test]
    fn signing_payload_is_six_key_map() {
        let fields = test_vector_fields();
        let payload = encode_signing_payload(
            &fields.h,
            &fields.nu,
            &fields.node,
            fields.tin,
            fields.tout,
            &fields.prev,
        );
        // Map header 0xA6 (6 entries); full vector has sig appended.
        assert_eq!(payload[0], 0xa6);
        let full = bytes_from_hex();
        assert_eq!(payload.len(), full.len() - (1 + 2 + 64));
    }

    #[test]
    fn trailing_bytes_rejected() {
        let mut bytes = bytes_from_hex();
        bytes.push(0x00);
        assert_eq!(decode_receipt(&bytes), Err(WireError::TrailingData));
    }

    #[test]
    fn wrong_bstr_length_rejected() {
        let fields = test_vector_fields();
        let mut bytes = encode_receipt(&fields);
        // The h byte-string length head sits at K0+1; 0x59 = byte-string major
        // type with a 2-byte length, which parses to 0x83A0 ≠ 32.
        bytes[K0 + 1] = 0x59;
        assert_eq!(decode_receipt(&bytes), Err(WireError::BadLength));
    }

    #[test]
    fn unknown_key_rejected() {
        let fields = test_vector_fields();
        let mut bytes = encode_receipt(&fields);
        bytes[K6] = 0x07; // sig key -> unknown key
        assert_eq!(decode_receipt(&bytes), Err(WireError::UnknownKey));
    }

    #[test]
    fn non_canonical_2byte_timestamp_rejected() {
        let fields = test_vector_fields();
        let mut bytes = encode_receipt(&fields);
        // Replace the tin head (0x1B) with a 4-byte form and shrink the value.
        bytes[K3 + 1] = 0x1a;
        bytes.drain(K3 + 2..K3 + 6);
        assert_eq!(decode_receipt(&bytes), Err(WireError::NonCanonical));
    }

    #[test]
    fn unordered_keys_rejected() {
        let fields = test_vector_fields();
        let mut bytes = encode_receipt(&fields);
        // tin and tout share the same value schema (8-byte uint), so swapping
        // their keys keeps parsing valid until the 0x03 key follows 0x04.
        assert_eq!(bytes[K3], 0x03);
        assert_eq!(bytes[K3 + 10], 0x04); // tout key, right after tin's 8-byte value
        bytes[K3] = 0x04;
        bytes[K3 + 10] = 0x03;
        assert_eq!(decode_receipt(&bytes), Err(WireError::UnorderedKey));
    }
}