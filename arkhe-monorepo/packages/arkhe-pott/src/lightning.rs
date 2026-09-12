//! Latency-aware Lightning policy and interplanetary link budgets.
//!
//! The extra CLTV margin forced by interplanetary light time and jitter is
//! (paper §6, Eq. 1):
//!
//! ```text
//! ∆extra_CLTV = ⌈(RTT + J) / btarget⌉ blocks,   btarget = 10 min (Bitcoin)
//! ```
//!
//! Operators then add their base policy (`Bbase`, e.g. 144 blocks) plus an
//! operational margin (`Mop`). Relative locktime (CSV) can be expressed in
//! time via BIP-68's 512-second granularity.

/// Bitcoin target L1 block interval (minutes).
pub const BITCOIN_BLOCK_MINUTES: u64 = 10;

/// Headers per year at a 10-minute cadence (~52,560).
pub const BLOCKS_PER_YEAR: u64 = 52_560;
/// Serialized header size (bytes).
pub const HEADER_BYTES: u64 = 80;

/// Round-trip light time for a one-way light time (minutes).
#[must_use]
pub fn rtt_minutes(owlt_minutes: u64) -> u64 {
    2 * owlt_minutes
}

/// `∆extra_CLTV = ⌈(RTT + J) / btarget⌉` blocks.
///
/// The function is a step function of `RTT + J` (paper, Fig. 3): it increases
/// by one block only when `RTT + J` crosses a multiple of `btarget`.
#[must_use]
pub fn cltv_extra_blocks(rtt_minutes: u64, jitter_minutes: u64) -> u64 {
    rtt_minutes.saturating_add(jitter_minutes).div_ceil(BITCOIN_BLOCK_MINUTES)
}

/// Recommended total CLTV: `Bbase + ∆extra_CLTV + Mop`.
#[must_use]
pub fn recommended_cltv(base_blocks: u64, rtt_minutes: u64, jitter_minutes: u64, op_margin: u64) -> u64 {
    base_blocks + cltv_extra_blocks(rtt_minutes, jitter_minutes) + op_margin
}

/// Converts a relative-locktime duration in seconds to BIP-68 time-based CSV
/// sequence units (512-second granularity).
#[must_use]
pub fn csv_time_units(seconds: u64) -> u64 {
    seconds.div_ceil(512)
}

/// Annual header replication volume (bytes/year): `80 B × 52,560 ≈ 4.2 MB` (paper §6.1).
#[must_use]
pub fn header_bytes_per_year() -> u64 {
    HEADER_BYTES * BLOCKS_PER_YEAR
}

/// Sustained header throughput (bits per second): ≈ 1.07 bps.
#[must_use]
pub fn header_bps() -> f64 {
    header_bytes_per_year() as f64 * 8.0 / (365.25 * 86_400.0)
}

/// Annual compact-filter volume for a median filter size (bytes): ≈ 1.05 GB/yr
/// at 20 kB/block.
#[must_use]
pub fn filter_bytes_per_year(median_filter_bytes: u64) -> u64 {
    median_filter_bytes * BLOCKS_PER_YEAR
}

/// Sustained compact-filter throughput (bits per second): ≈ 267 bps at 20 kB.
#[must_use]
pub fn filter_bps(median_filter_bytes: u64) -> f64 {
    filter_bytes_per_year(median_filter_bytes) as f64 * 8.0 / (365.25 * 86_400.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worked_example_owlt_22_j_60() {
        // Paper §6: OWLT 22 min → RTT 44, J 60 → 11 extra blocks.
        assert_eq!(rtt_minutes(22), 44);
        assert_eq!(cltv_extra_blocks(44, 60), 11);
        // Base 144 + op 2 → total 157.
        assert_eq!(recommended_cltv(144, 44, 60, 2), 157);
    }

    #[test]
    fn step_function_stays_flat_below_multiple() {
        // ⌈(RTT+J)/10⌉ is flat within a 10-minute band: 1 min → 1 block,
        // and the band boundary sits exactly at a multiple of btarget.
        assert_eq!(cltv_extra_blocks(9, 0), 1);
        assert_eq!(cltv_extra_blocks(10, 0), 1);
        assert_eq!(cltv_extra_blocks(11, 0), 2);
        // At the exact threshold RTT+J = 60 → 6 (not 5).
        assert_eq!(cltv_extra_blocks(44, 16), 6);
    }

    #[test]
    fn csv_time_units_round_up() {
        assert_eq!(csv_time_units(512), 1);
        assert_eq!(csv_time_units(1), 1);
        assert_eq!(csv_time_units(1025), 3);
    }

    #[test]
    fn paper_link_budgets_reproduced() {
        // ≈ 4.2 MB/yr and ≈ 1.07 bps for headers.
        assert_eq!(header_bytes_per_year(), 80 * 52_560);
        assert!((header_bps() - 1.07).abs() < 0.02, "got {}", header_bps());
        // ≈ 1.05 GB/yr and ≈ 267 bps at 20 kB filters.
        let gib = 1024f64 * 1024f64 * 1024f64;
        let gb_per_year = filter_bytes_per_year(20 * 1024) as f64 / gib;
        assert!((gb_per_year - 1.0).abs() < 0.06, "got {gb_per_year}");
        assert!((filter_bps(20 * 1024) - 267.0).abs() < 6.0);
    }
}