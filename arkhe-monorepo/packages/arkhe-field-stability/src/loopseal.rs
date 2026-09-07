//! Loopseal — eixo de garantia da Fase 7 (parecer v381.0, reancorado em v381.1,
//! bloco 1005).
//!
//! O eixo Loopseal mede a **novidade** de uma entrada dentro de uma cadeia de
//! dados encadeada por hash (append-only, Loopseal-2). Cadências de handover
//! legitímias são **aecliclicas**: cada nova entrada apresenta um hash distinto
//! de todos os anteriores — um loop ocorre quando um hash já visto reaparece,
//! o que indicaria uma cadeia cíclica ou uma repetição indevida.
//!
//! ## Invariantes
//!
//! * **Loopseal-2 — append-only / acyclic:** ao adicionar uma entrada, o seu
//!   hash (derivado da [`canonical`](crate::ledger::CoherenceEntry::canonical))
//!   não pode ser igual a nenhum hash já presente; repetição ⇒ [`LoopStatus::Loop`].
//! * **Loopseal-3 — audit trail:** cada entrada carrega `previous_hash`; o
//!   rastreio do encadeamento é reportado, incluindo o `previous_hash` da nova
//!   entrada.
//!
//! > **Ortogonalidade:** assim como a SemanticValidity ([`crate::validators`]),
//! > o Loopseal é um **eixo de garantia de relatório**, não um componente de Φ
//! > (a fórmula canônica permanece `Ω/Σ/Λ` em [`crate::coherence`]).
//!
//! Este módulo é agnóstico em relação ao tipo de entrada — ele trabalha sobre
//! qualquer sequência de `(hash, previous_hash)` — o que permite reutilizá-lo
//! tanto para o [`CoherenceLedger`](crate::ledger) quanto para cadeias de
//! handover MCP ([`HandoverPayload`](crate::mcp_stub)).

use serde::{Deserialize, Serialize};

/// Resultado da análise de novidade/loop de uma entrada numa cadeia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopStatus {
    /// O hash desta entrada é novo em relação a todos os anteriores — cadeia
    /// aelíclica; a entrada é inserida normalmente.
    New {
        /// Eventual hash do antecessor (`None` para a primeira entrada).
        previous_hash: Option<String>,
    },
    /// O hash desta entrada já apareceu na cadeia — loop detectado; a entrada
    /// **não** deve ser inserida (violação de aelíclicidade).
    Loop {
        /// Índice da primeira ocorrência do hash duplicado em `seen_hashes`.
        duplicate_index: usize,
    },
}

/// Um elo da cadeia — suficiente para a análise de novidade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainLink {
    /// Hash efetivo do elo atual.
    pub hash: String,
    /// Hash efetivo do antecessor imediato (Elo-raiz se primeiro).
    pub previous_hash: String,
}

/// Estado incremental da análise de novidade de uma cadeia encadeada por hash.
///
/// Mantém o conjunto de hashes já vistos na ordem de ingesta (append-only) e
/// verifica a aelíclicidade a cada elo — Loopseal-2.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoopSeal {
    /// Hashes já incorporados, na ordem de ingesta.
    seen: Vec<String>,
}

impl LoopSeal {
    /// Cadeia vazia (nenhum hash visto).
    pub fn new() -> Self {
        Self { seen: Vec::new() }
    }

    /// Hashes já incorporados (ordem de ingesta).
    pub fn seen_hashes(&self) -> &[String] {
        &self.seen
    }

    /// Verifica a novidade de um elo e, se [`LoopStatus::New`], o incorpora.
    ///
    /// Se o hash do elo já estiver presente, retorna
    /// [`LoopStatus::Loop`] **sem alterar** o estado (Loopseal-2: a cadeia
    /// permanece intacta). Caso contrário, o hash é anexado e retorna
    /// [`LoopStatus::New`] com o `previous_hash` do elo.
    pub fn push(&mut self, link: &ChainLink) -> LoopStatus {
        if let Some(idx) = self.seen.iter().position(|h| h == &link.hash) {
            return LoopStatus::Loop { duplicate_index: idx };
        }
        self.seen.push(link.hash.clone());
        LoopStatus::New {
            previous_hash: Some(link.previous_hash.clone()),
        }
    }

    /// Cadeia aelíclica — nenhum algoritmo de loop detectado até agora.
    pub fn is_acyclic(&self) -> bool {
        // Invariante: `seen` nunca contém duplicatas (push rejeita loops),
        // então a ausência de duplicata é garantida pelo construtor. A
        // verificação é explícita para o audit trail.
        let mut set: std::collections::HashSet<&str> = std::collections::HashSet::new();
        self.seen.iter().all(|h| set.insert(h.as_str()))
    }
}

/// Analisa uma cadeia de elos do início e reporta o primeiro loop, se houver.
///
/// Retorna `None` se a cadeia é totalmente aelíclica; caso contrário, o
/// [`LoopStatus::Loop`] com o índice do elo duplicado (Loopseal-3).
pub fn verify_chain_acyclic(links: &[ChainLink]) -> Option<LoopStatus> {
    let mut ls = LoopSeal::new();
    for link in links {
        match ls.push(link) {
            LoopStatus::Loop { .. } => return Some(LoopStatus::Loop {
                duplicate_index: ls.seen_hashes().len(),
            }),
            LoopStatus::New { .. } => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(hash: impl Into<String>, prev: impl Into<String>) -> ChainLink {
        ChainLink {
            hash: hash.into(),
            previous_hash: prev.into(),
        }
    }

    #[test]
    fn single_link_is_new_and_acyclic() {
        let mut ls = LoopSeal::new();
        let l = link("H1", "GENESIS");
        assert_eq!(
            ls.push(&l),
            LoopStatus::New {
                previous_hash: Some("GENESIS".into())
            }
        );
        assert_eq!(ls.seen_hashes(), &["H1"]);
        assert!(ls.is_acyclic());
    }

    #[test]
    fn distinct_hashes_are_new() {
        let mut ls = LoopSeal::new();
        assert_eq!(
            ls.push(&link("H1", "GENESIS")),
            LoopStatus::New {
                previous_hash: Some("GENESIS".into())
            }
        );
        assert!(matches!(
            ls.push(&link("H2", "H1")),
            LoopStatus::New { .. }
        ));
        assert!(ls.is_acyclic());
        assert_eq!(ls.seen_hashes(), &["H1", "H2"]);
    }

    #[test]
    fn repeated_hash_is_loop_and_state_unchanged() {
        let mut ls = LoopSeal::new();
        assert!(matches!(
            ls.push(&link("H1", "GENESIS")),
            LoopStatus::New { .. }
        ));
        assert!(matches!(
            ls.push(&link("H2", "H1")),
            LoopStatus::New { .. }
        ));
        // H1 reaparece → loop no índice 0; estado NÃO incorpora o duplicado.
        assert_eq!(ls.push(&link("H1", "H2")), LoopStatus::Loop { duplicate_index: 0 });
        assert_eq!(ls.seen_hashes(), &["H1", "H2"]);
        // A cadeia (sem o duplicado) permanece aelíclica.
        assert!(ls.is_acyclic());
    }

    #[test]
    fn verify_chain_detects_first_loop() {
        let links = [
            link("H1", "GENESIS"),
            link("H2", "H1"),
            link("H3", "H2"),
            link("H1", "H3"), // loop detetado aqui
        ];
        assert_eq!(verify_chain_acyclic(&links), Some(LoopStatus::Loop { duplicate_index: 3 }));
    }

    #[test]
    fn verify_chain_acyclic_returns_none() {
        let links = [
            link("H1", "GENESIS"),
            link("H2", "H1"),
            link("H3", "H2"),
        ];
        assert_eq!(verify_chain_acyclic(&links), None);
    }

    #[test]
    fn empty_chain_is_acyclic() {
        let links: Vec<ChainLink> = Vec::new();
        assert_eq!(verify_chain_acyclic(&links), None);
        assert!(LoopSeal::new().is_acyclic());
    }

    #[test]
    fn first_duplicate_at_index_one() {
        let mut ls = LoopSeal::new();
        ls.push(&link("H1", "GENESIS"));
        // H2 cujo hash é igual a H1 (colisão / repetição imediata).
        assert_eq!(ls.push(&link("H1", "H1")), LoopStatus::Loop { duplicate_index: 0 });
    }

    #[test]
    fn property_long_distinct_chain_is_acyclic_and_no_dup_index() {
        // Propriedade (7.4): uma cadeia longa de hashes distintos permanece
        // aelíclica e o índice de duplicidade nunca é alcançado.
        let n = 512;
        let mut links = Vec::with_capacity(n);
        let mut prev = "GENESIS-ROOT".to_string();
        for i in 0..n {
            let hash = format!("H{i:04x}");
            links.push(link(hash.clone(), prev.clone()));
            prev = hash;
        }
        assert_eq!(verify_chain_acyclic(&links), None);
        let mut ls = LoopSeal::new();
        for l in &links {
            assert!(matches!(ls.push(l), LoopStatus::New { .. }));
        }
        assert_eq!(ls.seen_hashes().len(), n);
        assert!(ls.is_acyclic());
    }

    #[test]
    fn property_repeated_hash_always_flag_loop_same_index() {
        // Propriedade (7.4): detector é determinístico — reavaliar a mesma
        // cadeia com um loop produz o mesmo índice do primeiro duplicado.
        let links = [
            link("H1", "G"),
            link("H2", "H1"),
            link("H1", "H2"),
            link("H3", "H1"),
        ];
        let a = verify_chain_acyclic(&links);
        let b = verify_chain_acyclic(&links);
        assert_eq!(a, b);
        assert_eq!(b, Some(LoopStatus::Loop { duplicate_index: 2 }));
    }
}