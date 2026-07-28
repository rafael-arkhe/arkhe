#[cfg(not(feature = "std"))]
use alloc::string::String;

use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Zeroize, ZeroizeOnDrop)]
pub struct MasterIdentity {
    pub ml_dsa_65_seed: [u8; 32],
    pub agent_id: String,
    pub fingerprint: String,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DidDocument {
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    pub id: String,
    pub verification_method: Vec<VerificationMethod>,
    pub authentication: Vec<String>,
    pub assertion_method: Vec<String>,
    pub created: String,
    pub updated: String,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerificationMethod {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub controller: String,
    pub public_key_multibase: String,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FirmwareBundle {
    pub name: String,
    pub version: String,
    pub target: String,
    pub digest_sha3: String,
    pub signature: Option<String>,
    pub did_signer: Option<String>,
    pub toon_receipt: Option<String>,
}