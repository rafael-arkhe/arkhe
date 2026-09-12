//! PyO3 wrappers for Safe-Core coherence evidence.

use arkhe_avalon_agc::untrusted::Untrusted;
use arkhe_avalon_agc::CoherenceEvidence;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// Create a hashed, attestable [`CoherenceEvidence`] from Python floats.
///
/// Float fields are validated for finiteness and gated by
/// [`CoherenceEvidence::new_unvalidated`] (DFT verification, node count,
/// coherence range). Returns the SHA3-256 digest bytes.
#[pyfunction]
#[pyo3(signature = (eps_tut, eps_obs, bound_satisfied, coherence_r, n_nodes, timestamp_ms, dft_verified))]
pub fn make_coherence_evidence(
    eps_tut: f64,
    eps_obs: f64,
    bound_satisfied: bool,
    coherence_r: f64,
    n_nodes: u32,
    timestamp_ms: u64,
    dft_verified: bool,
) -> PyResult<Vec<u8>> {
    let evidence = CoherenceEvidence::new_unvalidated(
        Untrusted::new(eps_tut),
        Untrusted::new(eps_obs),
        bound_satisfied,
        Untrusted::new(coherence_r),
        n_nodes,
        timestamp_ms,
        dft_verified,
    )
    .map_err(|e| PyValueError::new_err(format!("CoherenceEvidence rejected: {e:?}")))?;

    Ok(evidence.hash().to_vec())
}

/// Verify that a previously-hashed digest still matches a re-computation.
///
/// This is a *pure* local check; Safe-Core attests the digest, not the floats
/// — the floats themselves never leave the controlled process boundary.
#[pyfunction]
#[pyo3(signature = (eps_tut, eps_obs, bound_satisfied, coherence_r, n_nodes, timestamp_ms, dft_verified, digest_bytes))]
#[allow(clippy::too_many_arguments)]
pub fn verify_coherence_digest(
    eps_tut: f64,
    eps_obs: f64,
    bound_satisfied: bool,
    coherence_r: f64,
    n_nodes: u32,
    timestamp_ms: u64,
    dft_verified: bool,
    digest_bytes: Vec<u8>,
) -> PyResult<bool> {
    let evidence = CoherenceEvidence::new_unvalidated(
        Untrusted::new(eps_tut),
        Untrusted::new(eps_obs),
        bound_satisfied,
        Untrusted::new(coherence_r),
        n_nodes,
        timestamp_ms,
        dft_verified,
    )
    .map_err(|e| PyValueError::new_err(format!("CoherenceEvidence rejected: {e:?}")))?;

    if digest_bytes.len() != 32 {
        return Err(PyValueError::new_err("digest must be 32 bytes (SHA3-256)"));
    }
    let mut expected = [0u8; 32];
    expected.copy_from_slice(&digest_bytes);
    Ok(evidence.hash() == expected)
}

/// Register evidence functions in the module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(make_coherence_evidence, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(verify_coherence_digest, m)?)?;
    Ok(())
}