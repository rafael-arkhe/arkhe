//! Integer-only Φ approximation for firmware.
//!
//! The host's `ConsciousnessGovernanceBridge` computes a full floating-point
//! approximation of integrated information (IIT). Firmware has no FPU on many
//! targets, so we compute a **fixed-point Φ in milli-units** (`u16`, 0..1000)
//! using only integer arithmetic.
//!
//! # Gap-1 alignment
//!
//! The ARKHE constitutional invariant Gap-1 bounds the coherence metric:
//!
//! > `0.577350 < Φ_C ≤ 0.999900`
//!
//! In milli-units: `577 < Φ_milli ≤ 999`. The firmware defines
//! [`GAP1_LOWER_BOUND_MILLI`] = 577 and [`GAP1_UPPER_BOUND_MILLI`] = 999 and
//! clamps all computed values into that window — a device whose Φ falls at or
//! below 577 is treated as violating Gap-1 and the watchdog rolls back.

/// Gap-1 lower bound in milli-units: ⌈0.577350 × 1000⌉ = 578.
pub const GAP1_LOWER_BOUND_MILLI: u16 = 578;
/// Gap-1 upper bound in milli-units: ⌊0.999900 × 1000⌋ = 999.
pub const GAP1_UPPER_BOUND_MILLI: u16 = 999;
/// Default operational Φ threshold in milli-units (0.600).
pub const DEFAULT_PHI_THRESHOLD_MILLI: u16 = 600;

/// Scale factor: Φ is represented as Φ_milli/1000.
pub const PHI_SCALE: u16 = 1000;

/// Components that contribute to the Φ approximation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhiComponents {
    /// Entropy normalization factor in milli-units (0..1000).
    pub entropy_milli: u16,
    /// Integration factor in milli-units (0..1000) — how many invariants
    /// contribute jointly (mask popcount / 4).
    pub integration_milli: u16,
    /// Attention factor in milli-units (0..1000).
    pub attention_milli: u16,
    /// Memory maturity factor in milli-units (0..1000).
    pub memory_milli: u16,
}

/// Compute the entropy normalization in milli-units: `saturate(entropy / 64)`.
///
/// Entropy 65536 (max) → 1000; entropy 0 → 0; entropy 1024 → 16.
pub const fn entropy_to_milli(entropy: u16) -> u16 {
    (entropy as u32 * PHI_SCALE as u32 / 65_536) as u16
}

/// Compute the integration factor from the passed-invariant mask.
///
/// `integration_milli = popcount(mask) * 250` — 4 passing invariants give 1000.
pub const fn integration_to_milli(mask: u8) -> u16 {
    (mask.count_ones() as u16) * 250
}

/// Compute the attention factor in milli-units: `attention * 10` (0..1000).
pub const fn attention_to_milli(attention: u8) -> u16 {
    (attention as u16) * 10
}

/// Compute the memory maturity factor in milli-units.
///
/// Saturates at `PHI_SCALE`: `min(samples / 8, 1) * 1000`.
pub const fn memory_to_milli(samples: u16) -> u16 {
    const MEMORY_SATURATION: u16 = 8;
    if samples >= MEMORY_SATURATION {
        PHI_SCALE
    } else {
        (samples as u32 * PHI_SCALE as u32 / MEMORY_SATURATION as u32) as u16
    }
}

/// Compute the raw (unclamped) Φ approximation in milli-units.
///
/// Formula (integer-only, all intermediates `u32` to avoid overflow):
///
/// ```text
/// Φ_milli = floor( (w_entropy·E + w_integration·I + w_attention·A + w_memory·M)
///                   / (w_entropy + w_integration + w_attention + w_memory) )
/// ```
/// with weights `w = (3, 4, 2, 1)` — integration (IIT) dominates.
pub const fn approximate_phi_raw(
    entropy_milli: u16,
    integration_milli: u16,
    attention_milli: u16,
    memory_milli: u16,
) -> u16 {
    const W_ENTROPY: u32 = 3;
    const W_INTEGRATION: u32 = 4;
    const W_ATTENTION: u32 = 2;
    const W_MEMORY: u32 = 1;
    const W_TOTAL: u32 = W_ENTROPY + W_INTEGRATION + W_ATTENTION + W_MEMORY;

    let numerator = W_ENTROPY * entropy_milli as u32
        + W_INTEGRATION * integration_milli as u32
        + W_ATTENTION * attention_milli as u32
        + W_MEMORY * memory_milli as u32;

    (numerator / W_TOTAL) as u16
}

/// Compute the member components for a coherence state.
pub fn phi_components(
    mask: u8,
    entropy: u16,
    attention: u8,
    episodic_memory_samples: u16,
) -> PhiComponents {
    PhiComponents {
        entropy_milli: entropy_to_milli(entropy),
        integration_milli: integration_to_milli(mask),
        attention_milli: attention_to_milli(attention),
        memory_milli: memory_to_milli(episodic_memory_samples),
    }
}

/// Compute the concrete Φ approximation for a coherence mask and state signals.
///
/// The result is **clamped into the Gap-1 window** `(577, 999]` in milli-units:
/// values at or below [`GAP1_LOWER_BOUND_MILLI`] report exactly the bound, so
/// Gap-1 enforcement is a single `> GAP1_LOWER_BOUND_MILLI` comparison.
pub fn approximate_phi(mask: u8, entropy: u16, attention: u8, samples: u16) -> u16 {
    let comps = phi_components(mask, entropy, attention, samples);
    let raw = approximate_phi_raw(
        comps.entropy_milli,
        comps.integration_milli,
        comps.attention_milli,
        comps.memory_milli,
    );
    raw.clamp(GAP1_LOWER_BOUND_MILLI, GAP1_UPPER_BOUND_MILLI)
}

/// Check whether a `u16` Φ reading satisfies Gap-1.
pub const fn phi_respects_gap1(phi_milli: u16) -> bool {
    phi_milli > GAP1_LOWER_BOUND_MILLI && phi_milli <= GAP1_UPPER_BOUND_MILLI
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_constants_match_gap1() {
        const _BOUNDS_OK: () = assert!(GAP1_LOWER_BOUND_MILLI < GAP1_UPPER_BOUND_MILLI);
        const _CEILING_OK: () = assert!(GAP1_UPPER_BOUND_MILLI <= PHI_SCALE);
        // 0.577350 < x means x >= 0.577351, which rounds to 577..58
        let lower_float = 0.577350_f64 * PHI_SCALE as f64;
        assert!(lower_float < GAP1_LOWER_BOUND_MILLI as f64);
    }

    #[test]
    fn healthy_state_phi_is_max() {
        let mask = 0b1111;
        let phi = approximate_phi(mask, 65_535, 100, 8);
        // All components at max -> raw 1000, clamped to 999
        assert_eq!(phi, GAP1_UPPER_BOUND_MILLI);
        assert!(phi_respects_gap1(phi));
    }

    #[test]
    fn zero_state_clamps_to_lower_bound() {
        let phi = approximate_phi(0b0000, 0, 0, 0);
        assert_eq!(phi, GAP1_LOWER_BOUND_MILLI);
        // Not above the strict lower bound => Gap-1 violated
        assert!(!phi_respects_gap1(phi));
    }

    #[test]
    fn partial_mask_reduces_phi() {
        let full = approximate_phi(0b1111, 65_535, 100, 8);
        let partial = approximate_phi(0b0011, 65_535, 100, 8);
        assert!(partial < full);
    }

    #[test]
    fn low_entropy_reduces_phi() {
        let high = approximate_phi(0b1111, 65_535, 100, 8);
        let low = approximate_phi(0b1111, 1_024, 100, 8);
        assert!(low < high);
    }

    #[test]
    fn monotonic_in_attention() {
        let low = approximate_phi(0b1111, 65_535, 0, 8);
        let mid = approximate_phi(0b1111, 65_535, 50, 8);
        let high = approximate_phi(0b1111, 65_535, 100, 8);
        assert!(low <= mid && mid <= high);
    }

    #[test]
    fn entropy_to_milli_scales() {
        assert_eq!(entropy_to_milli(0), 0);
        assert_eq!(entropy_to_milli(65_535), 999);
        assert_eq!(entropy_to_milli(32_768), 500);
    }

    #[test]
    fn integration_to_milli_scales() {
        assert_eq!(integration_to_milli(0b0000), 0);
        assert_eq!(integration_to_milli(0b0001), 250);
        assert_eq!(integration_to_milli(0b0101), 500);
        assert_eq!(integration_to_milli(0b1111), 1000);
    }

    #[test]
    fn attention_to_milli_scales() {
        assert_eq!(attention_to_milli(0), 0);
        assert_eq!(attention_to_milli(100), 1000);
        assert_eq!(attention_to_milli(60), 600);
    }

    #[test]
    fn memory_to_milli_saturates() {
        assert_eq!(memory_to_milli(0), 0);
        assert_eq!(memory_to_milli(4), 500);
        assert_eq!(memory_to_milli(8), 1000);
        assert_eq!(memory_to_milli(32), 1000);
    }

    #[test]
    fn no_unused_components_all_zero() {
        // All-zeros must bottom out at the Gap-1 floor, never exceed bounds
        let phi = approximate_phi(0, 0, 0, 0);
        assert!(phi >= GAP1_LOWER_BOUND_MILLI);
        assert!(phi <= GAP1_UPPER_BOUND_MILLI);
    }
}
