use spectral_core::ml_dsa_guard::ml_dsa_65_offsets;

#[test]
fn test_ml_dsa_65_layout() {
    assert_eq!(ml_dsa_65_offsets::CTILDE_BYTES, 48);
    assert_eq!(ml_dsa_65_offsets::Z_BYTES, 3200);
    assert_eq!(ml_dsa_65_offsets::H_BYTES, 61);
    assert_eq!(ml_dsa_65_offsets::H_OFFSET, 3248);
    assert_eq!(ml_dsa_65_offsets::SIGNATURE_BYTES, 3309);
}