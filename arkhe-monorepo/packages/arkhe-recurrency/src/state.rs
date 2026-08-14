//! Conscious-state daemon — the *cellular* level of recurrency.
//!
//! Cellular recurrency sustains the brain's global state. Here it is modelled
//! as a five-regime arousal daemon (`Coma` → `Hypervigilant`) that owns an
//! inference clock and two modulation gains:
//!
//! * **drive gain** — how much exogenous drive is admitted to the perceptual
//!   loop (damped in reduced-consciousness regimes);
//! * **recurrence multiplier** — how much recurrent processing the substrate
//!   is allowed (amplified in `Hypervigilant`, which is the hallucination-risk
//!   regime the Safe-Core must police).
//!
//! Reduced-consciousness regimes also suppress global access, see
//! [`crate::GlobalWorkspace::offer_in_state`].

/// Arousal regime of the substrate (cellular recurrency → conscious state).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum ArousalRegime {
    /// No recurrence, no access (vegetative).
    Coma = 0,
    /// Minimal recurrence, no access.
    DeepSleep = 1,
    /// Reduced recurrence, no access.
    Drowsy = 2,
    /// Full recurrence and access.
    Alert = 3,
    /// Amplified recurrence, access, elevated hallucination risk.
    Hypervigilant = 4,
}

impl ArousalRegime {
    /// Scale applied to exogenous drive entering the perceptual loop.
    pub fn drive_gain(self) -> f64 {
        match self {
            ArousalRegime::Coma => 0.0,
            ArousalRegime::DeepSleep => 0.1,
            ArousalRegime::Drowsy => 0.4,
            ArousalRegime::Alert => 1.0,
            ArousalRegime::Hypervigilant => 1.3,
        }
    }

    /// Multiplier applied to the perceptual loop's feedback gain.
    pub fn recurrence(self) -> f64 {
        match self {
            ArousalRegime::Coma => 0.0,
            ArousalRegime::DeepSleep => 0.2,
            ArousalRegime::Drowsy => 0.5,
            ArousalRegime::Alert => 1.0,
            ArousalRegime::Hypervigilant => 1.6,
        }
    }

    /// Clock divisor: `1` = full inference rate, `8` = near-frozen.
    pub fn clock_divisor(self) -> u64 {
        match self {
            ArousalRegime::Coma => 8,
            ArousalRegime::DeepSleep => 4,
            ArousalRegime::Drowsy => 2,
            ArousalRegime::Alert => 1,
            ArousalRegime::Hypervigilant => 1,
        }
    }

    /// Whether the regime allows content to be globally broadcast.
    pub fn allows_global_access(self) -> bool {
        matches!(self, ArousalRegime::Alert | ArousalRegime::Hypervigilant)
    }
}

/// Gated driver for the perceptual loop.
///
/// Encodes the *cellular recurrency → conscious state* mapping: the daemon
/// decides how much exogenous drive is admitted, how fast the global inference
/// clock ticks, and whether the global workspace may broadcast.
#[derive(Clone, Debug, serde::Serialize)]
pub struct StateDaemon {
    regime: ArousalRegime,
    clock: u64,
}

impl StateDaemon {
    /// Create a daemon in the given regime with a zeroed clock.
    pub fn new(regime: ArousalRegime) -> Self {
        Self { regime, clock: 0 }
    }

    /// Current arousal regime.
    pub fn regime(&self) -> ArousalRegime {
        self.regime
    }

    /// Switch regime, returning the previous one.
    pub fn set_regime(&mut self, regime: ArousalRegime) -> ArousalRegime {
        let previous = self.regime;
        self.regime = regime;
        previous
    }

    /// Scale applied to exogenous drive entering the loop.
    pub fn drive_gain(&self) -> f64 {
        self.regime.drive_gain()
    }

    /// Multiplier applied to the perceptual loop's feedback gain.
    pub fn recurrence(&self) -> f64 {
        self.regime.recurrence()
    }

    /// Advance the inference clock and return the effective tick.
    ///
    /// Reduced-consciousness regimes tick at a fraction of the full rate
    /// (`Alert`/`Hypervigilant` tick every call; `Drowsy` every two, etc.).
    pub fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock.div_ceil(self.regime.clock_divisor())
    }

    /// Scale an exogenous drive value by the regime's drive gain.
    pub fn gate(&self, exogenous: f64) -> f64 {
        exogenous * self.regime.drive_gain()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_clock_advances_by_one_per_tick() {
        let mut d = StateDaemon::new(ArousalRegime::Alert);
        assert_eq!(d.tick(), 1);
        assert_eq!(d.tick(), 2);
        assert_eq!(d.tick(), 3);
    }

    #[test]
    fn drowsy_clock_advances_at_half_rate() {
        let mut d = StateDaemon::new(ArousalRegime::Drowsy);
        assert_eq!(d.tick(), 1);
        assert_eq!(d.tick(), 1);
        assert_eq!(d.tick(), 2);
        assert_eq!(d.tick(), 2);
    }

    #[test]
    fn drowsy_damps_exogenous_drive() {
        let alert = StateDaemon::new(ArousalRegime::Alert);
        let drowsy = StateDaemon::new(ArousalRegime::Drowsy);
        assert!((alert.gate(1.0) - 1.0).abs() < 1e-12);
        assert!((drowsy.gate(1.0) - 0.4).abs() < 1e-12);
    }

    #[test]
    fn switching_regime_changes_gain_and_returns_previous() {
        let mut d = StateDaemon::new(ArousalRegime::Alert);
        assert!((d.gate(2.0) - 2.0).abs() < 1e-12);
        assert_eq!(d.set_regime(ArousalRegime::DeepSleep), ArousalRegime::Alert);
        assert!((d.gate(2.0) - 0.2).abs() < 1e-12);
    }

    #[test]
    fn reduced_regimes_suppress_global_access() {
        for r in [
            ArousalRegime::Coma,
            ArousalRegime::DeepSleep,
            ArousalRegime::Drowsy,
        ] {
            assert!(!r.allows_global_access(), "{r:?} must not broadcast");
        }
        for r in [ArousalRegime::Alert, ArousalRegime::Hypervigilant] {
            assert!(r.allows_global_access(), "{r:?} must broadcast");
        }
    }

    #[test]
    fn hypervigilant_amplifies_recurrence() {
        let alert = StateDaemon::new(ArousalRegime::Alert);
        let hvg = StateDaemon::new(ArousalRegime::Hypervigilant);
        assert!(hvg.recurrence() > alert.recurrence());
    }
}
