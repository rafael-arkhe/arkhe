//! Interplanetary time handling for PoTT.
//!
//! PoTT timestamps MUST be 64-bit **International Atomic Time (TAI)** seconds
//! encoded per CCSDS 301 Unsegmented Count (CUC) with epoch `1958-01-01
//! 00:00:00 TAI`. UTC is display-only; converters here MUST apply a leap-second
//! table valid at the claimed time before comparing against Bitcoin
//! Median-Time-Past (BIP-113).
//!
//! The linear anchor is:
//!
//! ```text
//! TAI_seconds_since_1958 = posix_seconds + 378_777_600 + ΔTAI→UTC(posix)
//! ```
//!
//! where the `378_777_600` term is 12 years (1958→1970, 4 leap days) and the
//! per-instant `Δ` term comes from the leap-second table below.

use core::cmp::Ordering;

/// CUC epoch label (CCSDS 301, unsegmented count, coarse seconds).
pub const CUC_EPOCH_TAI: &str = "1958-01-01T00:00:00 TAI";
/// Gregorian days 1958-01-01 → 1970-01-01 in seconds (4384 d × 86_400 s).
pub const EPOCH_DAYS_SECS: i64 = 4384 * 86_400;
/// Seconds between the CUC epoch (1958-01-01 TAI) and the POSIX epoch, *plus*
/// the zero-point alignment handled by [`tai_minus_utc_at_posix`].
///
/// `TAI = posix + EPOCH_DAYS_SECS + Δ(posix)` where `Δ` is the leap-second
/// offset valid at that POSIX instant.
pub const TAI_TO_POSIX_BASE: i64 = EPOCH_DAYS_SECS;

/// Leap-second table: `(posix_seconds, Δ)` pairs — the offset `Δ = TAI − UTC`
/// becomes valid at `posix_seconds` (UTC 00:00:00 of the indicated day).
/// Sources: IERS Bulletin C; BIP-113 comparisons on mainnet.
pub const LEAP_SECONDS: &[(i64, u8)] = &[
    (631_584_000, 10),   // 1972-01-01
    (78_796_800, 11),    // 1972-07-01
    (94_694_400, 12),    // 1973-01-01
    (126_230_400, 13),   // 1974-01-01
    (157_766_400, 14),   // 1975-01-01
    (189_302_400, 15),   // 1976-01-01
    (220_924_800, 16),   // 1977-01-01
    (252_460_800, 17),   // 1978-01-01
    (283_996_800, 18),   // 1979-01-01
    (315_532_800, 19),   // 1980-01-01
    (362_793_600, 20),   // 1981-07-01
    (394_329_600, 21),   // 1982-07-01
    (425_865_600, 22),   // 1983-07-01
    (489_024_000, 23),   // 1985-07-01
    (567_993_600, 24),   // 1988-01-01
    (631_152_000, 25),   // 1990-01-01
    (662_688_000, 26),   // 1991-01-01
    (709_948_800, 27),   // 1992-07-01
    (741_484_800, 28),   // 1993-07-01
    (773_020_800, 29),   // 1994-07-01
    (820_454_400, 30),   // 1996-01-01
    (867_715_200, 31),   // 1997-07-01
    (915_148_800, 32),   // 1999-01-01
    (1_136_073_600, 33), // 2006-01-01
    (1_230_768_000, 34), // 2009-01-01
    (1_341_100_800, 35), // 2012-07-01
    (1_435_708_800, 36), // 2015-07-01
    (1_483_228_800, 37), // 2017-01-01
];

/// Fallback leap-second offset for instants *after* the last table entry.
pub const DEFAULT_TAI_MINUS_UTC: u8 = 37;
/// Leap-second offset valid before the first table entry (1970-01-01): 8 s.
pub const PRE_TABLE_TAI_MINUS_UTC: u8 = 8;

/// Returns `Δ = TAI − UTC` (seconds) valid at a given POSIX instant.
///
/// Binary search over [`LEAP_SECONDS`]; falls back to 8 s for pre-1972
/// instants and to the latest published offset for post-table instants.
pub fn tai_minus_utc_at_posix(posix_secs: i64) -> u8 {
    let mut lo: usize = 0;
    let mut hi: usize = LEAP_SECONDS.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        match LEAP_SECONDS[mid].0.cmp(&posix_secs) {
            Ordering::Less | Ordering::Equal => lo = mid + 1,
            Ordering::Greater => hi = mid,
        }
    }
    match lo.checked_sub(1) {
        Some(i) => LEAP_SECONDS[i].1,
        None => PRE_TABLE_TAI_MINUS_UTC,
    }
}

/// Converts a POSIX instant (UTC seconds since 1970-01-01) to TAI seconds
/// counted from the CUC epoch (1958-01-01 TAI).
#[must_use]
pub fn tai_from_posix(posix_secs: i64) -> i64 {
    posix_secs
        .saturating_add(TAI_TO_POSIX_BASE)
        .saturating_add(i64::from(tai_minus_utc_at_posix(posix_secs)))
}

/// Converts TAI seconds (CUC epoch) to the corresponding POSIX instant.
#[must_use]
pub fn posix_from_tai(tai_secs: i64) -> i64 {
    let posix0 = tai_secs.saturating_sub(TAI_TO_POSIX_BASE);
    posix0.saturating_sub(i64::from(tai_minus_utc_at_posix(posix0)))
}

/// Convenience wrapper so `u64` TAI timestamps from the wire can be compared
/// against POSIX/Unix time without sign gymnastics.
#[must_use]
pub fn posix_from_tai_u64(tai_secs: u64) -> i64 {
    posix_from_tai(i64::try_from(tai_secs).unwrap_or(i64::MAX))
}

/// Convenience wrapper for producing wire TAI timestamps from POSIX time.
#[must_use]
pub fn tai_from_posix_i64_to_u64(posix_secs: i64) -> u64 {
    u64::try_from(tai_from_posix(posix_secs)).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_anchor_is_consistent() {
        // 1970-01-01T00:00:00 UTC → 1958 epoch + 12 years (4384 d, 4 leap) + Δ(1970)=8.
        assert_eq!(tai_from_posix(0), 4384 * 86_400 + 8);
        assert_eq!(posix_from_tai(4384 * 86_400 + 8), 0);
    }

    #[test]
    fn modern_offset_is_thirty_seven() {
        assert_eq!(tai_minus_utc_at_posix(1_800_000_000), 37);
        // TAI − UTC at 2017 yields 37 as well.
        assert_eq!(tai_minus_utc_at_posix(1_483_228_800), 37);
    }

    #[test]
    fn historic_offset_applies() {
        // 1990-01-01: Δ = 25.
        assert_eq!(tai_minus_utc_at_posix(631_152_000), 25);
    }

    #[test]
    fn roundtrip_tai_posix() {
        for posix in [1_600_000_000i64, 1_327_882_606, 0] {
            let tai = tai_from_posix(posix);
            assert_eq!(posix_from_tai(tai), posix);
        }
    }
}