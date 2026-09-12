//! # ObjectiveField — Divergência de objetivos (correção de Carlip)
//!
//! **A crítica de Carlip (1998)** mostra que, no regime Newtoniano, a fórmula
//! de Penrose–Diósi degenera: para um objeto não-rotante em superposição de
//! posição, `E_Δ = 0` e o tempo de colapso é `T = ∞`. O colapso só sobrevive
//! quando há **divergência** — massas em rotação relativa, momentos de inércia
//! que não cancelam.
//!
//! O análogo no [`crate::temporal_field::TemporalField`]:
//!
//! | Penrose / Carlip | XLoop |
//! |:---|:---|
//! | Objeto não-rotante em superposição | Agente com um único objetivo em espera |
//! | `E_Δ = 0` (cancelamento) | Campo degenerado (`τ_eff = 1`, sem colapso) |
//! | Massas rotantes em superposição | Agente com **múltiplos objetivos concorrentes** |
//! | `E_Δ ≠ 0` | Divergência de objetivos força decisão |
//!
//! O `ObjectiveField` é o **campo de massas rotantes** do loop: mede a
//! divergência dos objetivos ativos. Com divergência zero (`E_Δ = 0`) o campo
//! temporal nunca colapsa — Kronos só devora o filho quando há tensão.

use serde::{Deserialize, Serialize};

/// Campo de objetivos — divergência dos objetivos ativos.
///
/// Invariante: `energy_divergence ∈ [0.0, 1.0)` (0 = objetivo único = campo
/// degenerado de Carlip; próximo de 1 = máxima diversidade, `Gini impurity`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveField {
    /// Objetivos ativos com peso relativo.
    objectives: Vec<(String, f64)>,
    /// Momento de inércia cognitivo (diversidade de objetivos).
    inertia: f64,
}

impl Default for ObjectiveField {
    fn default() -> Self {
        Self {
            objectives: Vec::new(),
            inertia: 0.0,
        }
    }
}

impl ObjectiveField {
    /// Cria um campo de objetivos vazio (degenerado — nunca colapsa).
    pub fn new() -> Self {
        Self::default()
    }

    /// Adiciona (ou atualiza) um objetivo com peso `w ≥ 0`.
    pub fn set_objective(&mut self, name: &str, weight: f64) {
        let weight = weight.max(0.0);
        if let Some(entry) = self.objectives.iter_mut().find(|(n, _)| n == name) {
            entry.1 = weight;
        } else {
            self.objectives.push((name.to_string(), weight));
        }
    }

    /// Remove um objetivo (peso zero).
    pub fn clear_objective(&mut self, name: &str) {
        self.objectives.retain(|(n, _)| n != name);
    }

    /// Número de objetivos ativos com peso positivo.
    pub fn active_objectives(&self) -> usize {
        self.objectives.iter().filter(|(_, w)| *w > 0.0).count()
    }

    /// `E_Δ` análogo — divergência entre objetivos.
    ///
    /// `1 − Σ pᵢ²` (Gini impurity) sobre os pesos normalizados:
    ///
    /// - objetivo único `p₁ = 1` → `0.0` (campo degenerado de Carlip)
    /// - `n` objetivos iguais → `1 − 1/n` (máxima tensão)
    pub fn energy_divergence(&self) -> f64 {
        let total: f64 = self.objectives.iter().map(|(_, w)| *w).sum();
        if total <= 0.0 {
            return 0.0;
        }
        self.objectives
            .iter()
            .map(|(_, w)| (w / total).powi(2))
            .sum::<f64>()
            .mul_add(-1.0, 1.0)
    }

    /// Momento de inércia cognitivo (peso total dos objetivos).
    pub fn inertia(&self) -> f64 {
        self.inertia
    }

    /// Recalcula o momento de inércia a partir dos pesos atuais.
    pub fn recompute_inertia(&mut self) {
        self.inertia = self.objectives.iter().map(|(_, w)| *w).sum();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TESTES
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_is_degenerate() {
        // Nenhum objetivo → E_Δ = 0 (campo de Carlip: nunca colapsa).
        let field = ObjectiveField::new();
        assert_eq!(field.energy_divergence(), 0.0);
    }

    #[test]
    fn test_single_objective_is_degenerate() {
        // Um único objetivo → E_Δ = 0 → o TemporalField nunca colapsa por
        // gravidade temporal (a equação de Kronos requer divergência).
        let mut field = ObjectiveField::new();
        field.set_objective("primary", 1.0);
        assert_eq!(field.energy_divergence(), 0.0);
    }

    #[test]
    fn test_two_equal_objectives_max_tension() {
        let mut field = ObjectiveField::new();
        field.set_objective("a", 1.0);
        field.set_objective("b", 1.0);
        let d = field.energy_divergence();
        assert!((d - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_three_equal_objectives() {
        let mut field = ObjectiveField::new();
        field.set_objective("a", 1.0);
        field.set_objective("b", 1.0);
        field.set_objective("c", 1.0);
        let d = field.energy_divergence();
        assert!((d - (1.0 - 1.0 / 3.0)).abs() < 1e-12);
    }

    #[test]
    fn test_dominant_objective_low_tension() {
        let mut field = ObjectiveField::new();
        field.set_objective("a", 0.98);
        field.set_objective("b", 0.02);
        let d = field.energy_divergence();
        assert!(d < 0.05, "dominant objective should have low divergence: {d}");
        assert!(d > 0.0);
    }

    #[test]
    fn test_set_twice_keeps_single_entry() {
        let mut field = ObjectiveField::new();
        field.set_objective("a", 1.0);
        field.set_objective("a", 2.0);
        field.set_objective("b", 1.0);
        assert_eq!(field.active_objectives(), 2);
    }

    #[test]
    fn test_clear_objective() {
        let mut field = ObjectiveField::new();
        field.set_objective("a", 1.0);
        field.set_objective("b", 1.0);
        field.clear_objective("a");
        assert_eq!(field.active_objectives(), 1);
    }
}