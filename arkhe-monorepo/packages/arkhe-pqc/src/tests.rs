//! Unit tests: ML-DSA signatures, ML-KEM, and the authenticated layer.

use crate::auth_kem::{auth_decapsulate, auth_encapsulate, PqcKeyMaterial};
use crate::handshake::{
    confirm_ss, initiator_finalize_ss, initiator_hello, responder_ack, verify_confirm,
};
use crate::kem::{generate_kem_keypair, quantum_decapsulate, quantum_encapsulate};
use crate::sign::{quantum_sign, quantum_verify, quantum_verify_sig};
use crate::wallet::{identity_fingerprint, PqcWallet};

#[test]
fn mldsa_sign_verify_roundtrip() {
    let msg = b"loop-closure finality at height 42";
    let sig = quantum_sign(msg);
    assert!(!sig.signature.is_empty());
    assert!(!sig.public.is_empty());
    quantum_verify_sig(&sig).expect("valid ML-DSA signature must verify");
}

#[test]
fn mldsa_rejects_tampered_message() {
    let sig = quantum_sign(b"commitment A");
    let forged = b"commitment B";
    assert!(quantum_verify(&sig.public, forged, &sig.signature).is_err());
}

#[test]
fn mldsa_detects_corrupted_signature() {
    let sig = quantum_sign(b"membrane attests this");
    let mut bad = sig.signature.clone();
    let last = bad.len() - 1;
    bad[last] ^= 0xff;
    let r = quantum_verify(&sig.public, &sig.message, &bad);
    assert!(r.is_err(), "corrupted signature must not verify");
}

#[test]
fn mlkem_shared_secret_equality() {
    let kp = generate_kem_keypair();
    let (ct, ss_a) = quantum_encapsulate(&kp.encapsulation).expect("encapsulate");
    let ss_b = quantum_decapsulate(&kp.decapsulation, &ct).expect("decapsulate");
    assert_eq!(ss_a, ss_b, "both parties must derive the same shared secret");
    assert_eq!(ss_a.len(), 32, "ML-KEM-768 shared secret is 32 bytes");
}

#[test]
fn mlkem_tampered_ciphertext_derives_different_secret() {
    let kp = generate_kem_keypair();
    let (mut ct, ss_a) = quantum_encapsulate(&kp.encapsulation).expect("encapsulate");
    let n = ct.len();
    ct[n - 1] ^= 0x01;
    let ss_b = quantum_decapsulate(&kp.decapsulation, &ct).expect("decapsulate");
    assert_ne!(ss_a, ss_b, "a perturbed capsule must not reproduce the secret");
}

#[test]
fn mlkem_distinct_keypair_gives_distinct_secrets() {
    let a = generate_kem_keypair();
    let b = generate_kem_keypair();
    assert_ne!(a.encapsulation, b.encapsulation, "fresh keypairs must differ");
}

#[test]
fn auth_kem_roundtrip_derives_matching_secret() {
    let alice = PqcKeyMaterial::generate();
    let bob = PqcKeyMaterial::generate();
    let label = b"arkhe:channel-a";
    let (auth, ss_alice) = auth_encapsulate(&alice.signer, &bob.identity, label).expect("encap");
    let ss_bob =
        auth_decapsulate(&alice.identity, &bob.kem.decapsulation, &auth).expect("decap");
    assert_eq!(ss_alice, ss_bob, "authenticated KEM must agree on the secret");
}

#[test]
fn auth_kem_rejects_unknown_sender() {
    let alice = PqcKeyMaterial::generate();
    let bob = PqcKeyMaterial::generate();
    let mallory = PqcKeyMaterial::generate();
    let (auth, _ss) =
        auth_encapsulate(&mallory.signer, &bob.identity, b"arkhe:chan").expect("encap");
    // Bob expects Alice, but the capsule came from Mallory.
    let r = auth_decapsulate(&alice.identity, &bob.kem.decapsulation, &auth);
    assert!(r.is_err(), "mismatched sender identity must be rejected");
}

#[test]
fn wallet_rotation_changes_active_identity() {
    let mut w = PqcWallet::new();
    let first = w.active_identity().clone();
    w.rotate();
    assert_ne!(first, *w.active_identity(), "rotation must mint a new key");
    assert!(!w.is_active(&first), "the rotated key is no longer active");
    assert!(w.is_active(w.active_identity()), "the new key is active");
    assert_eq!(w.history.len(), 2, "two entries after one rotation");
}

#[test]
fn wallet_revote_removes_active_status() {
    let mut w = PqcWallet::new();
    let cur = w.active_identity().clone();
    w.revoke_active();
    assert!(!w.is_active(&cur), "revoked key must not be active");
    assert_eq!(identity_fingerprint(&cur), identity_fingerprint(&cur));
}

#[test]
fn three_way_handshake_converges_on_shared_secret() {
    let initiator = PqcKeyMaterial::generate();
    let responder = PqcKeyMaterial::generate();
    let hello = initiator_hello(&initiator, 1);
    let (resp, ss_responder) = responder_ack(&responder, &hello).expect("ack");
    let ss_initiator =
        initiator_finalize_ss(&initiator, &hello, &resp).expect("finalize");
    assert_eq!(ss_initiator, ss_responder, "3WH shared secret must converge");

    let c = confirm_ss(&ss_initiator);
    assert!(verify_confirm(&ss_responder, &c), "confirm digest must be accepted");
    assert!(!verify_confirm(b"wrong-secret", &c), "wrong secret must not confirm");
}
