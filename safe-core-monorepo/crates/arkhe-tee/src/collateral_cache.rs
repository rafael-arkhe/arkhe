//! On-disk cache of PCCS collateral, with an explicit TTL and provenance.
//!
//! The cache exists so that a verifier can still reach a decision when the PCCS
//! is unreachable. Because "verified against collateral fetched from a remote
//! service" and "verified against collateral pulled off local disk" are very
//! different claims, every cache entry records where it came from, and the
//! loading functions rewrite the recorded source to
//! [`CollateralSource::LocalCache`]. A cached entry can therefore never be
//! reported as a live PCCS fetch (invariant `TEE-03`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use dcap_qvl::QuoteCollateralV3;
use serde::{Deserialize, Serialize};

use crate::error::{TeeError, TeeResult};

/// Default collateral time-to-live: 24 hours.
///
/// Intel's collateral carries its own `nextUpdate` deadline inside the signed
/// `tcb_info`; this TTL is a local freshness bound layered on top, not a
/// replacement for that check.
pub const DEFAULT_COLLATERAL_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// File-name prefix for cache entries.
const CACHE_FILE_PREFIX: &str = "collateral-";

/// File-name suffix for cache entries.
const CACHE_FILE_SUFFIX: &str = ".json";

/// Where a collateral set came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CollateralSource {
    /// Fetched live from a PCCS during this process.
    Pccs {
        /// Base URL that served the collateral.
        url: String,
    },
    /// Read back from a local cache file.
    LocalCache {
        /// Path of the cache file.
        path: PathBuf,
    },
}

impl CollateralSource {
    /// True only for a live PCCS fetch.
    pub fn is_pccs(&self) -> bool {
        matches!(self, CollateralSource::Pccs { .. })
    }

    /// Short human-readable description used in logs and evidence reports.
    pub fn describe(&self) -> String {
        match self {
            CollateralSource::Pccs { url } => format!("pccs:{url}"),
            CollateralSource::LocalCache { path } => format!("local-cache:{}", path.display()),
        }
    }
}

/// Freshness of a collateral set with respect to a TTL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollateralValidity {
    /// Fetched from the network and within its TTL.
    Fresh,
    /// Read from the local cache and within its TTL.
    LocalCache,
    /// Past its TTL. Usable only in degraded mode, and never silently treated
    /// as fresh.
    StaleCache,
}

/// A collateral set together with its fetch timestamp and provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedCollateral {
    /// The DCAP collateral itself.
    pub collateral: QuoteCollateralV3,
    /// When the collateral was obtained, in microseconds since the Unix epoch.
    pub fetched_at_unix_us: u64,
    /// Where the collateral was obtained from.
    pub source: CollateralSource,
}

impl CachedCollateral {
    /// Wrap a freshly obtained collateral set.
    pub fn new(
        collateral: QuoteCollateralV3,
        fetched_at_unix_us: u64,
        source: CollateralSource,
    ) -> Self {
        Self {
            collateral,
            fetched_at_unix_us,
            source,
        }
    }

    /// Age relative to an explicit `now`, saturating at zero for timestamps in
    /// the future.
    pub fn age_at(&self, now_unix_us: u64) -> Duration {
        Duration::from_micros(now_unix_us.saturating_sub(self.fetched_at_unix_us))
    }

    /// Age relative to the system clock.
    pub fn age(&self) -> Duration {
        self.age_at(crate::now_unix_us())
    }

    /// True when the entry is strictly older than `ttl`.
    ///
    /// The boundary is exclusive: an entry whose age is exactly `ttl` is *not*
    /// stale.
    pub fn is_stale(&self, ttl: Duration, now_unix_us: u64) -> bool {
        self.age_at(now_unix_us) > ttl
    }

    /// [`Self::is_stale`] against the system clock.
    pub fn is_stale_now(&self, ttl: Duration) -> bool {
        self.is_stale(ttl, crate::now_unix_us())
    }

    /// Classify this entry against `ttl`.
    pub fn validity(&self, ttl: Duration, now_unix_us: u64) -> CollateralValidity {
        if self.is_stale(ttl, now_unix_us) {
            CollateralValidity::StaleCache
        } else if self.source.is_pccs() {
            CollateralValidity::Fresh
        } else {
            CollateralValidity::LocalCache
        }
    }

    /// True when the collateral did not come from a live PCCS fetch.
    pub fn is_degraded(&self) -> bool {
        !self.source.is_pccs()
    }

    /// Timestamp rendered as RFC 3339, when representable.
    pub fn fetched_at_rfc3339(&self) -> Option<String> {
        crate::unix_us_to_rfc3339(self.fetched_at_unix_us)
    }

    /// The FMSPC encoded in the signed `tcb_info`, when it parses.
    ///
    /// `dcap_qvl::tcb_info::TcbInfo` is `camelCase` and requires all of
    /// `id`/`version`/`issueDate`/`nextUpdate`/`fmspc`/`pceId`/`tcbType`/
    /// `tcbEvaluationDataNumber`/`tcbLevels`; anything else falls back to the
    /// hash-based key.
    pub fn fmspc(&self) -> Option<String> {
        let parsed: dcap_qvl::tcb_info::TcbInfo =
            serde_json::from_str(&self.collateral.tcb_info).ok()?;
        Some(parsed.fmspc)
    }

    /// File-name stem used for this entry's cache file.
    ///
    /// Prefers the FMSPC, which is stable across reboots of the same platform;
    /// falls back to a truncated BLAKE3 of the `tcb_info` string when the signed
    /// blob cannot be parsed.
    pub fn cache_key(&self) -> String {
        match self.fmspc() {
            Some(fmspc) if !fmspc.trim().is_empty() => {
                format!("fmspc-{}", fmspc.trim().to_ascii_lowercase())
            }
            _ => {
                let digest = blake3::hash(self.collateral.tcb_info.as_bytes()).to_hex();
                format!("tcbinfo-{}", &digest[..16])
            }
        }
    }

    /// Path this entry occupies inside `dir`.
    pub fn cache_path(&self, dir: &Path) -> PathBuf {
        dir.join(format!(
            "{CACHE_FILE_PREFIX}{}{CACHE_FILE_SUFFIX}",
            self.cache_key()
        ))
    }

    /// Write the entry into `dir`, creating the directory when needed, and
    /// return the path written.
    ///
    /// The write is not atomic; a partially written file is detected as
    /// [`TeeError::CacheCorrupt`] by the loaders rather than being trusted.
    pub fn save(&self, dir: &Path) -> TeeResult<PathBuf> {
        std::fs::create_dir_all(dir).map_err(|e| TeeError::CacheIo {
            path: dir.to_path_buf(),
            source: e,
        })?;

        let path = self.cache_path(dir);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json).map_err(|e| TeeError::CacheIo {
            path: path.clone(),
            source: e,
        })?;
        Ok(path)
    }
}

/// Cache-file paths present in `dir`, sorted for deterministic iteration.
///
/// A missing directory yields an empty list; it is not an error.
pub fn list_cache_files(dir: &Path) -> TeeResult<Vec<PathBuf>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(TeeError::CacheIo {
                path: dir.to_path_buf(),
                source: e,
            })
        }
    };

    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| TeeError::CacheIo {
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };
        if name.starts_with(CACHE_FILE_PREFIX) && name.ends_with(CACHE_FILE_SUFFIX) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

/// Read one cache file and stamp it as [`CollateralSource::LocalCache`].
fn load_file(path: &Path) -> TeeResult<CachedCollateral> {
    let raw = std::fs::read_to_string(path).map_err(|e| TeeError::CacheIo {
        path: path.to_path_buf(),
        source: e,
    })?;

    let mut entry: CachedCollateral =
        serde_json::from_str(&raw).map_err(|e| TeeError::CacheCorrupt {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;

    // Provenance is rewritten on read: a file on local disk is never evidence of
    // a live PCCS fetch, whatever the file itself claims.
    entry.source = CollateralSource::LocalCache {
        path: path.to_path_buf(),
    };
    Ok(entry)
}

/// Load the entry stored under an explicit `key`, or `None` when absent.
pub fn load_by_key(dir: &Path, key: &str) -> TeeResult<Option<CachedCollateral>> {
    let path = dir.join(format!("{CACHE_FILE_PREFIX}{key}{CACHE_FILE_SUFFIX}"));
    if !path.exists() {
        return Ok(None);
    }
    load_file(&path).map(Some)
}

/// Load the most recently fetched entry in `dir`, or `None` when the directory
/// holds no usable entry.
///
/// Corrupt entries are skipped; if every entry present is corrupt, the last
/// failure is returned as [`TeeError::CacheCorrupt`].
pub fn load_newest(dir: &Path) -> TeeResult<Option<CachedCollateral>> {
    let files = list_cache_files(dir)?;
    let mut newest: Option<CachedCollateral> = None;
    let mut last_error: Option<TeeError> = None;

    for path in files {
        match load_file(&path) {
            Ok(entry) => {
                let replace = match &newest {
                    Some(current) => entry.fetched_at_unix_us > current.fetched_at_unix_us,
                    None => true,
                };
                if replace {
                    newest = Some(entry);
                }
            }
            Err(e) => last_error = Some(e),
        }
    }

    match (newest, last_error) {
        (Some(entry), _) => Ok(Some(entry)),
        (None, Some(e)) => Err(e),
        (None, None) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    /// Test scratch directory removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!(
                "arkhe-tee-cache-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Scratch(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A structurally complete `QuoteCollateralV3` with the given `tcb_info`.
    fn collateral_with_tcb_info(tcb_info: &str) -> QuoteCollateralV3 {
        QuoteCollateralV3 {
            pck_crl_issuer_chain: "issuer".into(),
            root_ca_crl: vec![1, 2, 3],
            pck_crl: vec![4, 5, 6],
            tcb_info_issuer_chain: "tcb-issuer".into(),
            tcb_info: tcb_info.to_string(),
            tcb_info_signature: vec![7, 8, 9],
            qe_identity_issuer_chain: "qe-issuer".into(),
            qe_identity: "{}".into(),
            qe_identity_signature: vec![10, 11, 12],
            pck_certificate_chain: Some("chain".into()),
        }
    }

    const TCB_INFO_WITH_FMSPC: &str = r#"{
        "id": "TDX",
        "version": 3,
        "issueDate": "2024-01-01T00:00:00Z",
        "nextUpdate": "2030-01-01T00:00:00Z",
        "fmspc": "00906EA10000",
        "pceId": "0000",
        "tcbType": 0,
        "tcbEvaluationDataNumber": 17,
        "tcbLevels": []
    }"#;

    fn pccs(url: &str) -> CollateralSource {
        CollateralSource::Pccs { url: url.into() }
    }

    #[test]
    fn ttl_boundary_is_exclusive() {
        let ttl = Duration::from_secs(3600);
        let now = 10_000_000_000u64;

        let exactly_at_ttl = CachedCollateral::new(
            collateral_with_tcb_info(TCB_INFO_WITH_FMSPC),
            now - ttl.as_micros() as u64,
            pccs("https://pccs.example"),
        );
        assert_eq!(exactly_at_ttl.age_at(now), ttl);
        assert!(!exactly_at_ttl.is_stale(ttl, now));
        assert_eq!(
            exactly_at_ttl.validity(ttl, now),
            CollateralValidity::Fresh
        );

        let one_microsecond_past = CachedCollateral::new(
            collateral_with_tcb_info(TCB_INFO_WITH_FMSPC),
            now - ttl.as_micros() as u64 - 1,
            pccs("https://pccs.example"),
        );
        assert!(one_microsecond_past.is_stale(ttl, now));
        assert_eq!(
            one_microsecond_past.validity(ttl, now),
            CollateralValidity::StaleCache
        );
    }

    #[test]
    fn future_timestamps_saturate_to_zero_age() {
        let entry = CachedCollateral::new(
            collateral_with_tcb_info(TCB_INFO_WITH_FMSPC),
            5_000_000,
            pccs("https://pccs.example"),
        );
        assert_eq!(entry.age_at(1_000_000), Duration::ZERO);
        assert!(!entry.is_stale(Duration::from_secs(1), 1_000_000));
    }

    #[test]
    fn validity_distinguishes_network_from_local_cache() {
        let ttl = Duration::from_secs(3600);
        let now = 10_000_000_000u64;

        let fresh = CachedCollateral::new(
            collateral_with_tcb_info(TCB_INFO_WITH_FMSPC),
            now,
            pccs("https://pccs.example"),
        );
        assert_eq!(fresh.validity(ttl, now), CollateralValidity::Fresh);
        assert!(!fresh.is_degraded());

        let cached = CachedCollateral::new(
            collateral_with_tcb_info(TCB_INFO_WITH_FMSPC),
            now,
            CollateralSource::LocalCache {
                path: PathBuf::from("/tmp/x.json"),
            },
        );
        assert_eq!(cached.validity(ttl, now), CollateralValidity::LocalCache);
        assert!(cached.is_degraded());

        let stale = CachedCollateral::new(
            collateral_with_tcb_info(TCB_INFO_WITH_FMSPC),
            now - ttl.as_micros() as u64 - 5,
            pccs("https://pccs.example"),
        );
        assert_eq!(stale.validity(ttl, now), CollateralValidity::StaleCache);
    }

    #[test]
    fn json_round_trip_preserves_collateral_bytes() {
        let entry = CachedCollateral::new(
            collateral_with_tcb_info(TCB_INFO_WITH_FMSPC),
            1_700_000_000_123_456,
            pccs("https://pccs.phala.network"),
        );

        let json = serde_json::to_string_pretty(&entry).unwrap();
        let back: CachedCollateral = serde_json::from_str(&json).unwrap();
        assert_eq!(back, entry);
        assert_eq!(back.collateral.root_ca_crl, vec![1, 2, 3]);
        assert_eq!(back.collateral.pck_crl, vec![4, 5, 6]);
        assert_eq!(back.collateral.pck_certificate_chain.as_deref(), Some("chain"));
        assert_eq!(back.fetched_at_unix_us, 1_700_000_000_123_456);
    }

    #[test]
    fn save_then_load_marks_the_provenance_as_local_cache() {
        let scratch = Scratch::new("roundtrip");
        let entry = CachedCollateral::new(
            collateral_with_tcb_info(TCB_INFO_WITH_FMSPC),
            1_700_000_000_000_000,
            pccs("https://pccs.phala.network"),
        );

        let path = entry.save(scratch.path()).unwrap();
        assert!(path.exists());
        assert!(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("00906ea10000"));

        let loaded = load_newest(scratch.path()).unwrap().expect("entry present");
        assert_eq!(loaded.collateral, entry.collateral);
        assert_eq!(loaded.fetched_at_unix_us, entry.fetched_at_unix_us);
        assert!(loaded.is_degraded(), "loaded entries are never PCCS-sourced");
        match &loaded.source {
            CollateralSource::LocalCache { path: p } => assert_eq!(p, &path),
            other => panic!("expected LocalCache provenance, got {other:?}"),
        }

        let by_key = load_by_key(scratch.path(), &entry.cache_key())
            .unwrap()
            .expect("entry present by key");
        assert_eq!(by_key.fetched_at_unix_us, entry.fetched_at_unix_us);
    }

    #[test]
    fn absent_collateral_is_not_an_error() {
        let scratch = Scratch::new("absent");
        assert!(load_newest(scratch.path()).unwrap().is_none());
        assert!(load_by_key(scratch.path(), "fmspc-00906ea10000")
            .unwrap()
            .is_none());

        // A directory that does not exist at all behaves the same way.
        let missing = scratch.path().join("nope");
        assert!(load_newest(&missing).unwrap().is_none());
        assert!(list_cache_files(&missing).unwrap().is_empty());
    }

    #[test]
    fn corrupt_cache_is_reported_not_trusted() {
        let scratch = Scratch::new("corrupt");
        let bad = scratch
            .path()
            .join(format!("{CACHE_FILE_PREFIX}fmspc-deadbeef{CACHE_FILE_SUFFIX}"));
        std::fs::write(&bad, "{ not json").unwrap();

        match load_newest(scratch.path()) {
            Err(TeeError::CacheCorrupt { path, .. }) => assert_eq!(path, bad),
            other => panic!("expected CacheCorrupt, got {other:?}"),
        }
        assert!(load_by_key(scratch.path(), "fmspc-deadbeef").is_err());
    }

    #[test]
    fn newest_entry_wins_and_corrupt_files_are_skipped() {
        let scratch = Scratch::new("newest");

        let older = CachedCollateral::new(
            collateral_with_tcb_info(TCB_INFO_WITH_FMSPC),
            1_000_000,
            pccs("https://pccs.example"),
        );
        older.save(scratch.path()).unwrap();

        let newer = CachedCollateral::new(
            collateral_with_tcb_info(TCB_INFO_WITH_FMSPC),
            2_000_000,
            pccs("https://pccs.example"),
        );
        newer.save(scratch.path()).unwrap();

        std::fs::write(
            scratch.path().join(format!("{CACHE_FILE_PREFIX}junk{CACHE_FILE_SUFFIX}")),
            "nonsense",
        )
        .unwrap();

        let loaded = load_newest(scratch.path()).unwrap().unwrap();
        assert_eq!(loaded.fetched_at_unix_us, 2_000_000);
    }

    #[test]
    fn cache_key_uses_fmspc_when_available_and_hashes_otherwise() {
        let with_fmspc = CachedCollateral::new(
            collateral_with_tcb_info(TCB_INFO_WITH_FMSPC),
            0,
            pccs("https://pccs.example"),
        );
        assert_eq!(with_fmspc.fmspc().as_deref(), Some("00906EA10000"));
        assert_eq!(with_fmspc.cache_key(), "fmspc-00906ea10000");

        let without = CachedCollateral::new(
            collateral_with_tcb_info("not json at all"),
            0,
            pccs("https://pccs.example"),
        );
        assert!(without.fmspc().is_none());
        let key = without.cache_key();
        assert!(key.starts_with("tcbinfo-"), "unexpected key {key}");
        assert_eq!(key, without.cache_key(), "fallback key must be stable");
    }

    #[test]
    fn timestamps_render_as_rfc3339() {
        let entry = CachedCollateral::new(
            collateral_with_tcb_info(TCB_INFO_WITH_FMSPC),
            1_700_000_000_000_000,
            pccs("https://pccs.example"),
        );
        assert_eq!(
            entry.fetched_at_rfc3339().as_deref(),
            Some("2023-11-14T22:13:20+00:00")
        );
    }
}
