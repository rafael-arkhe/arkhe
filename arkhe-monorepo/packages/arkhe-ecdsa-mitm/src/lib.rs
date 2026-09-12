//! ARKHE ECDSA·MITM — analogia de rotação de fase (módulo #25).
//!
//! Na física de neutrinos, a fase CP δ da matriz de mistura PMNS *rotaciona* os
//! estados de sabor por uma fase de U(1): grandezas **sensíveis à fase**
//! (invariante de Jarlskog J) mudam com δ, enquanto a probabilidade de
//! sobrevivência de reator `P(ν̄e→ν̄e)` (como medida por JUNO) é **cego à fase**.
//!
//! Este crate transporta a mesma estrutura para o setor de assinaturas ECDSA:
//!
//! - `PhaseRotation` (derivada da fase δ da matriz de camadas) *rotaciona* o
//!   digest pré-hash da assinatura — um blend multiplicativo mod 2²⁵⁶ (grupo U(1)
//!   numérico);
//! - assinar com a rotação correta e **verificar com a mesma rotação** é
//!   determinístico e válido (a chave recuperada confere);
//! - um **mitm** que troca a rotação esperada por outra (λ′ ≠ λ) faz a verificação
//!   **falhar** — a rotação de fase é *detectável*, exatamente como `J ≠ 0`.
//!
//! O teste de integração em `arkhe-field/tests/phase_rotation.rs` amarra o
//! crate-irmão `arkhe-field` (matriz de mistura) a este (rotação ECDSA).

pub mod ecdsa;
pub mod phase;

pub use ecdsa::{
    MitmError, PhaseAlignedSignature, intercept_phase, sign_phase, verify_phase,
};
pub use phase::{PhaseRotation, blend_rotation, mul_mod_2_256};