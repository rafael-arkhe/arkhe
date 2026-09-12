//! `Untrusted<T>` guard for values crossing the Python → Rust boundary.
//!
//! Per the 529-RUST-VALIDATE-KERNEL-API conventions: values received from an
//! *external* channel (here, floats computed by the Python ensemble) are
//! wrapped in [`Untrusted`] until an explicit validation step produces a
//! trusted value. `Untrusted<T>` is inexpensive — it is a single `T` —
//! and *cannot* be deref'd into a trusted use without calling a validator
//! that returns a dedicated out type.

/// Marker: encapsulates a value that must not be trusted before validation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Untrusted<T> {
    value: T,
}

impl<T> Untrusted<T> {
    /// Wrap an external/untrusted value.
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self { value }
    }

    /// Access the raw value (still untrusted).
    ///
    /// This is provided so validators can inspect the payload, but its use is
    /// a *loud* signal: anything derived from it must pass through a
    /// trusted validator before being used as evidence.
    #[inline]
    pub const fn raw(&self) -> &T {
        &self.value
    }
}

impl Untrusted<f64> {
    /// Try to interpret as a finite `f64` (validation for floats).
    ///
    /// Returns `None` for `NaN` or infinite — the classic attack on numerical
    /// attestation is forcing an invalid float through. Callers must handle
    /// the `None` path explicitly.
    #[must_use]
    pub fn to_finite(self) -> Option<f64> {
        let v = *self.raw();
        if v.is_finite() {
            Some(v)
        } else {
            None
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn finite_float_validates() {
        let u = Untrusted::new(0.429);
        assert_eq!(u.to_finite(), Some(0.429));
    }

    #[test]
    fn nan_and_inf_rejected() {
        assert_eq!(Untrusted::new(f64::NAN).to_finite(), None);
        assert_eq!(Untrusted::new(f64::INFINITY).to_finite(), None);
    }

    #[test]
    fn raw_is_available_to_validators() {
        let u = Untrusted::new(1.25f64);
        assert_eq!(*u.raw(), 1.25);
    }
}