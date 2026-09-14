//! Traits de memória para integração com arkhe-agi.
//!
//! arkhe-agi usa estas traits em vez de depender de arkhe-memory diretamente.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Entrada de memória genérica.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    pub score: f32,
    pub layer: MemoryLayer,
    pub timestamp: DateTime<Utc>,
}

/// Camada de memória.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryLayer {
    Working,
    Episodic,
    Semantic,
    Procedural,
}

/// Trait para memória do agente.
///
/// Implementado por `EvolutionaryMemory` do arkhe-memory,
/// mas arkhe-agi só conhece esta interface.
#[async_trait]
pub trait AgentMemory: Send + Sync {
    /// Armazena uma entrada na memória.
    async fn store(&self, entry: MemoryEntry) -> Result<(), String>;

    /// Recupera entradas por chave (exata).
    async fn get(&self, key: &str) -> Option<MemoryEntry>;

    /// Busca entradas por relevância.
    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry>;
}

/// Memória em memória — útil para testes e desenvolvimento.
#[derive(Default)]
pub struct InMemoryAgentMemory {
    store: tokio::sync::RwLock<std::collections::HashMap<String, MemoryEntry>>,
}

impl InMemoryAgentMemory {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AgentMemory for InMemoryAgentMemory {
    async fn store(&self, entry: MemoryEntry) -> Result<(), String> {
        let mut store = self.store.write().await;
        store.insert(entry.key.clone(), entry);
        Ok(())
    }

    async fn get(&self, key: &str) -> Option<MemoryEntry> {
        let store = self.store.read().await;
        store.get(key).cloned()
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let store = self.store.read().await;
        let query_lower = query.to_lowercase();
        let mut results: Vec<&MemoryEntry> = store
            .values()
            .filter(|e| e.value.to_lowercase().contains(&query_lower))
            .take(limit)
            .collect();
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.into_iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, value: &str, score: f32) -> MemoryEntry {
        MemoryEntry {
            key: key.to_string(),
            value: value.to_string(),
            score,
            layer: MemoryLayer::Working,
            timestamp: Utc::now(),
        }
    }

    #[tokio::test]
    async fn store_then_get_roundtrips() {
        let mem = InMemoryAgentMemory::new();
        mem.store(entry("k1", "hello world", 0.9)).await.unwrap();
        let got = mem.get("k1").await.expect("present");
        assert_eq!(got.value, "hello world");
        assert!(mem.get("missing").await.is_none());
    }

    #[tokio::test]
    async fn search_filters_by_value_and_respects_limit() {
        let mem = InMemoryAgentMemory::new();
        mem.store(entry("a", "climate data east africa", 0.5)).await.unwrap();
        mem.store(entry("b", "climate model output", 0.9)).await.unwrap();
        mem.store(entry("c", "unrelated note", 0.1)).await.unwrap();
        let hits = mem.search("climate", 10).await;
        assert_eq!(hits.len(), 2);
        assert!(hits[0].score >= hits[1].score);
        assert_eq!(mem.search("climate", 1).await.len(), 1);
    }
}
