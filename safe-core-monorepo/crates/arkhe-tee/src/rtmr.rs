//! RTMR extension driver.
//!
//! Runtime Measurement Registers (`RTMR0..RTMR3`) are extended with
//! `SHA-384(previous_value || new_digest)`. Two Linux backends are probed, in
//! order:
//!
//! 1. the TSM measurement-register interface, `/sys/kernel/tsm/mr/rtmrN`;
//! 2. the TDX guest device, `/dev/tdx-guest` (ioctl).
//!
//! Everything outside [`platform`] is platform-independent and testable
//! anywhere: the SHA-384 extension law, the agent-artifact digest, the index
//! validation and the ioctl request-number arithmetic.
//!
//! # This code has never run against TDX hardware
//!
//! The kernel ABI details below are *not* verified against a target kernel.
//! See [`TdxExtendRtmrReq`] and [`TDX_EXTEND_RTMR_IOCTL`] for the specifics and
//! for the discrepancy in the original design note.

use crate::error::{TeeError, TeeResult};
use crate::types::Digest48;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha384};

/// Number of RTMRs exposed by TDX.
pub const RTMR_COUNT: u32 = 4;

/// Digest width of a single RTMR, in bytes.
pub const RTMR_DIGEST_LEN: usize = 48;

/// Domain separation prefix for agent-artifact extensions.
pub const AGENT_ARTIFACT_DOMAIN: &[u8] = b"ARKHE-AGENT-ARTIFACT";

/// Directory of the TSM measurement-register interface (probed first).
pub const TSM_SYSFS_DIR: &str = "/sys/kernel/tsm/mr";

/// Path of the TDX guest device (probed second).
pub const TDX_GUEST_DEVICE: &str = "/dev/tdx-guest";

/// SHA-384 over the concatenation of `parts`.
pub fn sha384(parts: &[&[u8]]) -> [u8; RTMR_DIGEST_LEN] {
    let mut hasher = Sha384::new();
    for part in parts {
        hasher.update(part);
    }
    let mut out = [0u8; RTMR_DIGEST_LEN];
    out.copy_from_slice(&hasher.finalize());
    out
}

/// The RTMR extension law: `SHA-384(current || new)`.
///
/// No truncation, no reordering, no padding: this is the identity the hardware
/// applies, so a locally computed value can be compared against a quote.
pub fn extend_digest(current: &[u8; RTMR_DIGEST_LEN], new: &[u8; RTMR_DIGEST_LEN]) -> [u8; RTMR_DIGEST_LEN] {
    sha384(&[current, new])
}

/// Digest of a 32-byte agent artifact hash for RTMR extension:
/// `SHA-384(AGENT_ARTIFACT_DOMAIN || hash32)`.
///
/// Pure: usable to predict the post-extension RTMR value before any hardware
/// operation, and testable on any platform.
pub fn agent_artifact_digest(artifact_hash: &[u8; 32]) -> [u8; RTMR_DIGEST_LEN] {
    sha384(&[AGENT_ARTIFACT_DOMAIN, artifact_hash])
}

/// Reject RTMR indices outside `0..RTMR_COUNT`.
pub fn validate_index(index: u32) -> TeeResult<()> {
    if index < RTMR_COUNT {
        Ok(())
    } else {
        Err(TeeError::RtmrIndexOutOfRange {
            index,
            max_inclusive: RTMR_COUNT - 1,
        })
    }
}

/// Which RTMR extension backend the host provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RtmrBackend {
    /// `/sys/kernel/tsm/mr/rtmrN` — the TSM measurement-register interface.
    SysfsTsm,
    /// `/dev/tdx-guest` — the TDX guest device.
    TdxGuestDevice,
}

impl RtmrBackend {
    /// Lowercase identifier used in logs and JSON.
    pub const fn as_str(self) -> &'static str {
        match self {
            RtmrBackend::SysfsTsm => "sysfs-tsm",
            RtmrBackend::TdxGuestDevice => "tdx-guest-device",
        }
    }
}

/// Outcome of an RTMR extension attempt.
///
/// The kernel performs the extension, so the digest this crate *submits* is not
/// by itself evidence that the register holds the expected value. The fields
/// here separate the three claims: what was submitted, what the local SHA-384
/// law predicts, and what was actually read back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RtmrExtension {
    /// Index that was extended.
    pub index: u32,
    /// Backend that performed the operation.
    pub backend: RtmrBackend,
    /// Digest handed to the kernel. For the sysfs backend this is the raw
    /// 48-byte digest and the kernel computes the extension; the ioctl payload
    /// carries the same digest plus the index.
    pub submitted: Digest48,
    /// `SHA-384(previous || submitted)` computed locally, when the previous
    /// value could be read before the operation.
    ///
    /// Advisory: a concurrent extender can change the register between the
    /// pre-read and the write.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub expected: Option<Digest48>,
    /// Value read back after the operation, when the backend exposes a readable
    /// register.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub observed: Option<Digest48>,
    /// `expected == observed`, available only when both are known.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub matches_expected: Option<bool>,
}

// ---------------------------------------------------------------------------
// Kernel ABI constants
// ---------------------------------------------------------------------------

/// `_IOC_NRBITS` from `asm-generic/ioctl.h`.
pub const IOC_NRBITS: u32 = 8;
/// `_IOC_TYPEBITS` from `asm-generic/ioctl.h`.
pub const IOC_TYPEBITS: u32 = 8;
/// `_IOC_SIZEBITS` from `asm-generic/ioctl.h`.
pub const IOC_SIZEBITS: u32 = 14;
/// `_IOC_DIRBITS` from `asm-generic/ioctl.h`.
pub const IOC_DIRBITS: u32 = 2;
/// `_IOC_NRSHIFT`.
pub const IOC_NRSHIFT: u32 = 0;
/// `_IOC_TYPESHIFT`.
pub const IOC_TYPESHIFT: u32 = IOC_NRSHIFT + IOC_NRBITS;
/// `_IOC_SIZESHIFT`.
pub const IOC_SIZESHIFT: u32 = IOC_TYPESHIFT + IOC_TYPEBITS;
/// `_IOC_DIRSHIFT`.
pub const IOC_DIRSHIFT: u32 = IOC_SIZESHIFT + IOC_SIZEBITS;
/// `_IOC_WRITE` — userspace writes to the kernel.
pub const IOC_WRITE: u32 = 1;

/// `_IOW(type, nr, size)`, computed rather than transcribed.
///
/// Deriving the number from `size_of` keeps the encoded size field and the
/// struct that is actually passed in sync — which is precisely what the
/// original design note got wrong.
pub const fn iow(type_: u8, nr: u8, size: usize) -> u64 {
    let size_bits = (size as u32) & ((1 << IOC_SIZEBITS) - 1);
    ((IOC_WRITE << IOC_DIRSHIFT)
        | (size_bits << IOC_SIZESHIFT)
        | ((type_ as u32) << IOC_TYPESHIFT)
        | ((nr as u32) << IOC_NRSHIFT)) as u64
}

/// Payload for the RTMR-extension ioctl.
///
/// # ABI uncertainty — must be confirmed on the target kernel
///
/// The original design note hard-coded `TDX_CMD_EXTEND_RTMR = 0x40085403`,
/// which decomposes as `_IOW('T', 3, 8)`: a *write* command whose encoded size
/// field is **8 bytes**. That is inconsistent with any struct of the size
/// actually being passed (the note also described a 56-byte payload). This
/// payload follows the shape hinted at by the Linux TDX guest ABI — a 64-byte
/// digest buffer plus an index — giving a 72-byte struct, and
/// [`TDX_EXTEND_RTMR_IOCTL`] is derived from `size_of::<TdxExtendRtmrReq>()`
/// rather than transcribed, so the two can never drift apart again.
///
/// Whether the kernel expects `data[48]`, `data[64]` or a different index type
/// has **not** been verified. Treat any value produced by this module as
/// unproven until it has been checked against the headers of the kernel in use.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TdxExtendRtmrReq {
    /// Digest buffer. The 48-byte value occupies the first 48 bytes; the
    /// remainder is zero.
    pub data: [u8; 64],
    /// RTMR index.
    pub index: u64,
}

/// The ioctl request number, derived from the size of [`TdxExtendRtmrReq`].
pub const TDX_EXTEND_RTMR_IOCTL: u64 = iow(b'T', 3, core::mem::size_of::<TdxExtendRtmrReq>());

/// Path of the TSM sysfs attribute for an RTMR index.
pub fn sysfs_rtmr_path(index: u32) -> String {
    format!("{TSM_SYSFS_DIR}/rtmr{index}")
}

// ---------------------------------------------------------------------------
// Platform backends
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "linux"))]
mod platform {
    //! Non-Linux stub: every hardware operation reports a typed platform error
    //! so that the crate still compiles and its pure logic remains testable.

    use super::*;

    const NO_BACKEND: &str = "RTMR extension requires Linux, with either the TSM \
         measurement-register interface (/sys/kernel/tsm/mr/rtmrN) or /dev/tdx-guest; \
         this build targets a non-Linux platform";

    /// Always `None`: no RTMR backend exists off Linux.
    pub fn detect_backend() -> Option<RtmrBackend> {
        None
    }

    fn unsupported(index: u32) -> TeeError {
        TeeError::RtmrUnsupportedPlatform {
            tried: format!(
                "{} and {TDX_GUEST_DEVICE} — {NO_BACKEND}",
                sysfs_rtmr_path(index)
            ),
        }
    }

    /// Fails with [`TeeError::RtmrUnsupportedPlatform`].
    pub fn read_rtmr(index: u32) -> TeeResult<Digest48> {
        validate_index(index)?;
        Err(unsupported(index))
    }

    /// Fails with [`TeeError::RtmrUnsupportedPlatform`].
    pub fn extend(index: u32, _digest: &[u8; RTMR_DIGEST_LEN]) -> TeeResult<RtmrExtension> {
        validate_index(index)?;
        Err(unsupported(index))
    }
}

#[cfg(target_os = "linux")]
mod platform {
    //! Linux backends: TSM sysfs first, `/dev/tdx-guest` ioctl second.
    //!
    //! The digest is submitted raw and the *kernel* performs the SHA-384
    //! extension. That is the assumed ABI for both backends and is the part most
    //! in need of confirmation on real hardware — see the module documentation.

    use super::*;
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};
    use std::os::unix::io::AsRawFd;

    /// Probe order: sysfs interface, then the guest device.
    pub fn detect_backend() -> Option<RtmrBackend> {
        if Path::new(TSM_SYSFS_DIR).is_dir() {
            Some(RtmrBackend::SysfsTsm)
        } else if Path::new(TDX_GUEST_DEVICE).exists() {
            Some(RtmrBackend::TdxGuestDevice)
        } else {
            None
        }
    }

    fn no_backend(index: u32) -> TeeError {
        TeeError::RtmrUnsupportedPlatform {
            tried: format!(
                "{} and {} (requested rtmr{index})",
                sysfs_rtmr_path(index),
                TDX_GUEST_DEVICE
            ),
        }
    }

    fn backend_failure(backend: RtmrBackend, op: &str, reason: impl std::fmt::Display) -> TeeError {
        TeeError::RtmrBackendFailure {
            backend: backend.as_str().to_string(),
            op: op.to_string(),
            reason: reason.to_string(),
        }
    }

    fn sysfs_path(index: u32) -> PathBuf {
        PathBuf::from(sysfs_rtmr_path(index))
    }

    /// Read one 48-byte register through the TSM sysfs attribute.
    fn read_sysfs(index: u32) -> TeeResult<Digest48> {
        let path = sysfs_path(index);
        let mut file = std::fs::File::open(&path)
            .map_err(|e| backend_failure(RtmrBackend::SysfsTsm, "open", format!("{}: {e}", path.display())))?;

        let mut buf = [0u8; RTMR_DIGEST_LEN];
        file.read_exact(&mut buf).map_err(|e| {
            backend_failure(RtmrBackend::SysfsTsm, "read", format!("{}: {e}", path.display()))
        })?;
        Ok(buf.into())
    }

    /// Best-effort pre/post read: `None` when the register is not readable,
    /// which is the case for the ioctl backend.
    fn try_read(backend: RtmrBackend, index: u32) -> Option<Digest48> {
        match backend {
            RtmrBackend::SysfsTsm => read_sysfs(index).ok(),
            RtmrBackend::TdxGuestDevice => None,
        }
    }

    /// Submit the digest through the sysfs attribute.
    fn write_sysfs(index: u32, digest: &[u8; RTMR_DIGEST_LEN]) -> TeeResult<()> {
        let path = sysfs_path(index);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|e| backend_failure(RtmrBackend::SysfsTsm, "open-for-write", format!("{}: {e}", path.display())))?;

        file.write_all(digest).map_err(|e| {
            backend_failure(RtmrBackend::SysfsTsm, "write", format!("{}: {e}", path.display()))
        })
    }

    /// Submit the digest through `TDX_CMD_EXTEND_RTMR` on `/dev/tdx-guest`.
    ///
    /// The only `unsafe` block in the crate; the safety argument is inline.
    fn ioctl_extend(index: u32, digest: &[u8; RTMR_DIGEST_LEN]) -> TeeResult<()> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(TDX_GUEST_DEVICE)
            .map_err(|e| {
                backend_failure(
                    RtmrBackend::TdxGuestDevice,
                    "open",
                    format!("{TDX_GUEST_DEVICE}: {e}"),
                )
            })?;

        let mut req = TdxExtendRtmrReq {
            data: [0u8; 64],
            index: index as u64,
        };
        req.data[..RTMR_DIGEST_LEN].copy_from_slice(digest);

        // SAFETY: the descriptor in `file` is valid and stays open for the whole
        // call; `req` is a live, correctly aligned `#[repr(C)]` value that
        // outlives the call and is uniquely borrowed for it. The request number
        // is derived from `size_of::<TdxExtendRtmrReq>()`, so the size encoded in
        // `_IOW` matches the payload actually passed. This is the crate's only
        // unsafe operation, and it neither dereferences a raw pointer nor
        // constructs a reference from untrusted input.
        #[allow(unsafe_code)]
        let ret = unsafe {
            libc::ioctl(
                file.as_raw_fd(),
                TDX_EXTEND_RTMR_IOCTL as libc::c_ulong,
                &mut req as *mut TdxExtendRtmrReq,
            )
        };

        if ret != 0 {
            let err = std::io::Error::last_os_error();
            return Err(backend_failure(
                RtmrBackend::TdxGuestDevice,
                "ioctl(TDX_CMD_EXTEND_RTMR)",
                format!("{err} (errno {:?})", err.raw_os_error()),
            ));
        }
        Ok(())
    }

    /// Read an RTMR. Only the sysfs backend can read.
    pub fn read_rtmr(index: u32) -> TeeResult<Digest48> {
        validate_index(index)?;
        match detect_backend() {
            Some(RtmrBackend::SysfsTsm) => read_sysfs(index),
            Some(RtmrBackend::TdxGuestDevice) => Err(backend_failure(
                RtmrBackend::TdxGuestDevice,
                "read",
                "reading RTMRs through the guest device is not implemented by this crate; \
                 the TSM sysfs interface is required for readback",
            )),
            None => Err(no_backend(index)),
        }
    }

    /// Extend an RTMR, then read the register back when possible.
    pub fn extend(index: u32, digest: &[u8; RTMR_DIGEST_LEN]) -> TeeResult<RtmrExtension> {
        validate_index(index)?;
        let backend = detect_backend().ok_or_else(|| no_backend(index))?;

        let previous = try_read(backend, index);

        match backend {
            RtmrBackend::SysfsTsm => write_sysfs(index, digest)?,
            RtmrBackend::TdxGuestDevice => ioctl_extend(index, digest)?,
        }

        let observed = try_read(backend, index);
        let expected = previous.map(|prev| extend_digest(prev.as_bytes(), digest).into());
        let matches_expected = match (&expected, &observed) {
            (Some(expected), Some(observed)) => Some(expected == observed),
            _ => None,
        };

        Ok(RtmrExtension {
            index,
            backend,
            submitted: (*digest).into(),
            expected,
            observed,
            matches_expected,
        })
    }
}

pub use platform::{detect_backend, extend, read_rtmr};

/// Extend an RTMR with the digest of a 32-byte agent artifact hash.
///
/// The digest is computed by [`agent_artifact_digest`]; the hardware operation
/// is delegated to [`extend`], so off a TDX host this returns
/// [`TeeError::RtmrUnsupportedPlatform`] after the digest has been derived.
pub fn extend_with_agent_artifact(
    index: u32,
    artifact_hash: &[u8; 32],
) -> TeeResult<RtmrExtension> {
    let digest = agent_artifact_digest(artifact_hash);
    extend(index, &digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha384_matches_a_published_known_answer_vector() {
        // NIST vector: SHA-384("abc").
        let expected = "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed\
                        8086072ba1e7cc2358baeca134c825a7";
        assert_eq!(hex::encode(sha384(&[b"abc"])), expected);
    }

    #[test]
    fn sha384_concatenates_its_parts() {
        assert_eq!(sha384(&[b"ab", b"c"]), sha384(&[b"a", b"bc"]));
        assert_ne!(sha384(&[b"abc"]), sha384(&[b"acb"]));
    }

    #[test]
    fn extension_law_is_sha384_of_the_concatenation() {
        let prev = [0u8; RTMR_DIGEST_LEN];
        let new = [0u8; RTMR_DIGEST_LEN];
        let mut joined = [0u8; RTMR_DIGEST_LEN * 2];
        joined[..RTMR_DIGEST_LEN].copy_from_slice(&prev);
        joined[RTMR_DIGEST_LEN..].copy_from_slice(&new);

        assert_eq!(extend_digest(&prev, &new), sha384(&[&joined]));
        assert_eq!(extend_digest(&prev, &new).len(), RTMR_DIGEST_LEN);
    }

    #[test]
    fn extension_is_order_sensitive() {
        let a = [0x11u8; RTMR_DIGEST_LEN];
        let b = [0x22u8; RTMR_DIGEST_LEN];
        assert_ne!(extend_digest(&a, &b), extend_digest(&b, &a));
    }

    #[test]
    fn repeated_extension_is_deterministic_and_non_idempotent() {
        let start = [0u8; RTMR_DIGEST_LEN];
        let digest = [0x5au8; RTMR_DIGEST_LEN];
        let once = extend_digest(&start, &digest);
        let twice = extend_digest(&once, &digest);
        assert_eq!(once, extend_digest(&start, &digest));
        assert_ne!(once, start);
        assert_ne!(once, twice);
    }

    #[test]
    fn agent_artifact_digest_is_domain_separated() {
        let hash = [0x42u8; 32];
        let digest = agent_artifact_digest(&hash);
        assert_eq!(digest.len(), RTMR_DIGEST_LEN);

        // Domain-tagged, not a bare hash of the artifact hash.
        assert_ne!(digest, sha384(&[&hash]));
        assert_eq!(digest, sha384(&[AGENT_ARTIFACT_DOMAIN, &hash]));

        // Deterministic, and sensitive to the artifact hash.
        assert_eq!(digest, agent_artifact_digest(&hash));
        let mut other = hash;
        other[31] ^= 0x01;
        assert_ne!(digest, agent_artifact_digest(&other));
    }

    #[test]
    fn index_validation_covers_the_rtmr_range() {
        for index in 0..RTMR_COUNT {
            assert!(validate_index(index).is_ok(), "rtmr{index} should be valid");
        }
        match validate_index(RTMR_COUNT) {
            Err(TeeError::RtmrIndexOutOfRange {
                index,
                max_inclusive,
            }) => {
                assert_eq!(index, RTMR_COUNT);
                assert_eq!(max_inclusive, RTMR_COUNT - 1);
            }
            other => panic!("expected RtmrIndexOutOfRange, got {other:?}"),
        }
        assert!(validate_index(u32::MAX).is_err());
    }

    #[test]
    fn sysfs_paths_follow_the_tsm_layout() {
        assert_eq!(sysfs_rtmr_path(0), "/sys/kernel/tsm/mr/rtmr0");
        assert_eq!(sysfs_rtmr_path(3), "/sys/kernel/tsm/mr/rtmr3");
    }

    #[test]
    fn iow_reproduces_the_original_hard_coded_constant_for_an_8_byte_payload() {
        // The note's `0x40085403` is `_IOW('T', 3, 8)`: an 8-byte size field.
        assert_eq!(iow(b'T', 3, 8), 0x4008_5403);
        assert_eq!(iow(b'T', 3, 8), 0x4008_5403u64);
    }

    #[test]
    fn ioctl_number_is_derived_from_the_submitted_struct_size() {
        assert_eq!(
            core::mem::size_of::<TdxExtendRtmrReq>(),
            72,
            "a 64-byte digest buffer plus a u64 index is 72 bytes"
        );
        assert_eq!(TDX_EXTEND_RTMR_IOCTL, iow(b'T', 3, 72));
        // Which differs from the note's constant, because the sizes differ.
        assert_ne!(TDX_EXTEND_RTMR_IOCTL, 0x4008_5403);
        // Direction and type fields are unchanged: write, 'T'.
        assert_eq!(TDX_EXTEND_RTMR_IOCTL & 0xC000_0000, 0x4000_0000);
        assert_eq!((TDX_EXTEND_RTMR_IOCTL >> IOC_TYPESHIFT) & 0xff, b'T' as u64);
        assert_eq!(TDX_EXTEND_RTMR_IOCTL & 0xff, 3);
    }

    #[test]
    fn backend_names_are_stable() {
        assert_eq!(RtmrBackend::SysfsTsm.as_str(), "sysfs-tsm");
        assert_eq!(RtmrBackend::TdxGuestDevice.as_str(), "tdx-guest-device");
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn off_linux_every_hardware_operation_fails_with_a_typed_error() {
        assert_eq!(detect_backend(), None);

        match read_rtmr(0) {
            Err(TeeError::RtmrUnsupportedPlatform { tried }) => {
                assert!(tried.contains("/sys/kernel/tsm/mr/rtmr0"), "{tried}");
                assert!(tried.contains("/dev/tdx-guest"), "{tried}");
            }
            other => panic!("expected RtmrUnsupportedPlatform, got {other:?}"),
        }

        match extend(0, &[0u8; RTMR_DIGEST_LEN]) {
            Err(TeeError::RtmrUnsupportedPlatform { .. }) => {}
            other => panic!("expected RtmrUnsupportedPlatform, got {other:?}"),
        }

        match extend_with_agent_artifact(2, &[0xabu8; 32]) {
            Err(TeeError::RtmrUnsupportedPlatform { .. }) => {}
            other => panic!("expected RtmrUnsupportedPlatform, got {other:?}"),
        }

        // Index validation happens before any hardware probe.
        match extend(9, &[0u8; RTMR_DIGEST_LEN]) {
            Err(TeeError::RtmrIndexOutOfRange { index, .. }) => assert_eq!(index, 9),
            other => panic!("expected RtmrIndexOutOfRange, got {other:?}"),
        }
    }
}
