//! OrchOR state type — serialized as the payload of AFT frames.

use rand::rngs::OsRng;
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Serialized size of an `OrchORState`: 4×f64 (32) + 12×u16 (24) + 1 u8 = 57 bytes.
pub const ORCH_OR_STATE_LEN: usize = 57;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OrchORState {
    pub timestamp: f64,
    pub coherence_time: f64,
    pub frequency: f64,
    pub energy: f64,
    pub hexagon_state: [u16; 12],
    pub regime: u8,
}

impl OrchORState {
    /// CSPRNG-backed random state (OsRng).
    pub fn random() -> Self {
        let mut rng = OsRng;
        Self {
            timestamp: rng.gen(),
            coherence_time: rng.gen(),
            frequency: rng.gen(),
            energy: rng.gen(),
            hexagon_state: core::array::from_fn(|_| rng.gen()),
            regime: rng.gen(),
        }
    }

    pub fn to_bytes(&self) -> [u8; ORCH_OR_STATE_LEN] {
        let mut out = [0u8; ORCH_OR_STATE_LEN];
        out[0..8].copy_from_slice(&self.timestamp.to_le_bytes());
        out[8..16].copy_from_slice(&self.coherence_time.to_le_bytes());
        out[16..24].copy_from_slice(&self.frequency.to_le_bytes());
        out[24..32].copy_from_slice(&self.energy.to_le_bytes());
        for (i, v) in self.hexagon_state.iter().enumerate() {
            let off = 32 + i * 2;
            out[off..off + 2].copy_from_slice(&v.to_le_bytes());
        }
        out[ORCH_OR_STATE_LEN - 1] = self.regime;
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != ORCH_OR_STATE_LEN {
            return None;
        }
        let timestamp = f64::from_le_bytes(bytes[0..8].try_into().ok()?);
        let coherence_time = f64::from_le_bytes(bytes[8..16].try_into().ok()?);
        let frequency = f64::from_le_bytes(bytes[16..24].try_into().ok()?);
        let energy = f64::from_le_bytes(bytes[24..32].try_into().ok()?);
        let mut hexagon_state = [0u16; 12];
        for (i, v) in hexagon_state.iter_mut().enumerate() {
            let off = 32 + i * 2;
            *v = u16::from_le_bytes(bytes[off..off + 2].try_into().ok()?);
        }
        Some(Self {
            timestamp,
            coherence_time,
            frequency,
            energy,
            hexagon_state,
            regime: *bytes.last()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_bytes() {
        let s = OrchORState::random();
        let bytes = s.to_bytes();
        let back = OrchORState::from_bytes(&bytes).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn wrong_len_rejected() {
        assert!(OrchORState::from_bytes(&[0u8; 10]).is_none());
    }
}
