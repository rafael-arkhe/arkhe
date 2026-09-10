//! arkhe-aid — Agent Identity & Delegation (bloco 1074, VAIP).
//!
//! Realizacao de sanidade sobre substrato verificado da ontologia AID,
//! ancorada no IETF Internet-Draft `draft-nyantakyi-vaip-agent-identity-01`
//! (Vorim Agent Identity Protocol, abril 2026 — Ed25519, SHA-256, sete
//! escopos hierarquicos, trust score 0-100, audit trail assinado na fonte).
//!
//! Escopo deste bloco (honestidade de escopo — nada alem do que e provado):
//!   - AID-001  Identidade criptografica (keypair Ed25519, fingerprint SHA-256)
//!   - AID-002  Delegacao atenuada (escopos delegados ⊆ escopos do delegador)
//!   - AID-003  Evento de auditoria assinado na fonte + hash chain (append-only)
//!   - AID-010  Segregacao de funcoes (uma etapa por agente)
//!
//! NAO implementados neste bloco (declarados fora do escopo, sem substrato
//! fabricado): AID-004 revogacao distribuida, AID-005 trust score
//! (formula local nao mandatoria pelo VAIP), AID-006 did:key resolvido W3C
//! completo (apenas derivacao do identificador), AID-007 RFC 8785 completa,
//! AID-008 pre-action verdict, AID-009 token budget por janela.
//!
//! Canonicalizacao (AID-007 parcial): serializacao deterministica campo-a-campo
//! com ordem FIXA e representacao canonica de cada tipo — reutilizacao do
//! serializer estrutural (serde) sem introduzir `serde_json_canonicalizer`
//! (dep externa nova; Simplicity-2). A forma canonica e documentada em
//! `canonical_*` de cada modulo.
//!
//! Referencia: https://datatracker.ietf.org/doc/html/draft-nyantakyi-vaip-agent-identity-01
//! Nucleo Lean core: src/lean/AgentIdentityNucleus.lean (sem Mathlib, sem sorry).

mod audit;
mod delegation;
mod identity;
mod sod;

pub use audit::{SignedAuditEvent, AUDIT_FIELD_SEP};
pub use delegation::{Delegation, Scope, ScopeParseError};
pub use identity::{AgentIdentity, Fingerprint, KEY_PKCS8_PREFIX};
pub use sod::{OperationStep, SensitiveOperation, SodError};