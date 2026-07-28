use core::fmt;

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[derive(Debug)]
pub enum IdentityError {
    Bip39(String),
    Derivation(String),
    Pqc(String),
    DidResolution(String),
    Flock(String),
    Qdrant(String),
    InvalidSeed { expected: usize, got: usize },
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bip39(m) => write!(f, "BIP39 error: {m}"),
            Self::Derivation(m) => write!(f, "derivation error: {m}"),
            Self::Pqc(m) => write!(f, "PQC error: {m}"),
            Self::DidResolution(m) => write!(f, "DID resolution: {m}"),
            Self::Flock(m) => write!(f, "flock error: {m}"),
            Self::Qdrant(m) => write!(f, "Qdrant error: {m}"),
            Self::InvalidSeed { expected, got } => {
                write!(f, "invalid seed length: expected {expected}, got {got}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for IdentityError {}

#[cfg(feature = "std")]
impl From<bip39::Error> for IdentityError {
    fn from(e: bip39::Error) -> Self {
        IdentityError::Bip39(e.to_string())
    }
}