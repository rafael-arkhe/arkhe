use crate::error::IdentityError;
use crate::types::FirmwareBundle;

pub fn sign_firmware_bundle(
    bundle: &mut FirmwareBundle,
    sk: &[u8],
    did: &str,
) -> Result<(), IdentityError> {
    let msg = format!("{}:{}:{}:{}", bundle.name, bundle.version, bundle.target, bundle.digest_sha3);
    let signature = crate::derivation::pqc::ml_dsa_65_sign(sk, msg.as_bytes())?;
    bundle.signature = Some(hex::encode(&signature));
    bundle.did_signer = Some(did.to_string());
    Ok(())
}

pub fn verify_firmware_bundle(
    bundle: &FirmwareBundle,
    pk: &[u8],
) -> Result<bool, IdentityError> {
    let sig_hex = bundle.signature.as_ref()
        .ok_or_else(|| IdentityError::Flock("no signature".into()))?;
    let signature = hex::decode(sig_hex)
        .map_err(|e| IdentityError::Flock(format!("hex decode: {}", e)))?;
    let msg = format!("{}:{}:{}:{}", bundle.name, bundle.version, bundle.target, bundle.digest_sha3);
    crate::derivation::pqc::ml_dsa_65_verify(pk, msg.as_bytes(), &signature)
}