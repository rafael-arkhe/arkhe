//! arkhe-block-registry — Registro canonico de blocos (bloco 1075, v582.0-exec).
//!
//! Formato canonico do hash: **hex string de 64 caracteres**, identico nos tres
//! artefactos do pipeline (Rust, Python, PowerShell). O array binario `[u8; 32]`
//! nao e interoperavel em JSON e e 3x maior — a decisao canonica do v582.0.
//!
//! Hashing: **SHA3-256** (invariante Ghost-1, dep `sha3` ja no workspace).
//! Divergencia honesta do plano v582.0 (que prescrevia BLAKE3): `blake3` nao
//! resolve neste ambiente offline (`constant_time_eq ^0.3` fora do cache; so
//! ha `0.1.5`). O gerador Python usa `hashlib.sha3_256` (stdlib) — hashes
//! identicos aos do Rust para os mesmos inputs.
//!
//! Semantica de validacao (espelho do verifier PowerShell, supra de 1 regra):
//!   1. Parse — JSON valido e campos obrigatorios (`numero`, `tipo`, `hash`)
//!   2. NumberCollision — mesmo numero, tipos diferentes
//!   3. DuplicateHash — mesmo numero, mesmo tipo, hashes diferentes
//!   4. HashReuse — mesmo hash usado por numeros diferentes
//!   5. OrphanParent — `parent_hash` sem bloco correspondente
//!   6. Cadeia — `parent_hash` do bloco N+1 == hash do bloco N (so Rust)

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha3::{Digest, Sha3_256};
use std::collections::HashMap;
use thiserror::Error;

// ============================================================================
// TIPO HASH — CANONICO EM HEX STRING
// ============================================================================

/// Hash de 32 bytes com serializacao JSON canonica em hex string (64 chars).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    /// Serializa como hex string em minusculas (64 caracteres).
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Desserializa de hex string, exigindo exatamente 32 bytes.
    pub fn from_hex(s: &str) -> Result<Self, RegistryError> {
        let bytes =
            hex::decode(s).map_err(|e| RegistryError::InvalidHash(e.to_string()))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| RegistryError::InvalidHash("esperados 32 bytes".into()))?;
        Ok(Hash(arr))
    }

    /// Bytes crus (para encadeamento de hash e comparação estrutural).
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Serialize for Hash {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Hash::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

// ============================================================================
// ERROS
// ============================================================================

/// Erros estruturais do registro de blocos.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    /// Mesmo número com tipos diferentes (bloco {0} já existe com tipo diferente).
    #[error("colisão de número: bloco {0} já existe com tipo diferente")]
    NumberCollision(u32),
    /// Mesmo número, mesmo tipo, hash diferente (bloco {0}).
    #[error("hash duplicado para o mesmo bloco: {0}")]
    DuplicateHash(u32),
    /// Hash já usado por outro número ({0}).
    #[error("reutilização de hash: {0} já foi usado")]
    HashReuse(String),
    /// Parent_hash sem bloco correspondente ({0}).
    #[error("parent órfão: {0} não encontrado")]
    OrphanParent(String),
    /// Hash não é hex válido de 32 bytes ({0}).
    #[error("hash inválido: {0}")]
    InvalidHash(String),
}

// ============================================================================
// REGISTRO
// ============================================================================

/// Registro individual de um bloco no formato canonico JSON.
///
/// ```json
/// { "numero": 1, "tipo": "ROOT", "hash": "<64-hex>", "parent_hash": "<64-hex>"? }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockRecord {
    pub numero: u32,
    pub tipo: String,
    pub hash: Hash,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_hash: Option<Hash>,
}

/// Registro com verificacao estrutural: colisoes, reuso de hash e orfaos.
#[derive(Debug, Default)]
pub struct BlockRegistry {
    blocks: HashMap<u32, BlockRecord>,
    hash_index: HashMap<Hash, u32>,
}

impl BlockRegistry {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            hash_index: HashMap::new(),
        }
    }

    /// Insere um bloco aplicando as invariantes estruturais.
    ///
    /// - Idempotente: mesmo número, mesmo tipo e mesmo hash → Ok sem duplicar.
    /// - `NumberCollision` se o número existe com tipo diferente.
    /// - `DuplicateHash` se o número existe com hash diferente.
    /// - `HashReuse` se o hash já pertence a outro número.
    /// - `OrphanParent` se `parent_hash` não está no registro.
    pub fn insert(&mut self, block: BlockRecord) -> Result<(), RegistryError> {
        if let Some(existing) = self.blocks.get(&block.numero) {
            if existing.tipo != block.tipo {
                return Err(RegistryError::NumberCollision(block.numero));
            }
            if existing.hash != block.hash {
                return Err(RegistryError::DuplicateHash(block.numero));
            }
            return Ok(());
        }

        if let Some(&other_num) = self.hash_index.get(&block.hash) {
            if other_num != block.numero {
                return Err(RegistryError::HashReuse(block.hash.to_hex()));
            }
        }

        if let Some(ref parent) = block.parent_hash {
            let parent_exists = self.blocks.values().any(|b| &b.hash == parent);
            if !parent_exists {
                return Err(RegistryError::OrphanParent(parent.to_hex()));
            }
        }

        self.hash_index.insert(block.hash, block.numero);
        self.blocks.insert(block.numero, block);
        Ok(())
    }

    /// Verifica o encadeamento estrito: `parent_hash` de cada bloco (exceto o
    /// raiz) deve ser o hash do bloco imediatamente anterior por ordem numerica.
    pub fn verify_chain(&self) -> Result<(), RegistryError> {
        let mut nums: Vec<u32> = self.blocks.keys().copied().collect();
        nums.sort_unstable();

        for window in nums.windows(2) {
            let prev = &self.blocks[&window[0]];
            let next = &self.blocks[&window[1]];
            if let Some(ref parent) = next.parent_hash {
                if parent != &prev.hash {
                    return Err(RegistryError::OrphanParent(parent.to_hex()));
                }
            }
        }
        Ok(())
    }

    pub fn get(&self, numero: u32) -> Option<&BlockRecord> {
        self.blocks.get(&numero)
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

// ============================================================================
// HASH DETERMINISTICO (SHA3-256)
// ============================================================================

/// Hash canonico de um bloco: `SHA3-256(numero_le4 || tipo_utf8 || parent?)`.
///
/// Mesmos inputs do gerador Python (`tools/generate_test_blocks.py`) para
/// producirem hashes identicos nos dois artefactos.
pub fn compute_block_hash(numero: u32, tipo: &str, parent: Option<&Hash>) -> Hash {
    let mut hasher = Sha3_256::new();
    hasher.update(numero.to_le_bytes());
    hasher.update(tipo.as_bytes());
    if let Some(p) = parent {
        hasher.update(p.0);
    }
    let digest = hasher.finalize();
    Hash(digest.into())
}

// ============================================================================
// TESTES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn root(n: u32) -> BlockRecord {
        let hash = compute_block_hash(n, "ROOT", None);
        BlockRecord {
            numero: n,
            tipo: "ROOT".into(),
            hash,
            parent_hash: None,
        }
    }

    #[test]
    fn test_empty_registry() {
        let reg = BlockRegistry::new();
        assert!(reg.is_empty());
    }

    #[test]
    fn test_insert_simple() {
        let mut reg = BlockRegistry::new();
        assert!(reg.insert(root(1)).is_ok());
        assert_eq!(reg.len(), 1);
        assert_eq!(reg.get(1).unwrap().tipo, "ROOT");
    }

    #[test]
    fn test_number_collision() {
        let mut reg = BlockRegistry::new();
        reg.insert(root(1)).unwrap();
        let block = BlockRecord {
            numero: 1,
            tipo: "AUDITORIA".into(),
            hash: compute_block_hash(1, "AUDITORIA", None),
            parent_hash: None,
        };
        assert!(matches!(
            reg.insert(block),
            Err(RegistryError::NumberCollision(1))
        ));
    }

    #[test]
    fn test_duplicate_hash_same_number() {
        let mut reg = BlockRegistry::new();
        reg.insert(root(1)).unwrap();
        let block = BlockRecord {
            numero: 1,
            tipo: "ROOT".into(),
            hash: compute_block_hash(1, "ROOT", None),
            parent_hash: None,
        };
        // mesmo numero/tipo/hash → idempotente Ok
        assert!(reg.insert(block.clone()).is_ok());
        assert_eq!(reg.len(), 1);

        // mesmo numero/tipo, payload diferente → DuplicateHash
        let mut other = block;
        other.hash = compute_block_hash(2, "ROOT", None);
        assert!(matches!(
            reg.insert(other),
            Err(RegistryError::DuplicateHash(1))
        ));
    }

    #[test]
    fn test_hash_reuse() {
        let mut reg = BlockRegistry::new();
        reg.insert(root(1)).unwrap();
        let block = BlockRecord {
            numero: 2,
            tipo: "GHOST".into(),
            hash: compute_block_hash(1, "ROOT", None), // mesmo hash do bloco 1
            parent_hash: None,
        };
        assert!(matches!(reg.insert(block), Err(RegistryError::HashReuse(_))));
    }

    #[test]
    fn test_idempotent_insert() {
        let mut reg = BlockRegistry::new();
        let block = root(1);
        reg.insert(block.clone()).unwrap();
        reg.insert(block).unwrap();
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn test_orphan_parent() {
        let mut reg = BlockRegistry::new();
        let fake_parent = compute_block_hash(99, "FAKE", None);
        let block = BlockRecord {
            numero: 1,
            tipo: "CHILD".into(),
            hash: compute_block_hash(1, "CHILD", Some(&fake_parent)),
            parent_hash: Some(fake_parent),
        };
        assert!(matches!(reg.insert(block), Err(RegistryError::OrphanParent(_))));
    }

    #[test]
    fn test_valid_chain_and_verify() {
        let mut reg = BlockRegistry::new();
        let h1 = compute_block_hash(1, "ROOT", None);
        let h2 = compute_block_hash(2, "CHILD", Some(&h1));
        let h3 = compute_block_hash(3, "GRANDCHILD", Some(&h2));
        reg.insert(BlockRecord { numero: 1, tipo: "ROOT".into(), hash: h1, parent_hash: None }).unwrap();
        reg.insert(BlockRecord { numero: 2, tipo: "CHILD".into(), hash: h2, parent_hash: Some(h1) }).unwrap();
        reg.insert(BlockRecord { numero: 3, tipo: "GRANDCHILD".into(), hash: h3, parent_hash: Some(h2) }).unwrap();
        assert!(reg.verify_chain().is_ok());
    }

    #[test]
    fn test_chain_mismatch_detected() {
        let mut reg = BlockRegistry::new();
        let h1 = compute_block_hash(1, "ROOT", None);
        let h2 = compute_block_hash(2, "CHILD", Some(&h1));
        reg.insert(BlockRecord { numero: 1, tipo: "ROOT".into(), hash: h1, parent_hash: None }).unwrap();
        reg.insert(BlockRecord { numero: 2, tipo: "CHILD".into(), hash: h2, parent_hash: Some(h1) }).unwrap();
        // bloco 3 referencia h1 (existe no registro) mas o predecessor imediato
        // por ordem numerica e o bloco 2 (hash h2) → verify_chain falha
        let h3 = compute_block_hash(3, "GRANDCHILD", Some(&h1));
        reg.insert(BlockRecord { numero: 3, tipo: "GRANDCHILD".into(), hash: h3, parent_hash: Some(h1) }).unwrap();
        assert!(matches!(
            reg.verify_chain(),
            Err(RegistryError::OrphanParent(_))
        ));
    }

    #[test]
    fn test_hash_hex_serialization() {
        let hash = compute_block_hash(1, "ROOT", None);
        let json = serde_json::to_string(&hash).unwrap();
        assert!(json.starts_with('"'));
        assert_eq!(json.len(), 66); // 64 chars + 2 aspas
        let parsed: Hash = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, hash);
    }

    #[test]
    fn test_hash_invalid_hex_rejected() {
        let err = Hash::from_hex("zz").unwrap_err();
        assert!(matches!(err, RegistryError::InvalidHash(_)));
        let err2 = Hash::from_hex(&"ab".repeat(31)).unwrap_err(); // 62 chars: nao 32 bytes
        assert!(matches!(err2, RegistryError::InvalidHash(_)));
    }

    #[test]
    fn test_compute_block_hash_deterministic_64_hex() {
        let a = compute_block_hash(1, "ROOT", None);
        let b = compute_block_hash(1, "ROOT", None);
        assert_eq!(a, b);
        assert_eq!(a.to_hex().len(), 64);
        // distintos para inputs distintos
        assert_ne!(a, compute_block_hash(2, "ROOT", None));
        assert_ne!(a, compute_block_hash(1, "AUDITORIA", None));
    }
}