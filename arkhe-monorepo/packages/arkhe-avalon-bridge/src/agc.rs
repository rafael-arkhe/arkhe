//! PyO3 wrappers for the AGC controller.
//!
//! The injected slew clock uses the *real* host monotonic clock
//! (`std::time::Instant` as ms). No unsafe, no fabricated time.

use arkhe_avalon_agc::{dac::ClosureDac, AgcConfig, AgcController, GainEvidence};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// Host-side monotonic clock (ms).
///
/// `Instant` is monotonic (never goes backwards); timestamp semantics live in
/// the evidence record, not here.
#[inline]
fn now_ms() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(std::time::Instant::now);
    start.elapsed().as_millis() as u64
}

/// AGC controller exposed to Python as `AgcController`.
#[pyclass(name = "AgcController")]
pub struct PyAgcController {
    inner: AgcController<ClosureDac, fn() -> u64>,
}

#[pymethods]
impl PyAgcController {
    /// Create an AGC controller (host bridge — DAC is a no-op write sink;
    /// real hardware wiring lives in the SDR driver layer).
    ///
    /// * `dac_vref_mv`: DAC reference voltage, mV (default 3300)
    /// * `dac_bits`: DAC resolution, bits (default 12, max 16)
    /// * `min_mv` / `max_mv`: output range in mV
    /// * `slew_mv_per_ms`: maximum slew rate, mV/ms
    #[new]
    #[pyo3(signature = (dac_vref_mv=3300, dac_bits=12, min_mv=0, max_mv=1200, slew_mv_per_ms=10))]
    fn new(
        dac_vref_mv: u16,
        dac_bits: u8,
        min_mv: u16,
        max_mv: u16,
        slew_mv_per_ms: u16,
    ) -> PyResult<Self> {
        if dac_vref_mv == 0 {
            return Err(PyValueError::new_err("dac_vref_mv must be positive"));
        }
        if dac_bits == 0 || dac_bits > 16 {
            return Err(PyValueError::new_err("dac_bits must be in 1..=16"));
        }
        let config = AgcConfig {
            min_mv,
            max_mv,
            dac_vref_mv,
            dac_bits,
            slew_mv_per_ms,
            default_now_ms: now_ms(),
        };
        let inner = AgcController::new(config, ClosureDac::write_only(|_| Ok(())), now_ms as fn() -> u64);
        Ok(Self { inner })
    }

    /// Apply a target gain (mV), slew-limited. Returns the attestable
    /// [`PyGainEvidence`] record (fields read-only, bound-checked).
    ///
    /// pyo3 ≥ 0.26 requires pytests to run under GIL-attached contexts; this
    /// method runs with the GIL already held and therefore returns the typed
    /// evidence class instead of constructing a dict manually.
    fn set_gain(&mut self, target_mv: u16) -> PyResult<PyGainEvidence> {
        let ev = self
            .inner
            .set_gain(target_mv)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(ev.into())
    }

    /// Current applied gain, mV.
    fn current_gain(&self) -> Option<u16> {
        self.inner.current_gain()
    }
}

/// Evidence record exposed read-only to Python.
#[pyclass]
pub struct PyGainEvidence {
    #[pyo3(get)]
    pub requested_mv: u16,
    #[pyo3(get)]
    pub applied_mv: u16,
    #[pyo3(get)]
    pub dac_value: u16,
    #[pyo3(get)]
    pub slew_limited: bool,
    #[pyo3(get)]
    pub read_back_ok: Option<bool>,
    #[pyo3(get)]
    pub timestamp_ms: u64,
}

impl From<GainEvidence> for PyGainEvidence {
    fn from(ev: GainEvidence) -> Self {
        Self {
            requested_mv: ev.requested_mv,
            applied_mv: ev.applied_mv,
            dac_value: ev.dac_value,
            slew_limited: ev.slew_limited,
            read_back_ok: ev.read_back_ok,
            timestamp_ms: ev.timestamp_ms,
        }
    }
}

/// Register python classes in the module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAgcController>()?;
    m.add_class::<PyGainEvidence>()?;
    Ok(())
}