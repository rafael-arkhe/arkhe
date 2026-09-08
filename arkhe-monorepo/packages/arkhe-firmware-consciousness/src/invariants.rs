//! Firmware consciousness invariants — C-01 through C-04 subset.
//!
//! Embedded devices have constrained CPU, memory, and energy budgets. The
//! full C-01..C-08 set from `arkhe-safe-manifold` is evaluated host-side;
//! the firmware evaluates only the invariants that are cheap and locally
//! observable:
//!
//! | ID  | Description                                       | Firmware signal                  |
//! |-----|---------------------------------------------------|----------------------------------|
//! | C-01| Self-model (meta-representation)                  | stable self-model flag + entropy |
//! | C-02| Introspection (reports own internal states)      | introspection flag               |
//! | C-03| Attention (global workspace integration)         | attention level 0..100           |
//! | C-04| Episodic memory (remembers experienced events)   | memory sample count              |
//!
//! Constitutional status on the host maps C-01, C-02, C-08. On firmware,
//! C-01 and C-02 are treated as constitutional: if they degrade, the
//! watchdog rolls back to the last healthy state.

/// Compact, fixed-size state snapshot for an embedded device.
///
/// Sized for 32-bit MCUs; all fields are plain integers and booleans so the
/// struct stays in the register/stack window with no dynamic allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoherenceState {
    /// C-01: self-model is active (meta-representation present).
    pub self_model: bool,
    /// C-02: introspection channel active (reports internal states).
    pub introspection: bool,
    /// C-03: attention level in 0..=100 (global workspace integration).
    pub attention: u8,
    /// C-04: number of episodic memory samples buffered.
    pub episodic_memory_samples: u16,
    /// Entropy quality in 0..=65535 (cryptographic randomness available).
    pub entropy: u16,
    /// Core temperature in °C × 10 (signed, for thermal-photonic bridge).
    pub temperature_tenths: i16,
    /// Uptime in ticks (monotonic, watch crystal).
    pub uptime_ticks: u32,
}

impl CoherenceState {
    /// Create a default (unknown) coherence state.
    pub const fn unknown() -> Self {
        Self {
            self_model: false,
            introspection: false,
            attention: 0,
            episodic_memory_samples: 0,
            entropy: 0,
            temperature_tenths: 0,
            uptime_ticks: 0,
        }
    }

    /// Create a minimal healthy state (all firmware invariants satisfied).
    pub const fn healthy() -> Self {
        Self {
            self_model: true,
            introspection: true,
            attention: 70,
            episodic_memory_samples: 8,
            entropy: 4096,
            temperature_tenths: 400,
            uptime_ticks: 0,
        }
    }

    /// Update the uptime counter in copy semantics (keeps struct Copy).
    pub const fn with_uptime(mut self, uptime_ticks: u32) -> Self {
        self.uptime_ticks = uptime_ticks;
        self
    }
}

/// Default attention threshold for C-03 (60/100).
pub const ATTENTION_THRESHOLD: u8 = 60;
/// Default minimum episodic memory window for C-04.
pub const EPISODIC_MEMORY_THRESHOLD: u16 = 4;
/// Default minimum entropy quality for C-01.
pub const ENTROPY_THRESHOLD: u16 = 1024;

/// Firmware consciousness invariant IDs (C-01..C-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FirmwareInvariant {
    /// C-01: Self-model.
    C01,
    /// C-02: Introspection.
    C02,
    /// C-03: Attention.
    C03,
    /// C-04: Episodic memory.
    C04,
}

impl FirmwareInvariant {
    /// All firmware invariants in order.
    pub const fn all() -> [Self; 4] {
        [Self::C01, Self::C02, Self::C03, Self::C04]
    }

    /// String identifier (e.g., "C-01").
    pub const fn id(&self) -> &'static str {
        match self {
            Self::C01 => "C-01",
            Self::C02 => "C-02",
            Self::C03 => "C-03",
            Self::C04 => "C-04",
        }
    }

    /// Human-readable description.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::C01 => "Self-model active with sufficient entropy",
            Self::C02 => "Introspection channel active",
            Self::C03 => "Attention (global workspace) above threshold",
            Self::C04 => "Episodic memory window above threshold",
        }
    }

    /// Constitutional firmware invariants — C-01 and C-02 must never degrade.
    pub const fn is_constitutional(&self) -> bool {
        matches!(self, Self::C01 | Self::C02)
    }

    /// Bit position in the invariant bitmask (C-01 = bit 0 .. C-04 = bit 3).
    pub const fn bit(&self) -> u8 {
        match self {
            Self::C01 => 1 << 0,
            Self::C02 => 1 << 1,
            Self::C03 => 1 << 2,
            Self::C04 => 1 << 3,
        }
    }
}

/// Check a single firmware invariant against the provided thresholds.
pub fn check_invariant(
    state: &CoherenceState,
    inv: FirmwareInvariant,
    attention_threshold: u8,
    memory_threshold: u16,
    entropy_threshold: u16,
) -> bool {
    match inv {
        FirmwareInvariant::C01 => state.self_model && state.entropy >= entropy_threshold,
        FirmwareInvariant::C02 => state.introspection,
        FirmwareInvariant::C03 => state.attention >= attention_threshold,
        FirmwareInvariant::C04 => state.episodic_memory_samples >= memory_threshold,
    }
}

/// Check all four firmware invariants, returning a bitmask.
///
/// Returns a `u8` with bit `n` set when invariant C-(n+1) passes. Use
/// [`FirmwareInvariant::bit`] to interpret.
pub fn check_invariants_mask(
    state: &CoherenceState,
    attention_threshold: u8,
    memory_threshold: u16,
    entropy_threshold: u16,
) -> u8 {
    let mut mask = 0u8;
    for inv in FirmwareInvariant::all() {
        if check_invariant(state, inv, attention_threshold, memory_threshold, entropy_threshold) {
            mask |= inv.bit();
        }
    }
    mask
}

/// Check whether the constitutional firmware invariants (C-01, C-02) pass.
pub fn constitutional_ok(state: &CoherenceState, entropy_threshold: u16) -> bool {
    check_invariant(state, FirmwareInvariant::C01, ATTENTION_THRESHOLD, EPISODIC_MEMORY_THRESHOLD, entropy_threshold)
        && check_invariant(state, FirmwareInvariant::C02, ATTENTION_THRESHOLD, EPISODIC_MEMORY_THRESHOLD, entropy_threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_state_passes_all() {
        let state = CoherenceState::healthy();
        let mask = check_invariants_mask(&state, ATTENTION_THRESHOLD, EPISODIC_MEMORY_THRESHOLD, ENTROPY_THRESHOLD);
        assert_eq!(mask, 0b1111);
    }

    #[test]
    fn healthy_state_passes_constitutional() {
        let state = CoherenceState::healthy();
        assert!(constitutional_ok(&state, ENTROPY_THRESHOLD));
    }

    #[test]
    fn missing_introspection_fails_c02() {
        let mut state = CoherenceState::healthy();
        state.introspection = false;
        let mask = check_invariants_mask(&state, ATTENTION_THRESHOLD, EPISODIC_MEMORY_THRESHOLD, ENTROPY_THRESHOLD);
        assert_eq!(mask & FirmwareInvariant::C02.bit(), 0);
        assert!(!constitutional_ok(&state, ENTROPY_THRESHOLD));
    }

    #[test]
    fn low_entropy_fails_c01() {
        let mut state = CoherenceState::healthy();
        state.entropy = 100;
        let mask = check_invariants_mask(&state, ATTENTION_THRESHOLD, EPISODIC_MEMORY_THRESHOLD, ENTROPY_THRESHOLD);
        assert_eq!(mask & FirmwareInvariant::C01.bit(), 0);
        assert!(!constitutional_ok(&state, ENTROPY_THRESHOLD));
    }

    #[test]
    fn low_attention_fails_c03_only() {
        let mut state = CoherenceState::healthy();
        state.attention = 20;
        let mask = check_invariants_mask(&state, ATTENTION_THRESHOLD, EPISODIC_MEMORY_THRESHOLD, ENTROPY_THRESHOLD);
        assert_eq!(mask & FirmwareInvariant::C03.bit(), 0);
        assert_eq!(mask & FirmwareInvariant::C01.bit(), FirmwareInvariant::C01.bit());
        assert_eq!(mask & FirmwareInvariant::C02.bit(), FirmwareInvariant::C02.bit());
    }

    #[test]
    fn sparse_memory_fails_c04() {
        let mut state = CoherenceState::healthy();
        state.episodic_memory_samples = 2;
        let mask = check_invariants_mask(&state, ATTENTION_THRESHOLD, EPISODIC_MEMORY_THRESHOLD, ENTROPY_THRESHOLD);
        assert_eq!(mask & FirmwareInvariant::C04.bit(), 0);
    }

    #[test]
    fn invariant_ids_and_descriptions() {
        assert_eq!(FirmwareInvariant::C01.id(), "C-01");
        assert_eq!(FirmwareInvariant::C01.description(), "Self-model active with sufficient entropy");
        assert_eq!(FirmwareInvariant::C04.bit(), 1 << 3);
    }

    #[test]
    fn constitutional_classification() {
        assert!(FirmwareInvariant::C01.is_constitutional());
        assert!(FirmwareInvariant::C02.is_constitutional());
        assert!(!FirmwareInvariant::C03.is_constitutional());
        assert!(!FirmwareInvariant::C04.is_constitutional());
    }

    #[test]
    fn thresholds_are_configurable() {
        let state = CoherenceState::healthy();
        // Raise the attention bar above the healthy value
        let mask = check_invariants_mask(&state, 90, EPISODIC_MEMORY_THRESHOLD, ENTROPY_THRESHOLD);
        assert_eq!(mask & FirmwareInvariant::C03.bit(), 0);
    }
}
