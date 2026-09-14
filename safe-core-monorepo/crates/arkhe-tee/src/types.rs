//! Fixed-width byte types used by the crate's serde-visible structs.
//!
//! `serde` only implements `Serialize`/`Deserialize` for arrays up to length 32,
//! so TDX's 48-byte measurements and 64-byte `report_data` cannot be serialized
//! with a plain derive. `dcap-qvl` solves this with an internal
//! `serde-bytes`-style adapter; rather than take on another dependency, these
//! newtypes render themselves as lowercase hex. That also makes the JSON
//! written by this crate (collateral cache, provenance bindings, verification
//! reports) directly readable as evidence artifacts.

use crate::error::{TeeError, TeeResult};

macro_rules! hex_bytes {
    ($name:ident, $n:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub [u8; $n]);

        impl $name {
            /// Byte width of this type.
            pub const LEN: usize = $n;

            /// The all-zero value.
            pub const ZERO: Self = Self([0u8; $n]);

            /// Borrow the raw bytes.
            pub const fn as_bytes(&self) -> &[u8; $n] {
                &self.0
            }

            /// Owned copy of the raw bytes.
            pub const fn to_array(&self) -> [u8; $n] {
                self.0
            }

            /// Lowercase hex rendering (two characters per byte).
            pub fn to_hex(&self) -> String {
                hex::encode(self.0)
            }

            /// Parse from a hex string. Rejects wrong lengths and non-hex input.
            pub fn from_hex(value: &str) -> TeeResult<Self> {
                let raw = hex::decode(value).map_err(|e| TeeError::InvalidHex {
                    value: value.to_string(),
                    reason: e.to_string(),
                    expected_len: $n,
                })?;
                let bytes: [u8; $n] = raw.as_slice().try_into().map_err(|_| {
                    TeeError::InvalidHex {
                        value: value.to_string(),
                        reason: format!("decoded {} bytes", raw.len()),
                        expected_len: $n,
                    }
                })?;
                Ok(Self(bytes))
            }

            /// True when every byte is zero.
            pub fn is_zero(&self) -> bool {
                self.0.iter().all(|b| *b == 0)
            }
        }

        impl From<[u8; $n]> for $name {
            fn from(value: [u8; $n]) -> Self {
                Self(value)
            }
        }

        impl From<$name> for [u8; $n] {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                &self.0
            }
        }

        impl PartialEq<[u8; $n]> for $name {
            fn eq(&self, other: &[u8; $n]) -> bool {
                &self.0 == other
            }
        }

        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.to_hex())
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(&self.to_hex())
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(&self.to_hex())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = <String as serde::Deserialize>::deserialize(deserializer)?;
                Self::from_hex(&s).map_err(<D::Error as serde::de::Error>::custom)
            }
        }
    };
}

hex_bytes!(
    Digest32,
    32,
    "A 32-byte digest, e.g. a BLAKE3 binary hash or an agent artifact hash."
);
hex_bytes!(
    Digest48,
    48,
    "A 48-byte TDX measurement (MRTD, MR_CONFIG_ID, MR_OWNER, MR_OWNER_CONFIG or an RTMR)."
);
hex_bytes!(
    ReportData64,
    64,
    "The 64-byte `report_data` field of a TDX report."
);

/// SHA-384 digest width, in bytes.
pub const DIGEST48_LEN: usize = 48;

/// TDX `report_data` width, in bytes.
pub const REPORT_DATA64_LEN: usize = 64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let mut raw = [0u8; 48];
        for (i, b) in raw.iter_mut().enumerate() {
            *b = i as u8;
        }
        let d = Digest48::from(raw);
        assert_eq!(d.to_hex().len(), 96);
        assert_eq!(Digest48::from_hex(&d.to_hex()).unwrap(), d);
        assert_eq!(d.as_bytes(), &raw);
        assert!(!d.is_zero());
        assert!(Digest48::ZERO.is_zero());
    }

    #[test]
    fn from_hex_rejects_wrong_length_and_garbage() {
        match Digest48::from_hex("00") {
            Err(TeeError::InvalidHex { expected_len, .. }) => assert_eq!(expected_len, 48),
            other => panic!("expected InvalidHex, got {other:?}"),
        }
        assert!(Digest48::from_hex("zz").is_err());
    }

    #[test]
    fn json_round_trip_is_a_hex_string() {
        let d = ReportData64::from([0xabu8; 64]);
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.starts_with('"'));
        assert_eq!(json.len(), 2 + 128);
        assert_eq!(serde_json::from_str::<ReportData64>(&json).unwrap(), d);
    }

    #[test]
    fn equality_with_raw_arrays_works_both_directions_for_self() {
        let raw = [7u8; 32];
        let d = Digest32::from(raw);
        assert!(d == raw);
        assert_eq!(d.to_array(), raw);
        let back: [u8; 32] = d.into();
        assert_eq!(back, raw);
    }
}
