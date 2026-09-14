//! Teste do gate BIP-322 (stub) — Fix #9: imports dentro da função de teste.

// Fix #9: all imports live inside the test function. Previously
// `use bitcoin::Network;` at module top was orphaned when the
// feature was enabled (the only test using it was cfg'd out).

#[cfg(not(feature = "bip322"))]
#[test]
fn verify_bip322_returns_feature_disabled() {
    use arkhe_crypto::bip322::verify_bip322;
    use arkhe_crypto::error::CryptoError;
    use bitcoin::Network;

    let r = verify_bip322("msg", "bc1q...", "sig", "tr(...)", Network::Bitcoin);
    assert!(matches!(r, Err(CryptoError::FeatureDisabled)));
}