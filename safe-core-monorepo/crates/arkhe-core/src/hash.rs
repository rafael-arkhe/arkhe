//! Hashing BLAKE3 de conteúdo.
//!
//! [`blake3_hash`] produz os 32 bytes que [`ArkheHash`] tipa; [`hash_to_hex`]
//! é a forma textual — 64 caracteres hexadecimais — usada onde o hash tem de
//! ser chave, nome ou identificador legível (por exemplo o id de nó de
//! `arkhe-geometric-verifier/memory_graph.rs`).
//!
//! As crates consumidoras passam por aqui em vez de dependerem do `blake3`
//! diretamente (ver `arkhe-geometric-verifier/README.md`), para que haja **uma
//! só versão do hash em jogo**: um `blake3` próprio numa crate poderia subir de
//! versão sem que as outras dessem por isso — e dois hashes diferentes do mesmo
//! conteúdo são precisamente o que uma cadeia de evidência não pode ter.

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
