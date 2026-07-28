use crate::error::IdentityError;
use crate::types::{DidDocument, MasterIdentity, VerificationMethod};

pub fn build_did(fingerprint: &str) -> String {
    format!("did:arkhe:{}", fingerprint)
}

fn now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    format_rfc3339(secs)
}

/// Format a UTC unix timestamp (seconds) as an RFC3339 instant.
fn format_rfc3339(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let rem = unix_secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Convert days-since-unix-epoch to a proleptic-Gregorian (year, month, day).
/// Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn generate_did_document(
    identity: &MasterIdentity,
    ml_dsa_65_pk: &[u8],
) -> DidDocument {
    let did = build_did(&identity.fingerprint);
    let now = now_rfc3339();

    DidDocument {
        context: vec![
            "https://www.w3.org/ns/did/v1".into(),
            "https://arkhe.io/ns/did/v1".into(),
        ],
        id: did.clone(),
        verification_method: vec![VerificationMethod {
            id: format!("{}#keys-1", did),
            type_: "ML-DSA-65-2024".into(),
            controller: did.clone(),
            public_key_multibase: format!("z{}", bs58::encode(ml_dsa_65_pk).into_string()),
        }],
        authentication: vec![format!("{}#keys-1", did)],
        assertion_method: vec![format!("{}#keys-1", did)],
        created: now.clone(),
        updated: now,
    }
}

pub fn resolve_did(did: &str) -> Result<DidDocument, IdentityError> {
    if !did.starts_with("did:arkhe:") {
        return Err(IdentityError::DidResolution("invalid scheme".into()));
    }
    Err(IdentityError::DidResolution(
        "offline — use qdrant-resolver feature or query TOON chain directly".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::format_rfc3339;

    #[test]
    fn rfc3339_known_instants() {
        assert_eq!(format_rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_rfc3339(1_609_459_200), "2021-01-01T00:00:00Z");
        // 2021-01-01T00:00:00Z + 1h1m1s
        assert_eq!(format_rfc3339(1_609_459_200 + 3661), "2021-01-01T01:01:01Z");
        // A leap day: 2024-02-29T12:00:00Z
        assert_eq!(format_rfc3339(1_709_208_000), "2024-02-29T12:00:00Z");
    }
}