use crate::ArkheHash;

/// Computa hash BLAKE3 dos dados.
pub fn blake3_hash(data: &[u8]) -> ArkheHash {
    blake3::hash(data).into()
}

/// Converte hash para representação hex.
pub fn hash_to_hex(hash: &ArkheHash) -> String {
    hex::encode(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blake3_is_deterministic_and_content_sensitive() {
        let a = blake3_hash(b"arkhe");
        let b = blake3_hash(b"arkhe");
        let c = blake3_hash(b"arkhe!");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn hex_encoding_is_64_chars() {
        let h = blake3_hash(b"payload");
        let hex = hash_to_hex(&h);
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|ch| ch.is_ascii_hexdigit()));
    }
}
