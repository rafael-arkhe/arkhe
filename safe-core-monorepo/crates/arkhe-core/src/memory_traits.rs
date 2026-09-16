//! Traits de memória para integração com arkhe-agi.
//!
//! arkhe-agi usa estas traits em vez de depender de arkhe-memory diretamente.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Entrada de memória genérica.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Chave de identidade da entrada, única por loja. É por ela que
    /// [`AgentMemory::get`] procura, com correspondência exata — sem prefixos
    /// nem padrões. O `AgiCoordinator` compõe-na como
    /// `{session_id}:turn-{millis}` para a camada de trabalho e
    /// `{session_id}:ep:{uuid}` para o episódio.
    pub key: String,
    /// Conteúdo textual da entrada, e o único campo que
    /// [`AgentMemory::search`] compara com a consulta (substring, insensível a
    /// maiúsculas). O `AgiCoordinator` guarda aqui o turno completo
    /// (`User: …\nAssistant: …`) na camada de trabalho, e só a pergunta
    /// (`Query: …`) no episódio.
    pub value: String,
    /// Relevância da entrada, por convenção em `[0,1]` — maior é mais
    /// relevante. Ordena os resultados de [`AgentMemory::search`], por ordem
    /// descendente, e é o peso mostrado por nó no grafo de memória de
    /// `arkhe-geometric-verifier`. Não é validado: nada impede um valor fora
    /// do intervalo, e a ordenação trata `NaN` como empatado.
    pub score: f32,
    /// Camada cognitiva a que a entrada pertence. É metadado de classificação,
    /// não critério de procura: a [`InMemoryAgentMemory`] guarda todas as
    /// camadas no mesmo mapa, e nem `get` nem `search` filtram por ela. Quem a
    /// interpreta são os consumidores — por exemplo o grafo de memória, que
    /// distingue o turno ([`MemoryLayer::Working`]) do episódio que o originou
    /// ([`MemoryLayer::Episodic`]).
    pub layer: MemoryLayer,
    /// Instante UTC em que a entrada foi produzida — não o da escrita: o
    /// `AgiCoordinator` reutiliza o mesmo `now` do turno para todas as entradas
    /// que dele derivam. Serve para datar e ordenar a evidência.
    pub timestamp: DateTime<Utc>,
}

/// Camada de memória.
///
/// A taxonomia é a clássica dos agentes cognitivos: o que está em curso, o que
/// aconteceu, o que se sabe, e o que se sabe fazer. O `arkhe-core` transporta-a
/// como rótulo; a política de retenção e de promoção entre camadas é de quem
/// implementa [`AgentMemory`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryLayer {
    /// Memória de trabalho: o turno corrente, com pedido e resposta, tal como
    /// aconteceram. É a camada que o `AgiCoordinator` escreve a cada
    /// `process()`.
    Working,
    /// Memória episódica: o registo por episódio — no `AgiCoordinator`, só a
    /// pergunta que originou o turno. Ligada ao turno correspondente pelo grafo
    /// de memória (FI-124).
    Episodic,
    /// Memória semântica: conhecimento destilado, independente do episódio em
    /// que foi aprendido — factos, definições, relações. Nenhum consumidor do
    /// workspace escreve atualmente nesta camada.
    Semantic,
    /// Memória procedural: saber-fazer — como executar uma tarefa ou uma
    /// sequência de passos. Nenhum consumidor do workspace escreve atualmente
    /// nesta camada.
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
    /// Cria uma memória vazia.
    ///
    /// Equivale a `Self::default()`. Existe como construtor explícito porque o
    /// sítio de chamada costuma ler-se melhor assim — o `AgiCoordinator` e os
    /// seus testes instanciam-no como
    /// `Arc::new(InMemoryAgentMemory::new())`.
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
