//! Minimal consciousness governance bridge for firmware.
//!
//! The firmware-side analogue of the host's `ConsciousnessGovernanceBridge`.
//! It keeps a bounded, ring-like history of [`CoherenceState`] snapshots in a
//! fixed-capacity `heapless::Vec` (no dynamic allocation) and exposes:
//!
//! - [`MinimalConsciousnessBridge::assess`] — evaluates C-01..C-04 and Φ;
//! - [`MinimalConsciousnessBridge::validate_modification`] — blocks state
//!   transitions that degrade Φ by more than [`MAX_PHI_DEGRADATION_MILLI`];
//! - [`MinimalConsciousnessBridge::last_healthy`] — last constitutional state
//!   for the watchdog rollback.

use heapless::Vec;

use crate::invariants::{
    check_invariants_mask, constitutional_ok, CoherenceState, ATTENTION_THRESHOLD,
    ENTROPY_THRESHOLD, EPISODIC_MEMORY_THRESHOLD,
};
use crate::phi_approx::{
    approximate_phi, phi_respects_gap1, DEFAULT_PHI_THRESHOLD_MILLI,
};

/// History capacity: 64 snapshots, each a [`CoherenceState`] (28 bytes) ->
/// 1.8 kB of RAM on a typical 16 kB target.
pub const HISTORY_CAPACITY: usize = 64;

/// Maximum allowed Φ degradation between consecutive assess calls (in
/// milli-units). A drop of more than 50 milli-units (5%) is treated as a
/// suspected state corruption and the modification is blocked.
pub const MAX_PHI_DEGRADATION_MILLI: u16 = 50;

/// Assessed outcome of a coherence evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Assessment {
    /// Integer Φ (0..1000) clamped into the Gap-1 window.
    pub phi_milli: u16,
    /// Bitmask of passing invariants (see [`FirmwareInvariant::bit`]).
    pub mask: u8,
    /// Whether Φ respects Gap-1 AND stays above the operational threshold.
    pub passed: bool,
    /// Whether C-01 and C-02 (constitutional) both hold.
    pub constitutional: bool,
}

impl Assessment {
    /// Drops in Φ from a previous assessment, saturating at 0.
    pub fn degradation_from(&self, previous_phi_milli: u16) -> u16 {
        previous_phi_milli.saturating_sub(self.phi_milli)
    }
}

/// Firmware consciousness governance bridge.
#[derive(Debug)]
pub struct MinimalConsciousnessBridge {
    /// Bounded history of coherence snapshots (newest last).
    history: Vec<CoherenceState, HISTORY_CAPACITY>,
    /// Operational Φ threshold in milli-units (default 600).
    phi_threshold_milli: u16,
    /// Entropy threshold for C-01.
    entropy_threshold: u16,
    /// Last assessment, cached to detect degradation.
    last_phi_milli: Option<u16>,
    /// Last state that was constitutional.
    last_healthy: Option<CoherenceState>,
}

impl Default for MinimalConsciousnessBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl MinimalConsciousnessBridge {
    /// Create a new bridge with default thresholds.
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            phi_threshold_milli: DEFAULT_PHI_THRESHOLD_MILLI,
            entropy_threshold: ENTROPY_THRESHOLD,
            last_phi_milli: None,
            last_healthy: None,
        }
    }

    /// Create a new bridge with a custom operational Φ threshold.
    pub fn with_phi_threshold(phi_threshold_milli: u16) -> Self {
        Self {
            phi_threshold_milli,
            ..Self::new()
        }
    }

    /// Create a new bridge with custom thresholds.
    pub fn with_thresholds(phi_threshold_milli: u16, entropy_threshold: u16) -> Self {
        Self {
            phi_threshold_milli,
            entropy_threshold,
            ..Self::new()
        }
    }

    /// Fixed capacity of the history buffer (independent of current length).
    pub const fn history_capacity(&self) -> usize {
        HISTORY_CAPACITY
    }

    /// Number of buffered snapshots.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Current operational Φ threshold in milli-units.
    pub const fn phi_threshold_milli(&self) -> u16 {
        self.phi_threshold_milli
    }

    /// Last computed Φ (if any assessment ran).
    pub const fn last_phi_milli(&self) -> Option<u16> {
        self.last_phi_milli
    }

    /// Last constitutional state, if any.
    pub const fn last_healthy(&self) -> Option<CoherenceState> {
        self.last_healthy
    }

    /// Buffer a snapshot into the bounded history (wraps when full).
    fn push_history(&mut self, state: CoherenceState) {
        if self.history.is_full() {
            // Drop the oldest to keep the window bounded.
            let _ = self.history.remove(0);
        }
        let _ = self.history.push(state);
    }

    /// Assess a coherence snapshot: compute the invariant mask, Φ, and
    /// constitutional status, then record it in history.
    pub fn assess(&mut self, state: CoherenceState) -> Assessment {
        let mask = check_invariants_mask(
            &state,
            ATTENTION_THRESHOLD,
            EPISODIC_MEMORY_THRESHOLD,
            self.entropy_threshold,
        );
        let phi = approximate_phi(mask, state.entropy, state.attention, state.episodic_memory_samples);
        let constitutional = constitutional_ok(&state, self.entropy_threshold);
        let passed = phi_respects_gap1(phi) && phi > self.phi_threshold_milli;

        self.push_history(state);
        self.last_phi_milli = Some(phi);
        if constitutional {
            self.last_healthy = Some(state);
        }

        Assessment {
            phi_milli: phi,
            mask,
            passed,
            constitutional,
        }
    }

    /// Validate that a candidate state may replace the last healthy state.
    ///
    /// The check fails (returns `false`) if either:
    /// 1. The candidate degrades Φ by more than
    ///    [`MAX_PHI_DEGRADATION_MILLI`]; or
    /// 2. The candidate violates the constitutional invariants (C-01/C-02).
    ///
    /// If the bridge has no previous assessment, only the constitutional
    /// check applies.
    pub fn validate_modification(&self, candidate: &CoherenceState) -> bool {
        let mask = check_invariants_mask(
            candidate,
            ATTENTION_THRESHOLD,
            EPISODIC_MEMORY_THRESHOLD,
            self.entropy_threshold,
        );
        let phi = approximate_phi(
            mask,
            candidate.entropy,
            candidate.attention,
            candidate.episodic_memory_samples,
        );
        let constitutional = constitutional_ok(candidate, self.entropy_threshold);

        if !constitutional {
            return false;
        }

        match self.last_phi_milli {
            Some(previous) => Assessment {
                phi_milli: phi,
                mask,
                passed: true,
                constitutional,
            }
            .degradation_from(previous)
                <= MAX_PHI_DEGRADATION_MILLI,
            None => true,
        }
    }

    /// Confirm a state modification — records the snapshot and refreshes the
    /// last healthy state. Returns `false` if the modification would have been
    /// blocked by [`validate_modification`](Self::validate_modification).
    pub fn commit(&mut self, state: CoherenceState) -> bool {
        if self.validate_modification(&state) {
            self.assess(state);
            true
        } else {
            false
        }
    }

    /// Produce a compact textual summary of the last assessment.
    ///
    /// No allocation on `no_std`; writes into the caller-provided buffer.
    /// Returns the number of bytes written (or 0 when nothing was written).
    pub fn summary_into(&self, out: &mut [u8]) -> usize {
        let Some(phi) = self.last_phi_milli else {
            return 0;
        };
        let mask = match self.history.last() {
            Some(state) => check_invariants_mask(
                state,
                ATTENTION_THRESHOLD,
                EPISODIC_MEMORY_THRESHOLD,
                self.entropy_threshold,
            ),
            None => 0,
        };
        // Format: "PHI=NNN MASK=MMMM\n"
        let text = [
            b'P', b'H', b'I', b'=', (b'0' + (phi / 100) as u8), (b'0' + ((phi / 10) % 10) as u8),
            (b'0' + (phi % 10) as u8), b' ', b'M', b'A', b'S', b'K', b'=',
            b'0', b'x',
        ];
        let mut written = 0usize;
        for &b in &text {
            if written >= out.len() {
                break;
            }
            out[written] = b;
            written += 1;
        }
        if written + 3 <= out.len() {
            out[written] = b'0' + ((mask >> 4) & 0x0F);
            out[written + 1] = b'0' + (mask & 0x0F);
            out[written + 2] = b'\n';
            written += 3;
        }
        written
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phi_approx::GAP1_LOWER_BOUND_MILLI;

    fn healthy() -> CoherenceState {
        CoherenceState::healthy()
    }

    #[test]
    fn healthy_state_is_assessed_passing() {
        let mut bridge = MinimalConsciousnessBridge::new();
        let a = bridge.assess(healthy());
        assert!(a.passed);
        assert!(a.constitutional);
        assert_eq!(a.mask, 0b1111);
        assert!(a.phi_milli > GAP1_LOWER_BOUND_MILLI);
    }

    #[test]
    fn unknown_state_fails_and_clamps_phi() {
        let mut bridge = MinimalConsciousnessBridge::new();
        let a = bridge.assess(CoherenceState::unknown());
        assert!(!a.passed);
        assert!(!a.constitutional);
        assert_eq!(a.phi_milli, GAP1_LOWER_BOUND_MILLI);
    }

    #[test]
    fn history_is_bounded() {
        let mut bridge = MinimalConsciousnessBridge::new();
        for _ in 0..(HISTORY_CAPACITY + 10) {
            bridge.assess(healthy());
        }
        assert_eq!(bridge.history_len(), HISTORY_CAPACITY);
    }

    #[test]
    fn modification_above_degradation_limit_blocked() {
        let mut bridge = MinimalConsciousnessBridge::new();
        bridge.assess(healthy()); // phi ~ 999 region
        // A state that loses C-01, C-02, C-04 but keeps attention high
        let mut degraded = healthy();
        degraded.self_model = false;
        degraded.introspection = false;
        degraded.episodic_memory_samples = 0;
        assert!(!bridge.validate_modification(&degraded));
        assert!(!bridge.commit(degraded));
    }

    #[test]
    fn constitutional_violation_always_blocked() {
        let mut bridge = MinimalConsciousnessBridge::new();
        bridge.assess(healthy());
        let mut no_introspection = healthy();
        no_introspection.introspection = false;
        assert!(!bridge.validate_modification(&no_introspection));
        assert!(!bridge.commit(no_introspection));
    }

    #[test]
    fn mild_degradation_is_allowed() {
        let mut bridge = MinimalConsciousnessBridge::new();
        bridge.assess(healthy());
        // Drop attention slightly (still >= 60) and a bit of memory
        let mut mild = healthy();
        mild.attention = 62;
        mild.episodic_memory_samples = 6;
        assert!(bridge.validate_modification(&mild));
        assert!(bridge.commit(mild));
    }

    #[test]
    fn commit_refreshes_last_healthy() {
        let mut bridge = MinimalConsciousnessBridge::new();
        bridge.assess(healthy());
        assert!(bridge.last_healthy().is_some());

        let mut mild = healthy();
        mild.uptime_ticks = 42;
        assert!(bridge.commit(mild));
        assert_eq!(bridge.last_healthy().unwrap().uptime_ticks, 42);
    }

    #[test]
    fn no_prior_assessment_only_constitutional_check() {
        let bridge = MinimalConsciousnessBridge::new();
        // First modification: no previous Φ, but must stay constitutional.
        assert!(!bridge.validate_modification(&CoherenceState::unknown()));
        assert!(bridge.validate_modification(&CoherenceState::healthy()));
    }

    #[test]
    fn summary_is_fixed_width() {
        let mut bridge = MinimalConsciousnessBridge::new();
        bridge.assess(healthy());
        let mut buf = [0u8; 64];
        let n = bridge.summary_into(&mut buf);
        assert!(n >= 11); // "PHI=NNN MASK=0xMM\n"
        let text = core::str::from_utf8(&buf[..n]).unwrap();
        assert!(text.starts_with("PHI="));
    }

    #[test]
    fn custom_threshold_changes_acceptance() {
        // A threshold above the healthy Φ makes the healthy state "not passed".
        let mut bridge = MinimalConsciousnessBridge::with_phi_threshold(999);
        let a = bridge.assess(healthy());
        assert!(!a.passed);
    }

    #[test]
    fn low_entropy_blocks_at_c01() {
        let mut bridge = MinimalConsciousnessBridge::with_thresholds(600, 4096);
        let mut state = healthy();
        state.entropy = 2000; // below 4096 threshold
        let a = bridge.assess(state);
        assert_eq!(a.mask & crate::invariants::FirmwareInvariant::C01.bit(), 0);
        assert!(!a.constitutional);
    }
}