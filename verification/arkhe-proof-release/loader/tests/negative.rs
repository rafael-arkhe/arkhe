use arkhe_verify_self_demo::{
    load_verified_module, validate_trust_root_age, verify_self, VerifyError, MAX_ROOT_AGE_SECONDS,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use wasmtime::Engine;

const A: &[u8] = include_bytes!("../fixtures/arkhe_verify.wasm");
const B: &[u8] = include_bytes!("../fixtures/other.wasm");
const BA: &[u8] = include_bytes!("../fixtures/arkhe_verify.wasm.bundle.json");
const BB: &[u8] = include_bytes!("../fixtures/other.wasm.bundle.json");
fn positive_preconditions() {
    verify_self(A, BA).expect("fixture A must verify before mutation");
    verify_self(B, BB).expect("fixture B must verify before mutation");
    assert_ne!(A, B);
}
fn changed_bundle(f: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut bundle: Value = serde_json::from_slice(BA).unwrap();
    f(&mut bundle);
    serde_json::to_vec(&bundle).unwrap()
}

#[test]
fn bundle_for_other_artifact_rejected() {
    positive_preconditions();
    assert!(matches!(
        verify_self(A, BB),
        Err(VerifyError::Verification(_))
    ));
}

#[test]
fn tampered_wasm_rejected_before_compile() {
    positive_preconditions();
    let mut tampered = A.to_vec();
    tampered[0] ^= 0xff; // Also invalid WASM: error must be HashMismatch, not Compilation.
    assert!(matches!(
        load_verified_module(&Engine::default(), &tampered, BA),
        Err(VerifyError::HashMismatch)
    ));
}

#[test]
fn unapproved_but_validly_signed_module_rejected_by_hash_pin() {
    positive_preconditions();
    // B is valid WASM, has a fully valid attestation and the same authorized
    // signer. Only the build-pinned artifact identity prevents its loading.
    assert!(matches!(
        load_verified_module(&Engine::default(), B, BB),
        Err(VerifyError::HashMismatch)
    ));
}

#[test]
fn tampered_valid_wasm_rejected_by_attestation() {
    positive_preconditions();
    let mut tampered = A.to_vec();
    let n = tampered.len();
    tampered[n - 2] = 41; // Valid module, changes answer from 42 to 41.
    assert!(matches!(
        verify_self(&tampered, BA),
        Err(VerifyError::Verification(_))
    ));
}

#[test]
fn unrelated_rekor_entry_rejected() {
    positive_preconditions();
    let other: Value = serde_json::from_slice(BB).unwrap();
    let mixed = changed_bundle(|b| {
        b["verificationMaterial"]["tlogEntries"][0] =
            other["verificationMaterial"]["tlogEntries"][0].clone()
    });
    // Full B entry has valid SET, checkpoint and Merkle proof; only its binding
    // to A is wrong. A's messageSignature and certificate remain untouched.
    match verify_self(A, &mixed) {
        Err(VerifyError::Verification(reason)) => {
            assert!(reason.contains("artifact hash mismatch"), "{reason}")
        }
        other => panic!("expected artifact binding rejection, got {other:?}"),
    }
}

#[test]
fn damaged_merkle_path_rejected() {
    positive_preconditions();
    let bad = changed_bundle(|b| {
        b["verificationMaterial"]["tlogEntries"][0]["inclusionProof"]["hashes"][0] =
            json!(STANDARD.encode([0u8; 32]))
    });
    assert!(matches!(
        verify_self(A, &bad),
        Err(VerifyError::MerkleProof)
    ));
}

#[test]
fn forged_set_rejected() {
    positive_preconditions();
    let bad = changed_bundle(|b| {
        let s = &mut b["verificationMaterial"]["tlogEntries"][0]["inclusionPromise"]
            ["signedEntryTimestamp"];
        let mut bytes = STANDARD.decode(s.as_str().unwrap()).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        *s = json!(STANDARD.encode(bytes));
    });
    match verify_self(A, &bad) {
        Err(VerifyError::Verification(reason)) => {
            assert!(reason.contains("SET verification failed"), "{reason}")
        }
        other => panic!("expected SET rejection, got {other:?}"),
    }
}

#[test]
fn forged_checkpoint_rejected() {
    positive_preconditions();
    let bad = changed_bundle(|b| {
        let v = &mut b["verificationMaterial"]["tlogEntries"][0]["inclusionProof"]["checkpoint"]
            ["envelope"];
        let text = v.as_str().unwrap();
        let (prefix, signature) = text.trim_end().rsplit_once(' ').unwrap();
        let mut bytes = STANDARD.decode(signature).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        *v = json!(format!("{prefix} {}\n", STANDARD.encode(bytes)));
    });
    assert!(matches!(
        verify_self(A, &bad),
        Err(VerifyError::Verification(_))
    ));
}

#[test]
fn checkpoint_tree_size_mismatch_rejected() {
    positive_preconditions();
    let bad = changed_bundle(|b| {
        b["verificationMaterial"]["tlogEntries"][0]["inclusionProof"]["treeSize"] = json!("3")
    });
    assert!(matches!(
        verify_self(A, &bad),
        Err(VerifyError::MerkleProof)
    ));
}

#[test]
fn trust_root_stale_rejected() {
    let captured = 1_790_000_000;
    assert!(matches!(
        validate_trust_root_age(captured, captured + MAX_ROOT_AGE_SECONDS + 1),
        Err(VerifyError::TrustRootStale)
    ));
}

#[test]
fn trust_root_freshness_boundary_and_future() {
    let captured = 1_790_000_000;
    assert!(validate_trust_root_age(captured, captured + MAX_ROOT_AGE_SECONDS).is_ok());
    assert!(matches!(
        validate_trust_root_age(captured, captured - 1),
        Err(VerifyError::TrustRootFuture)
    ));
}
