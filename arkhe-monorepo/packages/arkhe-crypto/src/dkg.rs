//! Feldman VSS failure detection.
//!
//! # Conventions
//!
//! This module uses the *reconstruction threshold* convention:
//! `t` is the minimum number of honest shares required to reconstruct
//! the secret. A DKG is failed when either:
//!
//!   (a) `disq > t`             — too many disqualified participants;
//!   (b) `n - disq < t`         — too few honest participants remain.
//!
//! Note the strict `<` on (b): the boundary case `n - disq == t` is
//! **acceptable** (the secret is still reconstructible from exactly
//! the honest set). This is the convention used by `arkhe-vss`.
//!
//! # Alternative convention
//!
//! DKG specifications in the GJKR / Flow lineage (see `FlowDKG.cdc`,
//! `getNativeSuccessThreshold()`) treat `t` as the *degree of the
//! sharing polynomial* and require **more than** `t` honest
//! participants: `n - disq > t`. Under that reading, failure is
//! `n - disq <= t`, and the operator here would be `<=`.
//!
//! Both are correct within their respective conventions. This module
//! commits to the reconstruction-threshold convention; callers that
//! operate under the GJKR convention should map their `t` accordingly
//! (i.e. pass `t + 1`).

/// Returns `true` if the DKG must be aborted.
///
/// # Arguments
/// * `disq` — number of disqualified participants
/// * `n`    — total number of participants
/// * `t`    — reconstruction threshold (honest shares required)
pub fn is_dkg_failed(disq: u32, n: u32, t: u32) -> bool {
    disq > t || n.saturating_sub(disq) < t
}