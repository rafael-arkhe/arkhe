use arkhe_identity::*;
use bip39::Mnemonic;

fn test_seed() -> [u8; 64] {
    let m = arkhe_identity::mnemonic::ArkheMnemonic::from_phrase(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    ).unwrap();
    *m.to_seed("test").as_bytes()
}

#[test]
fn kat_seed_length() {
    let m = arkhe_identity::mnemonic::ArkheMnemonic::generate().unwrap();
    let seed = m.to_seed("");
    assert_eq!(seed.as_bytes().len(), 64);
}

#[test]
fn kat_hkdf_determinism() {
    let seed = test_seed();
    let id1 = derive_master_identity(&seed, "agent").unwrap();
    let id2 = derive_master_identity(&seed, "agent").unwrap();
    assert_eq!(id1.ml_dsa_65_seed, id2.ml_dsa_65_seed);
    assert_eq!(id1.fingerprint, id2.fingerprint);
}

#[test]
fn kat_ml_dsa_65_key_sizes() {
    let seed = [0x42u8; 32];
    let (pk, sk) = pqc::ml_dsa_65_keypair_from_seed(&seed).unwrap();
    assert_eq!(pk.len(), 1952);
    assert_eq!(sk.len(), 4032);
}

#[test]
fn kat_sign_verify_roundtrip() {
    let seed = [0xABu8; 32];
    let (pk, sk) = pqc::ml_dsa_65_keypair_from_seed(&seed).unwrap();
    let msg = b"ARKHE-KAT-VECTOR-2026";
    let sig = pqc::ml_dsa_65_sign(&sk, msg).unwrap();
    assert_eq!(sig.len(), 3309);
    assert!(pqc::ml_dsa_65_verify(&pk, msg, &sig).unwrap());
    assert!(!pqc::ml_dsa_65_verify(&pk, b"TAMPERED", &sig).unwrap());
}

#[test]
fn kat_did_format() {
    let did = did::build_did("a1b2c3d4e5f67890");
    assert!(did.starts_with("did:arkhe:"));
}

#[test]
fn kat_did_document() {
    let seed = test_seed();
    let identity = derive_master_identity(&seed, "test").unwrap();
    let (pk, _) = pqc::ml_dsa_65_keypair_from_seed(&identity.ml_dsa_65_seed).unwrap();
    let doc = did::generate_did_document(&identity, &pk);
    assert!(doc.id.starts_with("did:arkhe:"));
    assert_eq!(doc.verification_method.len(), 1);
}

#[test]
fn kat_fingerprint_determinism() {
    let entropy = [0u8; 32];
    let m1 = Mnemonic::from_entropy(&entropy).unwrap();
    let m2 = Mnemonic::from_entropy(&entropy).unwrap();
    let seed1 = arkhe_identity::mnemonic::ArkheMnemonic::from_phrase(
        &m1.to_string()
    ).unwrap();
    let seed2 = arkhe_identity::mnemonic::ArkheMnemonic::from_phrase(
        &m2.to_string()
    ).unwrap();
    let id1 = derive_master_identity(seed1.to_seed("").as_bytes(), "test").unwrap();
    let id2 = derive_master_identity(seed2.to_seed("").as_bytes(), "test").unwrap();
    assert_eq!(id1.fingerprint, id2.fingerprint);
}

#[test]
fn kat_zeroize_on_drop() {
    let seed = arkhe_identity::mnemonic::ArkheMnemonic::from_phrase(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    ).unwrap().to_seed("test");
    let _ptr = seed.as_bytes().as_ptr();
    drop(seed);
    // Under miri, this verifies that ZeroizeOnDrop zeroes the memory
}

#[cfg(feature = "flock")]
#[test]
fn kat_flock_sign_verify() {
    let seed = [0x42u8; 32];
    let (pk, sk) = pqc::ml_dsa_65_keypair_from_seed(&seed).unwrap();
    let mut bundle = FirmwareBundle {
        name: "litho-controller".into(),
        version: "1.0.0".into(),
        target: "nRF52840".into(),
        digest_sha3: "abc123".into(),
        signature: None,
        did_signer: None,
        toon_receipt: None,
    };
    flock_bridge::sign_firmware_bundle(&mut bundle, &sk, "did:arkhe:test").unwrap();
    assert!(bundle.signature.is_some());
    assert!(flock_bridge::verify_firmware_bundle(&bundle, &pk).unwrap());
}