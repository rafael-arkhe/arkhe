//! Graphene metamaterial THz sensor — surrogate model of
//! *Amraoui et al.*, Materials Today Nano (2026),
//! DOI 10.1016/j.mtnano.2026.100911.
//!
//! The article reports a dual-band tunable absorber:
//!
//! | Parameter | Band 1 | Band 2 |
//! |-----------|--------|--------|
//! | Resonance | 2.9215 THz | 9.0199 THz |
//! | Q-factor  | 2.6 | 51.9 |
//! | Peak absorption | 0.9982 | 0.9988 |
//! | Sensitivity | — | 495 GHz/RIU (1 µm, RI 1.32–1.40) |
//!
//! **Honesty note.** This crate implements the *analytical* model of the
//! article (a Lorentzian per band on top of the graphene Drude response), as
//! a deterministic surrogate for the CST/Random-Forest pipeline of the paper.
//! Computing a Lorentzian costs nanoseconds in Rust, so no machine learning is
//! needed to *simulate* a spectrum; ML remains the right tool for the inverse
//! problem (estimating RI/thickness from a measured absorption), which this
//! crate deliberately does not fake.
//!
//! Integration with [`arkhe_recurrency`] is explicit: a measured spectrum is
//! reduced to an 8-dimensional feature vector and fed through the real
//! [`arkhe_recurrency::RecurrencyEngine`], which emits
//! [`arkhe_recurrency::RecurrencyTicket`]s. The anomaly signal itself lives in
//! [`ThzMeasurement`].

pub mod analyte;
pub mod error;
pub mod geometry;
pub mod sensor;
pub mod spectrum;

pub use analyte::Analyte;
pub use error::ThzSensorError;
pub use geometry::UnitCellGeometry;
pub use sensor::{ThzDetectResult, ThzMeasurement, ThzMetamaterialSensor};
pub use spectrum::AbsorptionSpectrum;

/// Number of spectral features reduced from the measured spectrum. Must match
/// the `n` of the `PerceptionLoop` the sensor is driven through.
pub const FEATURE_DIM: usize = 8;
