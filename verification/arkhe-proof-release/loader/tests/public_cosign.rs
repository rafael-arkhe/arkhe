//! An upstream public-domain-of-trust Cosign blob fixture, separate from local WASM.
//! This proves native 0.6.6 blob interoperability, not a public Arkhe WASM release.
use sigstore_trust_root::TrustedRoot;
use sigstore_types::Bundle;
use sigstore_verify::{VerificationPolicy, VerificationResult, Verifier};

const IDENTITY: &str = "w.vollprecht@gmail.com";
const ISSUER: &str = "https://github.com/login/oauth";

fn verify_constraints(identity: &str, issuer: &str) -> sigstore_verify::Result<VerificationResult> {
    let artifact = include_bytes!("../fixtures/public-cosign/artifact.txt");
    let bundle = Bundle::from_json(include_str!("../fixtures/public-cosign/bundle.json")).unwrap();
    let root = TrustedRoot::from_json(include_str!("../fixtures/public-cosign/trusted_root.json"))
        .unwrap();
    let policy = VerificationPolicy {
        identity: Some(identity.into()),
        issuer: Some(issuer.into()),
        verify_tlog: true,
        verify_timestamp: true,
        verify_certificate: true,
        clock_skew_seconds: 60,
    };
    Verifier::new(&root).verify(artifact.as_slice(), &bundle, &policy)
}

#[test]
fn wildcard_identity_is_not_authorized() {
    verify_constraints(IDENTITY, ISSUER).expect("public fixture precondition");
    let result = verify_constraints("w.*@gmail.com", ISSUER);
    assert!(
        result.is_err(),
        "wildcard identity must not authorize: {result:?}"
    );
}

#[test]
fn wrong_issuer_is_not_authorized() {
    verify_constraints(IDENTITY, ISSUER).expect("public fixture precondition");
    let result = verify_constraints(IDENTITY, "https://wrong-issuer.test.invalid");
    assert!(
        result.is_err(),
        "wrong issuer must not authorize: {result:?}"
    );
}

#[test]
fn upstream_public_cosign_blob_verifies_with_exact_identity() {
    let artifact = include_bytes!("../fixtures/public-cosign/artifact.txt");
    let bundle = Bundle::from_json(include_str!("../fixtures/public-cosign/bundle.json")).unwrap();
    let root = TrustedRoot::from_json(include_str!("../fixtures/public-cosign/trusted_root.json"))
        .unwrap();
    let policy = VerificationPolicy {
        identity: Some("w.vollprecht@gmail.com".into()),
        issuer: Some("https://github.com/login/oauth".into()),
        verify_tlog: true,
        verify_timestamp: true,
        verify_certificate: true,
        clock_skew_seconds: 60,
    };
    let report = Verifier::new(&root)
        .verify(artifact.as_slice(), &bundle, &policy)
        .unwrap();
    assert!(report.success);
    assert!(report.warnings.is_empty());
    assert_eq!(report.identity.as_deref(), policy.identity.as_deref());
    assert!(report.integrated_time.is_some());
}
