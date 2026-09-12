//! # DecagonalField — oscilação explore/exploit
//!
//! Analogia: o **decágono austral de Saturno** — uma onda planetária acoplada
//! ao jato polar com período de ~32 dias e 10 vértices. O fluxo não mergulha
//! numa estratégia única: **oscila periodicamente**.
//!
//! No campo de agentes:
//! - Período `32` iterações (não dias).
//! - 10 vértices = 10 transições de modo por período.
//! - 3/4 do período em `Exploit`, 1/4 em `Explore` (a onda aloca um quarto
//!   do tempo para novidade — sem isso o vórtice morre de inércia).

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Período padrão de oscilação (análogo aos 32 dias do decágono austral).
pub const DEFAULT_PERIOD: u64 = 32;
/// Vértices padrão (decágono).
pub const DEFAULT_VERTICES: usize = 10;
/// Fração do período dedicada ao modo `Explore` (3/4 Exploit, 1/4 Explore).
pub const EXPLORE_FRACTION: f64 = 0.25;

/// Modo atual do campo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldMode {
    /// Exploração — busca novidade (fertiliza o swarm).
    Explore,
    /// Explotação — aproveita a capacidade conhecida.
    Exploit,
}

/// Oscilação explore/exploit com período de 32 iterações.
///
/// Inspirado no decágono austral de Saturno. Lock-free (`AtomicU64`), síncrono
/// por natureza; o wrapper `current_mode` é assíncrono apenas para casar o
/// seam do orquestrador.
#[derive(Debug)]
pub struct DecagonalField {
    /// Contador de iterações (0..period).
    tick: AtomicU64,
    /// Período de oscilação (default: 32, como o decágono).
    period: u64,
    /// Número de vértices (default: 10, decágono).
    vertices: usize,
}

impl Default for DecagonalField {
    fn default() -> Self {
        Self::new(DEFAULT_PERIOD, DEFAULT_VERTICES)
    }
}

impl DecagonalField {
    /// Cria um campo com `period` e `vertices` configuráveis.
    pub fn new(period: u64, vertices: usize) -> Self {
        Self {
            tick: AtomicU64::new(0),
            period: period.max(1),
            vertices: vertices.max(1),
        }
    }

    /// Retorna o modo atual (Exploit em 3/4 do período, Explore no restante).
    ///
    /// A cada chamada o tick avança — o campo é um **pêndulo**, não uma
    /// consulta estática.
    pub async fn current_mode(&self) -> FieldMode {
        let tick = self.tick.fetch_add(1, Ordering::Relaxed);
        let phase = (tick % self.period) as f64 / self.period as f64;
        let explore_cut = 1.0 - EXPLORE_FRACTION;

        // Último 1/4 do período = Explore; primeiros 3/4 = Exploit.
        if phase >= explore_cut {
            FieldMode::Explore
        } else {
            FieldMode::Exploit
        }
    }

    /// Percentual de Exploit dentro de um período completo (teste/estatística).
    pub fn exploit_fraction(&self) -> f64 {
        1.0 - EXPLORE_FRACTION
    }

    /// Número de vértices configurado.
    pub fn vertices(&self) -> usize {
        self.vertices
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TESTES
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn period_yields_25_percent_explore() {
        let field = DecagonalField::default();
        let mut explore = 0usize;
        let mut exploit = 0usize;
        for _ in 0..DEFAULT_PERIOD {
            match field.current_mode().await {
                FieldMode::Explore => explore += 1,
                FieldMode::Exploit => exploit += 1,
            }
        }
        assert_eq!(explore, 8);
        assert_eq!(exploit, 24);
    }

    #[tokio::test]
    async fn mode_oscillates_over_time() {
        let field = DecagonalField::new(4, 2);
        let mut modes = Vec::new();
        for _ in 0..4 {
            modes.push(field.current_mode().await);
        }
        // Período 4: fases 0.0, 0.25, 0.5, 0.75 → Explore apenas na fase 0.75.
        let explore = modes
            .iter()
            .filter(|m| **m == FieldMode::Explore)
            .count();
        assert_eq!(explore, 1);
        assert_eq!(modes.len() - explore, 3);
    }

    #[tokio::test]
    async fn explore_window_is_the_last_quarter() {
        let field = DecagonalField::new(8, 4);
        let mut modes = Vec::new();
        for _ in 0..8 {
            modes.push(field.current_mode().await);
        }
        // Período 8: fases 0..0.75 (0,0.125,0.25,0.375,0.5,0.625) → Exploit;
        // fases 0.75 e 0.875 → Explore.
        assert_eq!(modes[..6].iter().filter(|m| **m == FieldMode::Explore).count(), 0);
        assert_eq!(modes[6..].iter().filter(|m| **m == FieldMode::Explore).count(), 2);
    }
}