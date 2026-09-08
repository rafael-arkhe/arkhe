//! Compact serial protocol between firmware and host.
//!
//! Wire format is a strict, fixed **subset of CBOR (RFC 8949)** covering only
//! the primitives this crate emits — unsigned integers, negative integers,
//! booleans, and arrays. No dynamic allocation: encoding writes into
//! `heapless::Vec`, decoding reads from a borrowed byte slice and fails on
//! anything outside the subset (lenient parsing is treated as a security
//! hazard on embedded targets).
//!
//! # Messages
//!
//! ```text
//! FirmwareReport (device -> host)
//!   [ 1                          ; discriminant
//!   , device_id: u16             ; 0x0000..0xFFFF
//!   , phi_milli: u16             ; Gap-1 window 578..999
//!   , mask: u8                   ; passing invariants bitmask
//!   , constitutional: bool       ; C-01 ∧ C-02
//!   , temperature_tenths: i16    ; °C × 10
//!   , uptime_ticks: u32          ; monotonic
//!   ]
//!
//! HostDecision (host -> device)
//!   [ 2
//!   , action: u8                 ; FirmwareAction discriminant
//!   , phi_threshold_milli: u16   ; new Φ threshold (Reconfigure only)
//!   , rollback: bool             ; restore last healthy state
//!   ]
//! ```

use heapless::Vec;

use crate::invariants::CoherenceState;
use crate::minimal_bridge::Assessment;

/// Maximum encoded size of a [`FirmwareReport`].
pub const REPORT_BUF: usize = 64;
/// Maximum encoded size of a [`HostDecision`].
pub const DECISION_BUF: usize = 32;

/// CBOR subset decoding error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CborError {
    /// Buffer too small to hold the encoded message.
    BufferOverflow,
    /// Unexpected end of input.
    Truncated,
    /// Byte is outside the allowed CBOR subset.
    Unsupported{ byte: u8 },
    /// An integer exceeds the representable domain of the target field.
    Overflow,
    /// A boolean field was not CBOR true/false.
    ExpectedBool,
    /// The array length does not match the expected message shape.
    WrongArity,
    /// The discriminant did not match the expected message.
    WrongDiscriminant,
}

impl core::fmt::Display for CborError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            Self::BufferOverflow => "encoded message exceeds fixed buffer",
            Self::Truncated => "unexpected end of input",
            Self::Unsupported { byte } => return write!(f, "unsupported CBOR byte 0x{byte:02x}"),
            Self::Overflow => "integer out of field domain",
            Self::ExpectedBool => "expected CBOR boolean",
            Self::WrongArity => "unexpected array length",
            Self::WrongDiscriminant => "unexpected message discriminant",
        };
        f.write_str(msg)
    }
}

// --- CBOR subset primitives -------------------------------------------------

/// Minimum additional data byte for two-byte head.
const U8_INDEF: u8 = 24;
/// Two-byte (16-bit) head marker.
const U16_HEAD: u8 = 25;
/// Four-byte (32-bit) head marker.
const U32_HEAD: u8 = 26;

/// Encode an unsigned integer in CBOR major-0 form.
fn cbor_write_uint<const N: usize>(
    out: &mut Vec<u8, N>,
    value: u64,
) -> Result<(), CborError> {
    // Head: major 0
    if value < U8_INDEF as u64 {
        out.push(value as u8).map_err(|_| CborError::BufferOverflow)
    } else if value <= u8::MAX as u64 {
        out.push(0x18).map_err(|_| CborError::BufferOverflow)?;
        out.push(value as u8).map_err(|_| CborError::BufferOverflow)
    } else if value <= u16::MAX as u64 {
        out.push(U16_HEAD).map_err(|_| CborError::BufferOverflow)?;
        out.push((value >> 8) as u8).map_err(|_| CborError::BufferOverflow)?;
        out.push(value as u8).map_err(|_| CborError::BufferOverflow)
    } else {
        out.push(U32_HEAD).map_err(|_| CborError::BufferOverflow)?;
        let bytes = value.to_be_bytes();
        for &b in &bytes {
            out.push(b).map_err(|_| CborError::BufferOverflow)?;
        }
        Ok(())
    }
}

/// Encode an integer (may be negative) using CBOR major 0 or major 1.
fn cbor_write_i16<const N: usize>(out: &mut Vec<u8, N>, value: i16) -> Result<(), CborError> {
    if value >= 0 {
        cbor_write_uint(out, value as u64)
    } else {
        // CBOR negative: encodes -(n+1)
        let n = (-(value as i32) - 1) as u64;
        if n < U8_INDEF as u64 {
            out.push(0x20 | n as u8).map_err(|_| CborError::BufferOverflow)
        } else {
            out.push(0x38).map_err(|_| CborError::BufferOverflow)?;
            out.push(n as u8).map_err(|_| CborError::BufferOverflow)
        }
    }
}

/// Encode a boolean (CBOR simple values true/false).
fn cbor_write_bool<const N: usize>(
    out: &mut Vec<u8, N>,
    value: bool,
) -> Result<(), CborError> {
    let b = if value { 0xf5 } else { 0xf4 };
    out.push(b).map_err(|_| CborError::BufferOverflow)
}

/// Reader over a borrowed slice with position tracking.
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    const fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn byte(&mut self) -> Result<u8, CborError> {
        let b = *self.buf.get(self.pos).ok_or(CborError::Truncated)?;
        self.pos += 1;
        Ok(b)
    }

    /// Read the 16-bit payload following a `U16_HEAD` head byte.
    fn two_bytes(&mut self) -> Result<u64, CborError> {
        let hi = self.byte()? as u64;
        let lo = self.byte()? as u64;
        Ok((hi << 8) | lo)
    }

    /// Read the 32-bit payload following a `U32_HEAD` head byte.
    fn four_bytes(&mut self) -> Result<u64, CborError> {
        let mut v = 0u64;
        for _ in 0..4 {
            v = (v << 8) | self.byte()? as u64;
        }
        Ok(v)
    }

    /// Read a CBOR unsigned integer (major 0).
    fn uint(&mut self) -> Result<u64, CborError> {
        let b = self.byte()?;
        if b < U8_INDEF {
            Ok(b as u64)
        } else if b == 0x18 {
            Ok(self.byte()? as u64)
        } else if b == U16_HEAD {
            self.two_bytes()
        } else if b == U32_HEAD {
            self.four_bytes()
        } else {
            Err(CborError::Unsupported { byte: b })
        }
    }

    /// Read a CBOR signed integer (major 0 or major 1).
    fn i16(&mut self) -> Result<i16, CborError> {
        let b = self.byte()?;
        if b < U8_INDEF {
            Ok(b as i16)
        } else if b == 0x18 {
            let raw = self.byte()? as u64;
            if raw > i16::MAX as u64 {
                return Err(CborError::Overflow);
            }
            Ok(raw as i16)
        } else if b == U16_HEAD {
            let raw = self.two_bytes()?;
            if raw > i16::MAX as u64 {
                return Err(CborError::Overflow);
            }
            Ok(raw as i16)
        } else if b == U32_HEAD {
            let raw = self.four_bytes()?;
            if raw > i16::MAX as u64 {
                return Err(CborError::Overflow);
            }
            Ok(raw as i16)
        } else if (0x20..=0x37).contains(&b) {
            let n = (b & 0x1f) as i64;
            Ok((-1 - n) as i16)
        } else if b == 0x38 {
            let n = self.byte()? as i64;
            let value = -1 - n;
            if value < i16::MIN as i64 {
                return Err(CborError::Overflow);
            }
            Ok(value as i16)
        } else {
            Err(CborError::Unsupported { byte: b })
        }
    }

    /// Read a CBOR boolean (strict: only true/false simple values).
    fn boolean(&mut self) -> Result<bool, CborError> {
        let b = self.byte()?;
        match b {
            0xf4 => Ok(false),
            0xf5 => Ok(true),
            other => Err(CborError::Unsupported { byte: other }),
        }
    }

    /// Read a CBOR array header of exactly `expected` items.
    fn array_len(&mut self, expected: usize) -> Result<(), CborError> {
        let b = self.byte()?;
        if (0x80..0x98).contains(&b) {
            let len = (b & 0x1f) as usize;
            if len == expected {
                Ok(())
            } else {
                Err(CborError::WrongArity)
            }
        } else {
            Err(CborError::Unsupported { byte: b })
        }
    }
}

// --- Messages ---------------------------------------------------------------

/// Action byte the host can command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareAction {
    /// Continue normal operation.
    Continue,
    /// Roll back to the last healthy coherence state.
    Rollback,
    /// Notify the host (out-of-band alert).
    AlertHost,
    /// Reconfigure the operational Φ threshold.
    ReconfigureThreshold,
}

impl FirmwareAction {
    /// Decode from the wire discriminant.
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::Continue),
            1 => Some(Self::Rollback),
            2 => Some(Self::AlertHost),
            3 => Some(Self::ReconfigureThreshold),
            _ => None,
        }
    }

    /// Wire discriminant.
    pub const fn to_byte(self) -> u8 {
        match self {
            Self::Continue => 0,
            Self::Rollback => 1,
            Self::AlertHost => 2,
            Self::ReconfigureThreshold => 3,
        }
    }
}

/// Telemetry report emitted by the device to the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirmwareReport {
    /// Device identifier (e.g., last two bytes of UID).
    pub device_id: u16,
    /// Integer Φ in milli-units (Gap-1 window 578..999).
    pub phi_milli: u16,
    /// Passing-invariant bitmask (C-01..C-04).
    pub mask: u8,
    /// Whether the constitutional invariants (C-01 ∧ C-02) hold.
    pub constitutional: bool,
    /// Core temperature in °C × 10.
    pub temperature_tenths: i16,
    /// Monotonic uptime in ticks.
    pub uptime_ticks: u32,
}

impl FirmwareReport {
    /// Build a report from a coherence snapshot and its assessment.
    pub const fn new(
        device_id: u16,
        state: &CoherenceState,
        assessment: &Assessment,
    ) -> Self {
        Self {
            device_id,
            phi_milli: assessment.phi_milli,
            mask: assessment.mask,
            constitutional: assessment.constitutional,
            temperature_tenths: state.temperature_tenths,
            uptime_ticks: state.uptime_ticks,
        }
    }

    /// Encode into a fixed-capacity buffer.
    pub fn encode(&self) -> Result<Vec<u8, REPORT_BUF>, CborError> {
        let mut out = Vec::new();
        // array(7)
        out.push(0x87).map_err(|_| CborError::BufferOverflow)?;
        // discriminant 1
        cbor_write_uint(&mut out, 1)?;
        cbor_write_uint(&mut out, self.device_id as u64)?;
        cbor_write_uint(&mut out, self.phi_milli as u64)?;
        cbor_write_uint(&mut out, self.mask as u64)?;
        cbor_write_bool(&mut out, self.constitutional)?;
        cbor_write_i16(&mut out, self.temperature_tenths)?;
        cbor_write_uint(&mut out, self.uptime_ticks as u64)?;
        Ok(out)
    }

    /// Strict-decode from a byte slice.
    pub fn decode(buf: &[u8]) -> Result<Self, CborError> {
        let mut r = Reader::new(buf);
        r.array_len(7)?;
        if r.uint()? != 1 {
            return Err(CborError::WrongDiscriminant);
        }
        let device_id = r.uint()?;
        let phi_milli = r.uint()?;
        let mask = r.uint()?;
        let constitutional = r.boolean()?;
        let temperature_tenths = r.i16()?;
        let uptime_ticks = r.uint()?;

        if device_id > u16::MAX as u64 {
            return Err(CborError::Overflow);
        }
        if phi_milli > u16::MAX as u64 {
            return Err(CborError::Overflow);
        }
        if mask > u8::MAX as u64 {
            return Err(CborError::Overflow);
        }
        if uptime_ticks > u32::MAX as u64 {
            return Err(CborError::Overflow);
        }
        if r.pos != buf.len() {
            // Trailing bytes are not tolerated.
            return Err(CborError::Truncated);
        }

        Ok(Self {
            device_id: device_id as u16,
            phi_milli: phi_milli as u16,
            mask: mask as u8,
            constitutional,
            temperature_tenths,
            uptime_ticks: uptime_ticks as u32,
        })
    }
}

/// Decision issued by the host to the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostDecision {
    /// Commanded action.
    pub action: FirmwareAction,
    /// New Φ threshold (meaningful only for `ReconfigureThreshold`).
    pub phi_threshold_milli: u16,
    /// Whether to restore the last healthy state.
    pub rollback: bool,
}

impl HostDecision {
    /// Encode into a fixed-capacity buffer.
    pub fn encode(&self) -> Result<Vec<u8, DECISION_BUF>, CborError> {
        let mut out = Vec::new();
        // array(4)
        out.push(0x84).map_err(|_| CborError::BufferOverflow)?;
        cbor_write_uint(&mut out, 2)?;
        cbor_write_uint(&mut out, self.action.to_byte() as u64)?;
        cbor_write_uint(&mut out, self.phi_threshold_milli as u64)?;
        cbor_write_bool(&mut out, self.rollback)?;
        Ok(out)
    }

    /// Strict-decode from a byte slice.
    pub fn decode(buf: &[u8]) -> Result<Self, CborError> {
        let mut r = Reader::new(buf);
        r.array_len(4)?;
        if r.uint()? != 2 {
            return Err(CborError::WrongDiscriminant);
        }
        let action_byte = r.uint()?;
        let threshold = r.uint()?;
        let rollback = r.boolean()?;

        if action_byte > u8::MAX as u64 {
            return Err(CborError::Overflow);
        }
        if threshold > u16::MAX as u64 {
            return Err(CborError::Overflow);
        }
        if r.pos != buf.len() {
            return Err(CborError::Truncated);
        }

        let action = FirmwareAction::from_byte(action_byte as u8)
            .ok_or(CborError::Unsupported { byte: action_byte as u8 })?;

        Ok(Self {
            action,
            phi_threshold_milli: threshold as u16,
            rollback,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report() -> FirmwareReport {
        let state = CoherenceState::healthy();
        let mut bridge = crate::minimal_bridge::MinimalConsciousnessBridge::new();
        let assessment = bridge.assess(state);
        FirmwareReport::new(0xA5B2, &state, &assessment)
    }

    #[test]
    fn report_round_trip() {
        let r = sample_report();
        let encoded = r.encode().unwrap();
        let decoded = FirmwareReport::decode(&encoded).unwrap();
        assert_eq!(decoded, r);
    }

    #[test]
    fn decision_round_trip() {
        let d = HostDecision {
            action: FirmwareAction::ReconfigureThreshold,
            phi_threshold_milli: 650,
            rollback: false,
        };
        let encoded = d.encode().unwrap();
        let decoded = HostDecision::decode(&encoded).unwrap();
        assert_eq!(decoded, d);
    }

    #[test]
    fn negative_temperature_survives() {
        let mut r = sample_report();
        r.temperature_tenths = -30;
        let encoded = r.encode().unwrap();
        let decoded = FirmwareReport::decode(&encoded).unwrap();
        assert_eq!(decoded.temperature_tenths, -30);
    }

    #[test]
    fn trailing_bytes_rejected() {
        let r = sample_report();
        let mut encoded = r.encode().unwrap();
        encoded.push(0x00).unwrap();
        assert!(FirmwareReport::decode(&encoded).is_err());
    }

    #[test]
    fn truncated_input_rejected() {
        let r = sample_report();
        let encoded = r.encode().unwrap();
        assert!(FirmwareReport::decode(&encoded[..encoded.len() - 2]).is_err());
    }

    #[test]
    fn wrong_discriminant_rejected() {
        let d = HostDecision {
            action: FirmwareAction::Continue,
            phi_threshold_milli: 600,
            rollback: false,
        };
        let encoded = d.encode().unwrap();
        assert!(FirmwareReport::decode(&encoded).is_err());
    }

    #[test]
    fn unknown_action_rejected() {
        // array(4) disc=2 action=9 (undefined) threshold=600 rollback=false
        let buf = [0x84, 0x02, 0x09, 0x60, 0x00, 0xf4];
        assert!(HostDecision::decode(&buf).is_err());
    }

    #[test]
    fn non_boolean_in_bool_slot_rejected() {
        // array(7) disc=1 device=0x0010 phi=0x6000 mask=0x08,
        // constitutional = 0x00 (unsupported, not 0xf4) ...
        let mut encoded = sample_report().encode().unwrap();
        // Locate the constitutional bool byte (0xf5 when healthy); find first 0xf4/0xf5.
        let idx = encoded
            .iter()
            .position(|&b| b == 0xf4 || b == 0xf5)
            .expect("healthy report must contain a bool");
        encoded[idx] = 0x00;
        assert!(FirmwareReport::decode(&encoded).is_err());
    }

    #[test]
    fn fixnum_short_form_used_for_small_values() {
        let d = HostDecision {
            action: FirmwareAction::Continue,
            phi_threshold_milli: 600,
            rollback: false,
        };
        let encoded = d.encode().unwrap();
        // disc 2 and action 0 are short-form single bytes
        assert_eq!(encoded[1], 0x02);
        assert_eq!(encoded[2], 0x00);
    }

    #[test]
    fn game_actions_mapped() {
        assert_eq!(FirmwareAction::from_byte(0), Some(FirmwareAction::Continue));
        assert_eq!(FirmwareAction::from_byte(1), Some(FirmwareAction::Rollback));
        assert_eq!(FirmwareAction::from_byte(2), Some(FirmwareAction::AlertHost));
        assert_eq!(FirmwareAction::from_byte(3), Some(FirmwareAction::ReconfigureThreshold));
        assert_eq!(FirmwareAction::from_byte(7), None);
    }

    #[test]
    fn decode_rejects_unsupported_head() {
        // 0x9f = indefinite-length array: out of subset
        let bad = [0x9f];
        assert!(FirmwareReport::decode(&bad).is_err());
        assert!(HostDecision::decode(&bad).is_err());
    }
}