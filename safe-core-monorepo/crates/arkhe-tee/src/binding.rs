//! Provenance binding.
//!
//! A [`ProvenanceBinding`] is a compact, serializable record that ties a GDID
//! and the hash of an agent binary to the measurements of a TDX quote
//! (`MRTD`, the four RTMRs, `report_data`) plus the quote format version.
//!
//! It exists so that a later quote can be checked against an earlier decision:
//! if any component moves, the binding no longer matches. This type is defined
//! only here — nothing else in the repository provides it.

use serde::{Deserialize, Serialize};

use crate::error::{TeeError, TeeResult};
use crate::quote_meta::TdxQuoteMetadata;
use crate::rtmr::{agent_artifact_digest, extend_digest, RTMR_COUNT};
use crate::types::{Digest32, Digest48, ReportData64};

/// A binding between an agent identity and the measurements of a TDX quote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceBinding {
    /// GDID of the agent the binding belongs to.
    pub gdid: String,
    /// BLAKE3 hash of the agent binary.
    pub binary_hash: Digest32,
    /// Quote format version the measurements were taken from.
    pub quote_version: u16,
    /// `MRTD` at binding time.
    pub mr_td: Digest48,
    /// `RTMR0..RTMR3` at binding time, in index order.
    pub rtmrs: [Digest48; RTMR_COUNT as usize],
    /// `report_data` at binding time.
    pub report_data: ReportData64,
    /// When the binding was created, in microseconds since the Unix epoch.
    pub bound_at_unix_us: u64,
}

impl ProvenanceBinding {
    /// Build a binding from decoded quote metadata.
    ///
    /// Fails when `gdid` is empty: an anonymous binding cannot be checked
    /// against anything later.
    pub fn new(
        gdid: impl Into<String>,
        binary_hash: Digest32,
        quote_version: u16,
        mr_td: Digest48,
        rtmrs: [Digest48; RTMR_COUNT as usize],
        report_data: ReportData64,
        bound_at_unix_us: u64,
    ) -> TeeResult<Self> {
        let gdid = gdid.into();
        if gdid.trim().is_empty() {
            return Err(TeeError::internal("provenance binding requires a non-empty gdid"));
        }
        Ok(Self {
            gdid,
            binary_hash,
            quote_version,
            mr_td,
            rtmrs,
            report_data,
            bound_at_unix_us,
        })
    }

    /// Build a binding directly from quote metadata.
    pub fn from_quote_meta(
        gdid: impl Into<String>,
        binary_hash: Digest32,
        meta: &TdxQuoteMetadata,
        bound_at_unix_us: u64,
    ) -> TeeResult<Self> {
        Self::new(
            gdid,
            binary_hash,
            meta.quote_version,
            meta.mr_td,
            meta.rtmrs,
            meta.report_data,
            bound_at_unix_us,
        )
    }

    /// BLAKE3 hash of a binary, ready to be used as
    /// [`Self::binary_hash`].
    pub fn hash_binary(bytes: &[u8]) -> Digest32 {
        Digest32::from(*blake3::hash(bytes).as_bytes())
    }

    /// Check a later quote against this binding.
    ///
    /// Every field is compared separately so that a mismatch names the field
    /// that moved. The first divergence found is reported.
    pub fn verify_against(&self, meta: &TdxQuoteMetadata) -> TeeResult<()> {
        if self.quote_version != meta.quote_version {
            return Err(TeeError::BindingMismatch {
                field: "quote_version".into(),
                expected: self.quote_version.to_string(),
                actual: meta.quote_version.to_string(),
            });
        }
        if self.mr_td != meta.mr_td {
            return Err(TeeError::BindingMismatch {
                field: "mr_td".into(),
                expected: self.mr_td.to_hex(),
                actual: meta.mr_td.to_hex(),
            });
        }
        for index in 0..RTMR_COUNT as usize {
            let bound = self.rtmrs[index];
            let observed = meta.rtmrs[index];
            if bound != observed {
                return Err(TeeError::BindingMismatch {
                    field: format!("rtmr{index}"),
                    expected: bound.to_hex(),
                    actual: observed.to_hex(),
                });
            }
        }
        if self.report_data != meta.report_data {
            return Err(TeeError::BindingMismatch {
                field: "report_data".into(),
                expected: self.report_data.to_hex(),
                actual: meta.report_data.to_hex(),
            });
        }
        Ok(())
    }

    /// The RTMR value this binding implies for `index` after extending with
    /// `agent_artifact_hash`.
    ///
    /// Pure: it lets a caller compare a binding against a predicted value
    /// without touching hardware.
    pub fn rtmr_after_agent_artifact_extension(
        &self,
        index: u32,
        agent_artifact_hash: &Digest32,
    ) -> Option<Digest48> {
        let current = self.rtmrs.get(index as usize)?;
        let digest = agent_artifact_digest(agent_artifact_hash.as_bytes());
        Some(extend_digest(current.as_bytes(), &digest).into())
    }

    /// Canonical BLAKE3 digest of the binding, usable as its identifier.
    ///
    /// Fields are length-prefixed so distinct bindings cannot collide by
    /// concatenation ambiguity.
    pub fn digest(&self) -> Digest32 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ARKHE-TEE-BINDING");
        update_with_len(&mut hasher, self.gdid.as_bytes());
        update_with_len(&mut hasher, self.binary_hash.as_bytes());
        hasher.update(&self.quote_version.to_le_bytes());
        update_with_len(&mut hasher, self.mr_td.as_bytes());
        for rtmr in &self.rtmrs {
            update_with_len(&mut hasher, rtmr.as_bytes());
        }
        update_with_len(&mut hasher, self.report_data.as_bytes());
        hasher.update(&self.bound_at_unix_us.to_le_bytes());
        Digest32::from(*hasher.finalize().as_bytes())
    }

    /// Serialize the binding as pretty JSON.
    pub fn to_json(&self) -> TeeResult<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Parse a binding previously written by [`Self::to_json`].
    pub fn from_json(json: &str) -> TeeResult<Self> {
        Ok(serde_json::from_str(json)?)
    }
}

fn update_with_len(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quote_meta::{TdReportKind, TdxQuoteMetadata};

    fn meta_with(
        mr_td: [u8; 48],
        rtmrs: [[u8; 48]; 4],
        report_data: [u8; 64],
    ) -> TdxQuoteMetadata {
        TdxQuoteMetadata {
            quote_version: 4,
            tee_type: crate::quote_meta::TEE_TYPE_TDX,
            report_kind: TdReportKind::Td10,
            tee_tcb_svn: [0; 16],
            mr_td: mr_td.into(),
            mr_config_id: [0x10; 48].into(),
            mr_owner: [0x11; 48].into(),
            mr_owner_config: [0x12; 48].into(),
            rtmrs: rtmrs.map(Digest48::from),
            report_data: report_data.into(),
            mr_service_td: None,
        }
    }

    fn baseline() -> ProvenanceBinding {
        ProvenanceBinding::from_quote_meta(
            "gdid:arkhe:test-agent",
            ProvenanceBinding::hash_binary(b"agent-binary-v1"),
            &meta_with([0xaa; 48], [[0x01; 48], [0x02; 48], [0x03; 48], [0x04; 48]], [0x55; 64]),
            1_700_000_000_000_000,
        )
        .unwrap()
    }

    #[test]
    fn json_round_trip_preserves_every_field() {
        let binding = baseline();
        let json = binding.to_json().unwrap();
        let back = ProvenanceBinding::from_json(&json).unwrap();
        assert_eq!(back, binding);
        assert_eq!(back.digest(), binding.digest());
        assert!(json.contains("gdid:arkhe:test-agent"));
    }

    #[test]
    fn an_empty_gdid_is_refused() {
        assert!(ProvenanceBinding::new(
            "   ",
            Digest32::ZERO,
            4,
            Digest48::ZERO,
            [Digest48::ZERO; 4],
            ReportData64::ZERO,
            0,
        )
        .is_err());
    }

    #[test]
    fn matching_metadata_verifies() {
        let binding = baseline();
        let meta = meta_with([0xaa; 48], [[0x01; 48], [0x02; 48], [0x03; 48], [0x04; 48]], [0x55; 64]);
        assert!(binding.verify_against(&meta).is_ok());
    }

    #[test]
    fn each_measured_field_is_independently_pinned() {
        let binding = baseline();

        let mutated_mr_td = meta_with([0xbb; 48], [[0x01; 48], [0x02; 48], [0x03; 48], [0x04; 48]], [0x55; 64]);
        match binding.verify_against(&mutated_mr_td) {
            Err(TeeError::BindingMismatch { field, .. }) => assert_eq!(field, "mr_td"),
            other => panic!("expected mr_td mismatch, got {other:?}"),
        }

        let mutated_rtmr2 = meta_with([0xaa; 48], [[0x01; 48], [0x02; 48], [0x03; 48], [0x99; 48]], [0x55; 64]);
        match binding.verify_against(&mutated_rtmr2) {
            Err(TeeError::BindingMismatch { field, .. }) => assert_eq!(field, "rtmr3"),
            other => panic!("expected rtmr3 mismatch, got {other:?}"),
        }

        let mutated_report_data = meta_with([0xaa; 48], [[0x01; 48], [0x02; 48], [0x03; 48], [0x04; 48]], [0x56; 64]);
        match binding.verify_against(&mutated_report_data) {
            Err(TeeError::BindingMismatch { field, .. }) => assert_eq!(field, "report_data"),
            other => panic!("expected report_data mismatch, got {other:?}"),
        }
    }

    #[test]
    fn digest_is_stable_and_sensitive() {
        let binding = baseline();
        assert_eq!(binding.digest(), binding.digest());

        let mut moved = baseline();
        moved.bound_at_unix_us += 1;
        assert_ne!(binding.digest(), moved.digest());

        let mut other_binary = baseline();
        other_binary.binary_hash = ProvenanceBinding::hash_binary(b"agent-binary-v2");
        assert_ne!(binding.digest(), other_binary.digest());
    }

    #[test]
    fn length_prefixing_prevents_concatenation_collisions() {
        // "ab" + "c" must not hash the same as "a" + "bc".
        let mut a = baseline();
        a.gdid = "ab".into();
        let mut b = baseline();
        b.gdid = "a".into();
        a.binary_hash = ProvenanceBinding::hash_binary(b"c");
        b.binary_hash = ProvenanceBinding::hash_binary(b"bc");
        assert_ne!(a.digest(), b.digest());
    }

    #[test]
    fn predicted_rtmr_matches_the_extension_law() {
        let binding = baseline();
        let artifact = ProvenanceBinding::hash_binary(b"artifact");
        let predicted = binding.rtmr_after_agent_artifact_extension(1, &artifact).unwrap();
        let expected = extend_digest(
            &[0x02; 48],
            &agent_artifact_digest(artifact.as_bytes()),
        );
        assert!(predicted == expected);
        assert!(binding
            .rtmr_after_agent_artifact_extension(9, &artifact)
            .is_none());
    }
}
