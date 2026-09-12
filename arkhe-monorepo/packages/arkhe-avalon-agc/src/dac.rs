//! Minimal DAC abstraction.
//!
//! Honest design correction: the `embedded-hal` `Dac` trait has *only*
//! [`Dac::write`]. There is no `read` method in the trait — the Phase-2
//! proposal's `self.dac.read()` does not exist upstream. Genuine read-back is
//! instead modelled by the optional [`ReadBack`](crate::ReadBack) trait so it
//! is reported truthfully (never mocked).

use crate::{AgcError, ReadBack};
use core::fmt;

/// A digital-to-analog gain element.
///
/// Only a write channel is assumed. Anything that claims more (read-back,
/// calibration) must additionally implement [`ReadBack`].
pub trait Dac {
    /// Underlying driver error.
    type Error: fmt::Debug;

    /// Apply `value` (raw DAC code) to the output.
    fn write(&mut self, value: u16) -> Result<(), Self::Error>;
}

/// Convert a driver error into an [`AgcError`].
pub fn map_dac_error<E: fmt::Debug>(_: E) -> AgcError {
    AgcError::DacError
}

/// A [`Dac`] trait-object-friendly grab bag used by host-side tests and the
/// PyO3 bridge: wraps a closure-based write with an optional read-back.
pub struct ClosureDac {
    last_value: Option<u16>,
    readback: Option<u16>,
    write_fn: Option<fn(u16) -> Result<(), AgcError>>,
}

impl ClosureDac {
    /// Build a write-only closure DAC with no read-back capability.
    pub fn write_only(on_write: fn(u16) -> Result<(), AgcError>) -> Self {
        Self { last_value: None, readback: None, write_fn: Some(on_write) }
    }

    /// Build a closure DAC with a read-back that always reports `value`.
    pub fn with_static_readback(value: u16) -> Self {
        Self { last_value: None, readback: Some(value), write_fn: None }
    }

    /// The last value passed to [`Dac::write`].
    pub fn last_written(&self) -> Option<u16> {
        self.last_value
    }
}

impl Dac for ClosureDac {
    type Error = AgcError;

    fn write(&mut self, value: u16) -> Result<(), Self::Error> {
        if let Some(f) = self.write_fn {
            f(value)?;
        }
        self.last_value = Some(value);
        Ok(())
    }
}

impl ReadBack for ClosureDac {
    type Error = AgcError;

    fn read_current_value(&mut self) -> Result<u16, Self::Error> {
        self.readback
            .ok_or(AgcError::ReadBackMismatch)
    }
}