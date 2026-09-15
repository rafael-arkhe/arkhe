//! A fixture `arkhe.gguf`: 24 bytes, determinística, sem tensores e sem pares
//! chave-valor.
//!
//! Estes testes leem `fixtures/arkhe.gguf` (caminho relativo à raiz do crate,
//! que é o diretório de trabalho de um teste de integração) e fixam quatro
//! factos sobre esses bytes: o magic, a versão, as duas contagens e o tamanho.
//!
//! Os quatro primeiros testes leem os campos **à mão**, byte a byte, de
//! propósito: se lessem por `arkhe_verify::gguf::parse_header`, uma regressão
//! no leitor deixaria os dois lados errados da mesma maneira e os testes
//! continuariam verdes. O último teste é que liga a fixture ao código do
//! projeto — e o digest que ele afirma foi **calculado** por esse mesmo código
//! sobre estes bytes, não copiado de outro sítio:
//!
//! ```text
//! cargo run -p arkhe-verify --example conferir_gguf -- crates/arkhe-verify/fixtures/arkhe.gguf
//! ```
//!
//! O valor coincide também com `sha256sum` e com `hashlib.sha256` (Python) —
//! três implementações independentes sobre os mesmos 24 bytes.

/// Os 24 bytes da fixture: `GGUF`, versão 3 (u32 LE), 0 tensores (u64 LE),
/// 0 pares chave-valor (u64 LE).
const FIXTURE_HEX: &str = "474755460300000000000000000000000000000000000000";

/// O digest que `arkhe_verify::gguf::model_digest` devolve para a fixture.
const FIXTURE_SHA256: &str = "a4e5e156ddec27e286f75328784d7106b60a4eb1d246e950a001a3f944fbda99";

#[test]
fn fixture_has_correct_magic() {
    let f = std::fs::read("fixtures/arkhe.gguf").unwrap();
    assert_eq!(&f[0..4], b"GGUF");
}

#[test]
fn fixture_has_version_3() {
    let f = std::fs::read("fixtures/arkhe.gguf").unwrap();
    let v = u32::from_le_bytes(f[4..8].try_into().unwrap());
    assert_eq!(v, 3);
}

#[test]
fn fixture_has_no_tensors() {
    let f = std::fs::read("fixtures/arkhe.gguf").unwrap();
    let n = u64::from_le_bytes(f[8..16].try_into().unwrap());
    assert_eq!(n, 0);
}

#[test]
fn fixture_has_no_kv_pairs() {
    let f = std::fs::read("fixtures/arkhe.gguf").unwrap();
    let n = u64::from_le_bytes(f[16..24].try_into().unwrap());
    assert_eq!(n, 0);
}

#[test]
fn fixture_is_exactly_24_bytes() {
    let f = std::fs::read("fixtures/arkhe.gguf").unwrap();
    assert_eq!(f.len(), 24);
}

#[test]
fn digest_of_fixture_is_stable() {
    let f = std::fs::read("fixtures/arkhe.gguf").unwrap();
    let digest = arkhe_verify::gguf::model_digest(&f);
    assert_eq!(digest, FIXTURE_SHA256);
}

/// A fixture em disco é exatamente o hex documentado — não só tem o mesmo
/// digest (que, em princípio, outras sequências de 24 bytes também teriam).
#[test]
fn fixture_bytes_are_the_documented_hex() {
    let f = std::fs::read("fixtures/arkhe.gguf").unwrap();
    let hex: String = f.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(hex, FIXTURE_HEX);
}
