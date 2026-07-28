use bip39::{Language, Mnemonic};
use zeroize::{Zeroize, ZeroizeOnDrop};
use crate::error::IdentityError;

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct ArkheMnemonic {
    phrase: String,
}

impl ArkheMnemonic {
    pub fn generate() -> Result<Self, IdentityError> {
        let m = Mnemonic::generate_in(Language::English, 24)?;
        Ok(Self { phrase: m.to_string() })
    }

    pub fn from_phrase(phrase: &str) -> Result<Self, IdentityError> {
        Mnemonic::parse(phrase)?;
        Ok(Self { phrase: phrase.to_string() })
    }

    pub fn to_seed(&self, passphrase: &str) -> ArkheSeed {
        let m = Mnemonic::parse(&self.phrase).expect("phrase validated");
        ArkheSeed { bytes: m.to_seed(passphrase) }
    }

    pub fn phrase(&self) -> &str { &self.phrase }
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct ArkheSeed {
    bytes: [u8; 64],
}

impl ArkheSeed {
    pub fn as_bytes(&self) -> &[u8; 64] { &self.bytes }
}

pub fn mnemonic_to_master(m: &ArkheMnemonic, passphrase: &str, agent_id: &str)
    -> Result<crate::types::MasterIdentity, IdentityError>
{
    let seed = m.to_seed(passphrase);
    crate::derivation::derive_master_identity(seed.as_bytes(), agent_id)
}