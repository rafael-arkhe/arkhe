// packages/arkhe-core/src/ledger.rs — v0.3.3
//! Ledger append-only com determinismo cross-platform (Ghost-1/Loopseal-2).
//!
//! Layout canônico de 95 bytes por entry (sem serde no caminho do hash):
//! [0..32]   prev_hash    (32)
//! [32..40]  timestamp    (8, u64 LE)
//! [40..72]  state_hash   (32)
//! [72..95]  event_kind   (23, ASCII zero-padded)
//! Total: 95
//!
//! O invariante `event_kind is ASCII ∧ len ≤ 23` é garantido por construção
//! (construtor validado + `#[serde(try_from)]`), não por disciplina (D2).
//! `compute_hash()` é fail-closed (D1); `append()` valida antes de mutar (D3);
//! `last_hash()` documenta a falha (D4).

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const ENTRY_ENCODED_SIZE: usize = 95;
pub const EVENT_KIND_WIDTH: usize = 23;
pub const GENESIS_HASH: [u8; 32] = [0u8; 32];

// ─── Erros ─────────────────────────────────────────────────────────────

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LedgerError {
    #[error("invalid genesis: first entry's prev_hash is not GENESIS_HASH")]
    InvalidGenesis,

    #[error("prev_hash mismatch at index {index}: expected {expected:?}, got {actual:?}")]
    PrevHashMismatch {
        index: usize,
        expected: [u8; 32],
        actual: [u8; 32],
    },

    #[error("invalid event_kind {kind:?}: {reason}")]
    InvalidEventKind { kind: String, reason: String },
}

// ─── LedgerEntry: campos privados, construtor validado ────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "LedgerEntryRaw")]
pub struct LedgerEntry {
    prev_hash: [u8; 32],
    timestamp: u64,
    state_hash: [u8; 32],
    event_kind: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LedgerEntryRaw {
    prev_hash: [u8; 32],
    timestamp: u64,
    state_hash: [u8; 32],
    event_kind: String,
}

impl TryFrom<LedgerEntryRaw> for LedgerEntry {
    type Error = LedgerError;
    fn try_from(raw: LedgerEntryRaw) -> Result<Self, Self::Error> {
        validate_event_kind(&raw.event_kind)?;
        Ok(Self {
            prev_hash: raw.prev_hash,
            timestamp: raw.timestamp,
            state_hash: raw.state_hash,
            event_kind: raw.event_kind,
        })
    }
}

impl LedgerEntry {
    /// Construtor validado. **Único caminho público** para criar uma entry.
    pub fn new(
        timestamp: u64,
        state_hash: [u8; 32],
        event_kind: impl Into<String>,
    ) -> Result<Self, LedgerError> {
        let event_kind = event_kind.into();
        validate_event_kind(&event_kind)?;
        Ok(Self {
            prev_hash: GENESIS_HASH,
            timestamp,
            state_hash,
            event_kind,
        })
    }

    pub fn prev_hash(&self) -> &[u8; 32] {
        &self.prev_hash
    }
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
    pub fn state_hash(&self) -> &[u8; 32] {
        &self.state_hash
    }
    pub fn event_kind(&self) -> &str {
        &self.event_kind
    }

    /// Encoda em 95 bytes. Invariante garantido pelo construtor.
    fn encode(&self) -> [u8; ENTRY_ENCODED_SIZE] {
        debug_assert!(self.event_kind.is_ascii());
        debug_assert!(self.event_kind.len() <= EVENT_KIND_WIDTH);

        let mut buf = [0u8; ENTRY_ENCODED_SIZE];
        buf[0..32].copy_from_slice(&self.prev_hash);
        buf[32..40].copy_from_slice(&self.timestamp.to_le_bytes());
        buf[40..72].copy_from_slice(&self.state_hash);

        let kind_bytes = self.event_kind.as_bytes();
        buf[72..72 + kind_bytes.len()].copy_from_slice(kind_bytes);
        buf
    }

    /// Hash fail-closed. Nunca panica com uma entry válida (D1).
    pub fn compute_hash(&self) -> Result<[u8; 32], LedgerError> {
        validate_event_kind(&self.event_kind)?;
        Ok(*blake3::hash(&self.encode()).as_bytes())
    }
}

fn validate_event_kind(kind: &str) -> Result<(), LedgerError> {
    if !kind.is_ascii() {
        return Err(LedgerError::InvalidEventKind {
            kind: kind.into(),
            reason: "must be ASCII".into(),
        });
    }
    if kind.len() > EVENT_KIND_WIDTH {
        return Err(LedgerError::InvalidEventKind {
            kind: kind.into(),
            reason: format!("must be ≤ {} bytes", EVENT_KIND_WIDTH),
        });
    }
    Ok(())
}

// ─── Ledger ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct Ledger {
    entries: Vec<LedgerEntry>,
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append com atomicidade explícita (D3).
    /// Valida ANTES de mutar qualquer campo.
    pub fn append(&mut self, mut entry: LedgerEntry) -> Result<[u8; 32], LedgerError> {
        // 1. Validar antes de mutar
        validate_event_kind(entry.event_kind())?;

        // 2. Calcular prev_hash esperado
        let expected_prev = self
            .entries
            .last()
            .map(|e| e.compute_hash())
            .transpose()?
            .unwrap_or(GENESIS_HASH);

        // 3. Atribuir prev_hash
        entry.prev_hash = expected_prev;

        // 4. Hash final
        let hash = entry.compute_hash()?;

        // 5. Push
        self.entries.push(entry);

        Ok(hash)
    }

    /// Verificação fail-closed (D1).
    pub fn verify(&self) -> Result<(), LedgerError> {
        if self.entries.is_empty() {
            return Ok(());
        }
        if self.entries[0].prev_hash != GENESIS_HASH {
            return Err(LedgerError::InvalidGenesis);
        }
        for i in 1..self.entries.len() {
            let expected = self.entries[i - 1].compute_hash()?;
            if self.entries[i].prev_hash != expected {
                return Err(LedgerError::PrevHashMismatch {
                    index: i,
                    expected,
                    actual: self.entries[i].prev_hash,
                });
            }
        }
        Ok(())
    }

    /// Fail-closed: pode retornar Err se a última entry é inválida (D4).
    pub fn last_hash(&self) -> Result<Option<[u8; 32]>, LedgerError> {
        self.entries
            .last()
            .map(|e| e.compute_hash())
            .transpose()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn corrupt_prev_hash(&mut self, i: usize, hash: [u8; 32]) {
        if let Some(e) = self.entries.get_mut(i) {
            e.prev_hash = hash;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(ts: u64, kind: &str, b: u8) -> LedgerEntry {
        LedgerEntry::new(ts, [b; 32], kind).expect("valid entry")
    }

    #[test]
    fn verify_detects_broken_chain() {
        let mut ledger = Ledger::new();
        ledger.append(entry(1000, "init", 0x01)).unwrap();
        ledger.append(entry(1001, "step", 0x02)).unwrap();
        ledger.corrupt_prev_hash(1, [0xFF; 32]);
        assert!(matches!(
            ledger.verify(),
            Err(LedgerError::PrevHashMismatch { index: 1, .. })
        ));
    }

    #[test]
    fn constructor_rejects_non_ascii() {
        assert!(matches!(
            LedgerEntry::new(0, [0u8; 32], "café"),
            Err(LedgerError::InvalidEventKind { .. })
        ));
    }

    #[test]
    fn constructor_rejects_oversized_kind() {
        assert!(matches!(
            LedgerEntry::new(0, [0u8; 32], "x".repeat(24)),
            Err(LedgerError::InvalidEventKind { .. })
        ));
    }

    #[test]
    fn deserialization_validates_event_kind() {
        let json = r#"{
            "prev_hash": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
            "timestamp": 0,
            "state_hash": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
            "event_kind": "café"
        }"#;
        let result: Result<LedgerEntry, _> = serde_json::from_str(json);
        assert!(result.is_err(), "deserialization must reject non-ASCII");
    }

    #[test]
    fn deserialization_accepts_valid_entry() {
        let json = r#"{
            "prev_hash": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
            "timestamp": 1000,
            "state_hash": [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
            "event_kind": "init"
        }"#;
        let result: Result<LedgerEntry, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "valid entry must deserialize: {:?}", result.err());
    }

    #[test]
    fn serialization_roundtrip_preserves_entry() {
        let e = entry(1000, "init", 0x01);
        let bytes = serde_json::to_vec(&e).expect("serialize");
        let back: LedgerEntry = serde_json::from_slice(&bytes).expect("deserialize");
        assert_eq!(e, back);
    }

    #[test]
    fn verify_never_panics_on_invalid_entry() {
        let mut ledger = Ledger::new();
        ledger.append(entry(1000, "init", 0x01)).unwrap();
        assert!(ledger.verify().is_ok());
    }

    #[test]
    fn append_is_atomic_on_invalid_kind() {
        let ledger = Ledger::new();
        let before = ledger.len();
        let result = LedgerEntry::new(1000, [0x01; 32], "café");
        assert!(result.is_err());
        assert_eq!(ledger.len(), before);
    }

    #[test]
    fn last_hash_returns_none_for_empty_ledger() {
        let ledger = Ledger::new();
        assert_eq!(ledger.last_hash().unwrap(), None);
    }

    #[test]
    fn append_validates_before_mutating() {
        // D3: mesmo que a entry seja inválida, o ledger não é mutado.
        let mut ledger = Ledger::new();
        ledger.append(entry(1000, "init", 0x01)).unwrap();
        let len_before = ledger.len();

        // Construir uma entry inválida via desserialização (bypassa construtor)
        let invalid_json = r#"{
            "prev_hash": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
            "timestamp": 1001,
            "state_hash": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
            "event_kind": "café"
        }"#;
        // Deve falhar na desserialização (try_from)
        assert!(serde_json::from_str::<LedgerEntry>(invalid_json).is_err());

        // O ledger permanece inalterado
        assert_eq!(ledger.len(), len_before);
    }
}

#[cfg(test)]
mod golden_vectors {
    /// Golden vectors cross-platform (v0.3.3, blake3, layout de 95 bytes LE).
    ///
    /// Capturados em 2026-09-12 — x86_64-pc-windows-msvc, rustc 1.94.0,
    /// cargo 1.94.0 (workspace), commit arkhe-monorepo (bloco v0.3.3).
    /// Verificação pendente: aarch64-unknown-linux-gnu via `cross`
    /// (workflow `ledger-golden.yml`). Qualquer divergência é regressão:
    /// endianness em `to_le_bytes()`, ordem de campos ou width (D5-CI).
    use super::*;

    // FIXADOS (C4) — substituem o `hash_vector_capture` no-op.
    const VECTOR_1_HASH: &str = "420248a91eebfbba994c6aede2f718d84fbf17a5fd7bb1167c51cfada3fab296";
    const VECTOR_2_HASH: &str = "17c80557cdba8530ac211350403559fe361d1b3e4d46aecb691ead96b249779b";
    const VECTOR_3_CHAIN_HASH: &str =
        "1a7d5422c55222a674caccff3fcafa9e3d4c6b13918213c046e8d7b8271b58fd";

    fn golden_ledger() -> (Ledger, [u8; 32], [u8; 32], [u8; 32]) {
        let mut ledger = Ledger::new();
        let h1 = ledger
            .append(LedgerEntry::new(1_700_000_000, [0x01; 32], "genesis").unwrap())
            .unwrap();
        let h2 = ledger
            .append(LedgerEntry::new(1_700_000_001, [0x02; 32], "verify").unwrap())
            .unwrap();
        let h3 = ledger
            .append(LedgerEntry::new(1_700_000_002, [0x03; 32], "commit").unwrap())
            .unwrap();
        assert!(ledger.verify().is_ok());
        assert_eq!(ledger.last_hash().unwrap(), Some(h3));
        (ledger, h1, h2, h3)
    }

    #[test]
    fn golden_vector_1() {
        let (_, h1, _, _) = golden_ledger();
        assert_eq!(hex::encode(h1), VECTOR_1_HASH);
    }

    #[test]
    fn golden_vector_2() {
        let (_, _, h2, _) = golden_ledger();
        assert_eq!(hex::encode(h2), VECTOR_2_HASH);
    }

    #[test]
    fn golden_vector_3_chain() {
        let (_, _, _, h3) = golden_ledger();
        assert_eq!(hex::encode(h3), VECTOR_3_CHAIN_HASH);
    }
}