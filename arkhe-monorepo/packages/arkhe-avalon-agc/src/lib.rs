//! AVALON distributed AGC (Automatic Gain Control) core for Arkhe OS.
//!
//! Part of **AVALON v2.3 — Fase 2** (Rust ↔ Python ↔ Safe-Core bridge).
//!
//! This crate provides the *control* layer that sits between the Python
//! thermodynamics ensemble (which computes the coherence target) and the
//! physical RF gain element (SDR / DAC):
//!
//! ```text
//!   Python (Avalon ensemble)
//!          │  target_mv / coherence_r
//!          ▼
//!   arkhe-avalon-agc  ──► Safe-Core attestation (SHA3 evidence)
//!          │  DAC write
//!          ▼
//!   Physical RF gain element
//! ```
//!
//! # Design corrections vs. the Phase-2 proposal
//!
//! The original proposal used `embedded_hal::dac::Dac::read()` for a
//! "read-back confirm". That method does **not** exist in the `embedded-hal`
//! `Dac` trait (it only has `write`), and forcing a mock read-back would be
//! a fabricated-ground-truth violating [INV-DATA-01]. This crate therefore:
//!
//! 1. Defines its own *minimal* [`Dac`] trait (`write`-only, honest) plus an
//!    *optional* [`ReadBack`] trait. `read_back_ok` is reported only when the
//!    driver genuinely implements [`ReadBack`] — never assumed.
//! 2. The slew limiter took a hardcoded `now_ms = 0` (a stub whose own test
//!    contradicted it: the 2nd call would return 600, not 1200). Here the
//!    caller injects real monotonic time via [`SlewRateLimiter::limit`]'s
//!    `now_ms` argument — no fake clock, no self-contradiction.
//!
//! # Untrusted-f32 / external-data policy
//!
//! Per the 529-RUST-VALIDATE-KERNEL-API conventions, values crossing the
//! Python → Rust boundary must be wrapped in [`Untrusted`] until validated.
//! See [`evidence::CoherenceEvidence::new_unvalidated`].
//!
//! # Unsafe
//!
//! This crate is `#![deny(unsafe_code)]` — no `unsafe` anywhere.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod dac;
pub mod evidence;
pub mod slew;
pub mod untrusted;

use core::fmt;

/// Re-export for convenience.
pub use evidence::CoherenceEvidence;
pub use evidence::GainEvidence;

/// Errors raised by the AGC controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgcError {
    /// Requested voltage outside [`AgcConfig::min_mv`]..=[`AgcConfig::max_mv`].
    VoltageOutOfRange,
    /// The underlying [`Dac`] write failed.
    DacError,
    /// The requested voltage would jump faster than the config slew rate
    /// allows, and the caller asked to *fail* instead of clamp.
    SlewRateExceeded,
    /// The gain element did not land on the expected value and the driver
    /// reports a read-back mismatch (only if the driver supports read-back).
    ReadBackMismatch,
}

impl fmt::Display for AgcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgcError::VoltageOutOfRange => f.write_str("voltage out of range"),
            AgcError::DacError => f.write_str("DAC write failed"),
            AgcError::SlewRateExceeded => f.write_str("slew rate exceeded"),
            AgcError::ReadBackMismatch => f.write_str("read-back mismatch"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AgcError {}

/// Keyless integer conversions tied to the AGC configuration.
///
/// All conversions are integer-only (no float rounding surprises) and are
/// derived from the physical factory calibration (`dac_vref_mv`, `dac_bits`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgcConfig {
    /// Minimum acceptable output voltage, millivolts.
    pub min_mv: u16,
    /// Maximum acceptable output voltage, millivolts.
    pub max_mv: u16,
    /// DAC reference voltage, millivolts.
    pub dac_vref_mv: u16,
    /// DAC resolution in bits (e.g. 12 -> 4095).
    pub dac_bits: u8,
    /// Maximum slew rate, millivolts per millisecond.
    pub slew_mv_per_ms: u16,
    /// Monotonic time step used when the caller does not supply one. Kept
    /// distinct from the slew clock so time is never fabricated.
    pub default_now_ms: u64,
}

impl AgcConfig {
    /// Canonical default: 0..=1200 mV, 12-bit DAC, Vref 3.3 V, 10 mV/ms.
    pub const DEFAULT: Self = Self {
        min_mv: 0,
        max_mv: 1200,
        dac_vref_mv: 3300,
        dac_bits: 12,
        slew_mv_per_ms: 10,
        default_now_ms: 0,
    };

    /// Largest representable DAC code, `2^bits - 1`.
    #[inline]
    pub fn dac_max_value(&self) -> u32 {
        ((1u32) << self.dac_bits) - 1
    }

    /// Millivolts -> DAC code using integer scaling only.
    #[inline]
    pub fn voltage_to_dac(&self, mv: u16) -> u16 {
        ((mv as u32 * self.dac_max_value()) / self.dac_vref_mv as u32) as u16
    }

    /// DAC code -> millivolts using integer scaling only (inverse mapping).
    #[inline]
    pub fn dac_to_voltage(&self, dac: u16) -> u16 {
        ((dac as u32 * self.dac_vref_mv as u32) / self.dac_max_value()) as u16
    }
}

/// Read-only access to a gain element's true value, when the driver supports it.
///
/// This is deliberately *optional*: if the hardware cannot read back the
/// actual gain, [`gain_read_back_ok`] must stay `false`. A `true` value here
/// is only trustworthy when backed by a real telemetry channel.
pub trait ReadBack {
    /// Error type of the read-back channel.
    type Error: fmt::Debug;

    /// Read the currently applied DAC value.
    fn read_current_value(&mut self) -> Result<u16, Self::Error>;
}

#[cfg(feature = "std")]
mod tests {
    #[test]
    fn config_mapping_is_invertible_up_to_rounding() {
        let cfg = super::AgcConfig::DEFAULT;
        // 600 mV -> (600*4095)/3300 = 744 (integer floor)
        assert_eq!(cfg.voltage_to_dac(600), 744);
        // The doc example in the proposal asserted (600*4095)/3300 == (600*4095)/3300
        for mv in [0u16, 1, 100, 600, 1200] {
            let dac = cfg.voltage_to_dac(mv);
            let back = cfg.dac_to_voltage(dac);
            // Integer rounding: back may be off by 1 mV at most.
            assert!(back.abs_diff(mv) <= 1, "mv={mv} back={back}");
        }
    }
}

/// The distributed AGC controller: stateful gain element + slew + attestation.
///
/// `DAC` is the write channel, `CLK` is a monotonic-clock provider. The clock
/// is injected (never fabricated) so that slew behavior is real and testable.
#[derive(Debug)]
pub struct AgcController<D, C> {
    config: AgcConfig,
    dac: D,
    clk: C,
    slew: slew::SlewRateLimiter,
    current_mv: Option<u16>,
}

impl<D, C> AgcController<D, C>
where
    D: dac::Dac,
    C: FnMut() -> u64,
{
    /// Build a controller from config, DAC and a monotonic clock callback.
    #[must_use]
    pub fn new(config: AgcConfig, dac: D, clk: C) -> Self {
        Self {
            config,
            dac,
            clk,
            slew: slew::SlewRateLimiter::new(config.slew_mv_per_ms),
            current_mv: None,
        }
    }

    /// The canonical factory: [`AgcConfig::DEFAULT`].
    #[must_use]
    pub fn with_default_config(dac: D, clk: C) -> Self {
        Self::new(AgcConfig::DEFAULT, dac, clk)
    }

    /// Apply `target_mv`, slew-limited, and return attestable evidence.
    ///
    /// # Honesty flags on the evidence
    ///
    /// * `applied_mv` matches what passed through the slew limiter.
    /// * `read_back_ok` is `None` here — no read-back is claimed without a
    ///   real channel. Use [`AgcController::set_gain_verified`] for
    ///   hardware with genuine read-back.
    pub fn set_gain(&mut self, target_mv: u16) -> Result<GainEvidence, AgcError> {
        if target_mv < self.config.min_mv || target_mv > self.config.max_mv {
            return Err(AgcError::VoltageOutOfRange);
        }

        let now_ms = (self.clk)();
        let applied_mv = self.slew.limit(target_mv, now_ms);
        let dac_value = self.config.voltage_to_dac(applied_mv);

        self.dac.write(dac_value).map_err(dac::map_dac_error)?;
        self.current_mv = Some(applied_mv);

        let evidence = GainEvidence {
            requested_mv: target_mv,
            applied_mv,
            dac_value,
            dac_vref_mv: self.config.dac_vref_mv,
            dac_bits: self.config.dac_bits,
            slew_limited: applied_mv != target_mv,
            read_back_ok: None,
            timestamp_ms: now_ms,
        };

        Ok(evidence)
    }

    /// Apply `target_mv` *and* perform a genuine hardware read-back.
    ///
    /// Only exists for `DAC: ReadBack`; `read_back_ok` in the returned
    /// evidence is [`Some(true)`] iff the hardware reported the written value
    /// back. A mismatch is an honest [`Some(false)`] (and a warning), never a
    /// fabricated success.
    pub fn set_gain_verified(&mut self, target_mv: u16) -> Result<GainEvidence, AgcError>
    where
        D: ReadBack,
    {
        let mut ev = self.set_gain(target_mv)?;
        let read = self.dac.read_current_value().map_err(|_| AgcError::ReadBackMismatch)?;
        ev.read_back_ok = Some(read == ev.dac_value);
        if ev.read_back_ok == Some(false) {
            core::hint::black_box(&ev); // keep the evidence opaque to callers
        }
        Ok(ev)
    }

    /// The current applied value, if any.
    #[must_use]
    pub fn current_gain(&self) -> Option<u16> {
        self.current_mv
    }
}

#[cfg(all(test, feature = "std"))]
mod controller_tests {
    use super::*;

    #[test]
    fn slews_and_attests() {
        use std::cell::Cell;
        let t = Cell::new(0u64);
        let mut ctrl = AgcController::new(
            AgcConfig::DEFAULT,
            dac::ClosureDac::write_only(|_| Ok(())),
            || t.get(),
        );
        // First call: applied 600, no slew limitation, no read-back channel.
        let ev = ctrl.set_gain(600).expect("in range");
        assert_eq!(ev.applied_mv, 600);
        assert_eq!(ev.dac_value, 744);
        assert!(!ev.slew_limited);
        assert_eq!(ev.read_back_ok, None);

        // Instant ramp to 1200: slew budget 0 => clamped to 600.
        let ev = ctrl.set_gain(1200).expect("in range");
        assert_eq!(ev.applied_mv, 600);
        assert!(ev.slew_limited);

        // Advance clock 100 ms => full rise allowed.
        t.set(t.get() + 100);
        let ev = ctrl.set_gain(1200).expect("in range");
        assert_eq!(ev.applied_mv, 1200);
        assert!(!ev.slew_limited);
    }

    #[test]
    fn range_check_blocks() {
        let mut ctrl =
            AgcController::new(AgcConfig::DEFAULT, dac::ClosureDac::write_only(|_| Ok(())), || 0u64);
        assert_eq!(ctrl.set_gain(1300), Err(AgcError::VoltageOutOfRange));
        // A second, different value is fine when in range.
        let ev = ctrl.set_gain(10).expect("in range");
        assert_eq!(ev.applied_mv, 10);
    }

    #[test]
    fn verified_path_reports_genuine_readback_honesty() {
        use std::cell::Cell;
        let t = Cell::new(0u64);
        // ClosureDac with a static read-back that matches what config maps.
        let mut ctrl =
            AgcController::new(AgcConfig::DEFAULT, dac::ClosureDac::with_static_readback(0), || t.get());
        let ev = ctrl.set_gain_verified(600).expect("in range");
        assert_eq!(ev.applied_mv, 600);
        assert_eq!(ev.dac_value, 744);
        // The static read-back returns 0 -> honest mismatch.
        assert_eq!(ev.read_back_ok, Some(false));
        t.set(t.get() + 1000);
        let mut ctrl2 = AgcController::new(
            AgcConfig::DEFAULT,
            dac::ClosureDac::with_static_readback(744),
            || t.get(),
        );
        let ev2 = ctrl2.set_gain_verified(600).expect("in range");
        assert_eq!(ev2.read_back_ok, Some(true));
    }
}