//! `arkhe-verify-wasm` — §2.1–2.3 do plano Arkhe OS: **verificação
//! client-side que carrega no navegador**.
//!
//! Este crate compila para `wasm32-unknown-unknown` e expõe, via
//! `wasm-bindgen`, a API que o navegador usa para conferir uma atestação
//! **sem confiar no servidor que a produziu**:
//!
//! | Função | O que confere |
//! |:---|:---|
//! | [`wasm::verify_sha256`] | o digest SHA-256 de um payload |
//! | [`wasm::verify_signature`] | uma assinatura Ed25519 contra um trust root |
//! | [`wasm::verify_inclusion`] | uma prova de inclusão Merkle RFC 6962 |
//! | [`wasm::verify_witness_quorum`] | quórum de testemunhas (mínimo 2) |
//! | [`wasm::verify_attestation`] | o pipeline completo, compondo as quatro |
//!
//! Mais três auxiliares que a página precisa para integrar: [`wasm::sha256_hex`],
//! [`wasm::merkle_root_hex`] e [`wasm::attestation_subject_hex`].
//!
//! # Duas superfícies
//!
//! A lógica vive nos módulos [`hash`], [`signature`], [`merkle`], [`quorum`] e
//! [`attestation`], que são Rust normal e **testável no host**. O módulo
//! [`wasm`] é uma casca fina: traduz `String`/`&[u8]`/`u32` para essas
//! funções e devolve `bool` ou uma `String` JSON. Essa separação é o motivo
//! pelo qual o crate declara `crate-type = ["cdylib", "rlib"]` — o `cdylib` é
//! o artefato do navegador, e o `rlib` permite que os testes rodem nativos.
//!
//! # Tratamento de erro
//!
//! Nenhuma função exposta ao JavaScript lança. Entrada malformada devolve
//! `false`, ou um relatório JSON com `ok: false` e a causa. Os caminhos de
//! biblioteca não usam `panic!`, `unwrap()` nem `expect()` — as falhas
//! previsíveis são valores de [`VerifyError`].
//!
//! # Sobre `std`
//!
//! O crate usa `std`. Isto é necessário, não incidental: `wasm-bindgen` exige
//! `std`, e o alvo `wasm32-unknown-unknown` o fornece. O alvo **não**
//! suportado é `wasm32-wasi` — ver o README para o porquê e para a rota
//! opcional do `sigstore`.
//!
//! # `unsafe`
//!
//! `#![deny(unsafe_code)]` está ativo. Nenhum `unsafe` é escrito à mão neste
//! crate.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod attestation;
pub mod encoding;
pub mod error;
pub mod hash;
pub mod merkle;
pub mod quorum;
pub mod signature;
pub mod wasm;

pub use attestation::{Attestation, AttestationReport, ATTESTATION_DOMAIN};
pub use encoding::{TrustRoot, WitnessJson};
pub use error::VerifyError;
pub use hash::{sha256_bytes, sha256_hex, verify_sha256_inner, SHA256_LEN};
pub use merkle::{merkle_root_from_leaves, verify_inclusion_inner};
pub use quorum::{count_valid_witnesses, verify_witness_quorum_inner, QUORUM_MIN};
pub use signature::{verify_signature_inner, verify_with_key};
