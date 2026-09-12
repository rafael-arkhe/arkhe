//! Slew-rate limiter.
//!
//! Correção honesta vs. o rascunho da Fase 2: o código proposto usava um
//! relógio fake (`now_ms = 0` fixo). Isso quebrava o próprio teste (na 2ª
//! chamada `elapsed = 0` => nenhum passo de tempo decorrido => slew sempre
//! "limitado", devolvendo 600, não 1200). Aqui o relógio é **injetado** pelo
//! chamador via [`SlewRateLimiter::limit`] — modelo honesto e testável.

use core::fmt;

/// Error when the caller requests a change faster than the slew rate and
/// refuses clamping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlewError {
    /// Current applied voltage, mV.
    pub current_mv: u16,
    /// Desired target voltage, mV.
    pub target_mv: u16,
    /// Maximum allowed change over the elapsed time, mV.
    pub max_delta_mv: u16,
}

impl fmt::Display for SlewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "slew rate exceeded: {}+/-{} max/sample vs requested delta {}",
            self.current_mv,
            self.max_delta_mv,
            self.target_mv.abs_diff(self.current_mv)
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SlewError {}

/// Stateful ramp limiter. Time is always supplied by the caller (`now_ms`).
#[derive(Debug, Clone)]
pub struct SlewRateLimiter {
    max_mv_per_ms: u16,
    last_mv: Option<u16>,
    last_ms: Option<u64>,
}

impl SlewRateLimiter {
    /// Create a limiter with the given rate (mV per ms).
    pub const fn new(max_mv_per_ms: u16) -> Self {
        Self { max_mv_per_ms, last_mv: None, last_ms: None }
    }

    /// Slew limit without a delay primitive: returns the clamped value.
    ///
    /// `now_ms` is the caller-provided monotonic millisecond clock.
    /// This is deliberately a *pure* function of the state and `now_ms` —
    /// the caller owns the clock (real hardware time, host monotonic clock,
    /// or a deterministic test clock).
    pub fn limit(&mut self, target_mv: u16, now_ms: u64) -> u16 {
        let (last_mv, last_ms) = match (self.last_mv, self.last_ms) {
            (Some(mv), Some(ms)) => (mv, ms),
            _ => {
                // First sample: accept as-is (nothing to slew from).
                self.last_mv = Some(target_mv);
                self.last_ms = Some(now_ms);
                return target_mv;
            }
        };

        let elapsed = now_ms.saturating_sub(last_ms);
        let max_delta = self.max_mv_per_ms.saturating_mul(elapsed.min(u16::MAX as u64) as u16);

        let delta = target_mv.abs_diff(last_mv);

        let applied = if delta > max_delta {
            // Clamp toward the target.
            if target_mv > last_mv {
                last_mv + max_delta
            } else {
                last_mv - max_delta
            }
        } else {
            target_mv
        };

        self.last_mv = Some(applied);
        self.last_ms = Some(now_ms);
        applied
    }

    /// Strict variant: err if the requested delta exceeds the slew budget.
    pub fn limit_strict(&mut self, target_mv: u16, now_ms: u64) -> Result<u16, SlewError> {
        let current = self.last_mv.unwrap_or(target_mv);
        let elapsed = now_ms.saturating_sub(self.last_ms.unwrap_or(now_ms));
        let max_delta = self.max_mv_per_ms.saturating_mul(elapsed.min(u16::MAX as u64) as u16);
        if target_mv.abs_diff(current) > max_delta {
            return Err(SlewError { current_mv: current, target_mv, max_delta_mv: max_delta });
        }
        Ok(self.limit(target_mv, now_ms))
    }

    /// Current applied value, if any.
    pub fn current(&self) -> Option<u16> {
        self.last_mv
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn first_sample_passes_through() {
        let mut s = SlewRateLimiter::new(10);
        assert_eq!(s.limit(600, 0), 600);
    }

    #[test]
    fn ramp_limits_step_by_max_delta_per_ms() {
        let mut s = SlewRateLimiter::new(10);
        assert_eq!(s.limit(600, 0), 600);
        // t=100ms later, budget=1000mV: 600->1200 allowed fully.
        assert_eq!(s.limit(1200, 100), 1200);
        // t=200ms later, budget=1000mV, current=1200: no overshoot.
        assert_eq!(s.limit(1200, 200), 1200);
    }

    #[test]
    fn clamps_when_time_has_not_advanced() {
        let mut s = SlewRateLimiter::new(10);
        assert_eq!(s.limit(600, 0), 600);
        // Same instant, target 1200, budget=0 -> clamp to 600.
        assert_eq!(s.limit(1200, 0), 600);
        // After 10ms budget=100 -> 700.
        assert_eq!(s.limit(1200, 10), 700);
    }

    #[test]
    fn strict_variant_rejects_too_fast() {
        let mut s = SlewRateLimiter::new(10);
        s.limit(600, 0);
        assert!(s.limit_strict(1200, 0).is_err());
        assert!(s.limit_strict(1200, 100).is_ok());
    }
}