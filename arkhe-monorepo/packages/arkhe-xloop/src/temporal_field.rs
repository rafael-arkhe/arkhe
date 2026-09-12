//! # TemporalField — O campo métrico do XLoop
//!
//! Kronos não é um contador. É um campo.
//!
//! Este módulo implementa a equação de Kronos como **métrica do loop**:
//!
//! ```text
//! ds² = -c² dt² + τ_eff · dℓ²
//! ```
//!
//! Onde:
//! - `c` = velocidade máxima de iteração (iterações/segundo)
//! - `τ_eff` = dilatação subjetiva (0.5 = rápido, 2.0 = lento)
//! - `dℓ` = distância percorrida (progresso na tarefa)
//!
//! ## Equação de Kronos
//!
//! ```text
//! Continuar se dτ_eff / dℓ > 0
//! ```
//!
//! Ou seja: **continue enquanto o tempo subjetivo está diminuindo
//! em relação ao progresso**. Se a dilatação cresce (mais erro,
//! mais novidade, mais latência), o campo está "expandindo" e o loop
//! deve colapsar — Kronos devora o próprio filho.
//!
//! ## Invariantes
//!
//! - `dilation ∈ [MIN_DILATION, MAX_DILATION]`
//! - `curvature ∈ [0.0, 1.0]`
//! - `energy` é monotonicamente crescente (nunca decresce)
//! - `objective_ms` e `proper_time_ms` são acumulados monotonamente
//!
//! ## Física — conexão com Penrose–Diósi e a crítica de Carlip
//!
//! O colapso é isomórfico a `T ~ ℏ / E_Δ` (auto-energia gravitacional da
//! diferença). O análogo cognitivo é `E_Δ = Var_cog` (divergência cognitiva).
//! Pela crítica de Carlip (1998), um objeto **não-rotante** (agente com um
//! único objetivo) tem `E_Δ = 0` e nunca colapsa — ver [`crate::objective_field`]
//! para o análogo das massas rotantes (divergência de objetivos).

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tracing::{debug, warn};

/// Constantes empíricas do campo temporal.
///
/// Ajustadas para que o comportamento do loop seja:
/// - `dilation ∈ [0.5, 2.0]` (calmo → lento)
/// - `curvature ∈ [0.0, 1.0]` (linear → dobrado sobre si)
pub mod constants {
    /// Constante de saturação ε (equação de Kronos).
    pub const EPSILON: f64 = 0.5;
    /// Escala de atualização mínima k.
    pub const K: f64 = 0.05;
    /// Constante de calibração empírica B.
    pub const B: f64 = 1.0;
    /// EMA α para suavizar dilatação.
    pub const EMA_ALPHA: f64 = 0.9;
    /// Tamanho da janela de histórico para detecção de curvatura.
    pub const HISTORY_WINDOW: usize = 32;
    /// Limite inferior de dilatação (tempo subjetivo máximo = rápido).
    pub const MIN_DILATION: f64 = 0.5;
    /// Limite superior de dilatação (tempo subjetivo mínimo = lento).
    pub const MAX_DILATION: f64 = 2.0;
    /// Curvatura crítica (acima disso = loop infinito).
    pub const CURVATURE_CRITICAL: f64 = 0.8;
    /// Curvatura de aviso (warn).
    pub const CURVATURE_WARNING: f64 = 0.5;
    /// Conversão de milissegundos para segundos na geodésica
    /// (os limiares 1.5/2.0 são expressos em `s / unidade-de-progresso`).
    pub const MS_PER_SEC: f64 = 1000.0;
    /// Taxa nominal de expansão temporal (`dτ/dℓ = 1.0` em condições ideais).
    pub const UNIT_RATE: f64 = 1.0;
    /// Múltiplo da taxa nominal que dispara `Collapse::Fallback`.
    pub const COLLAPSE_THRESHOLD: f64 = 2.0;
    /// Múltiplo da taxa nominal que dispara `Warning`.
    pub const WARNING_THRESHOLD: f64 = 1.5;
}

/// Campo temporal do XLoop — define a métrica do loop.
///
/// Invariantes:
/// - `dilation ∈ [MIN_DILATION, MAX_DILATION]`
/// - `curvature ∈ [0.0, 1.0]`
/// - `energy ≥ 0.0` (monotonicamente crescente, nunca decresce)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalField {
    /// Dilatação subjetiva (0.5 = rápido, 2.0 = lento).
    pub dilation: f64,
    /// Variância cognitiva na janela recente.
    pub cognitive_variance: f64,
    /// Carga de trabalho (0.0–1.0).
    pub cognitive_load: f64,
    /// Arousal (0.0–1.0).
    pub arousal: f64,
    /// Curvatura do campo (0.0 = linear, 1.0 = dobrado sobre si).
    pub curvature: f64,
    /// Energia acumulada (fuel consumido, tokens, iterações).
    pub energy: f64,
    /// Velocidade máxima de iteração (iterações/segundo).
    pub max_rate: f64,
    /// Histórico de assinaturas de estado (para detectar loops).
    history: VecDeque<String>,
    /// Tempo objetivo decorrido (ms) no momento da última atualização.
    pub objective_ms: u64,
    /// Tempo próprio acumulado (∫ τ_eff dt).
    pub proper_time_ms: f64,
}

impl Default for TemporalField {
    fn default() -> Self {
        Self {
            dilation: 1.0,
            cognitive_variance: 0.0,
            cognitive_load: 0.0,
            arousal: 0.0,
            curvature: 0.0,
            energy: 0.0,
            max_rate: 10.0,
            history: VecDeque::with_capacity(constants::HISTORY_WINDOW),
            objective_ms: 0,
            proper_time_ms: 0.0,
        }
    }
}

impl TemporalField {
    /// Cria um novo campo temporal com taxa máxima configurável.
    pub fn new(max_rate: f64) -> Self {
        Self {
            max_rate,
            ..Default::default()
        }
    }

    /// Atualiza o campo após cada iteração do loop.
    ///
    /// ## Equação
    ///
    /// ```text
    /// Var_cog = 0.4 · error_rate + 0.3 · novelty + 0.3 · min(latency/1000, 1)
    /// τ_eff   = 1 / (1 + ε · k · B · Var_cog)
    /// τ_eff  ← α · τ_eff_old + (1-α) · τ_eff_new
    /// ```
    ///
    /// Onde:
    /// - `error_rate ∈ [0, 1]` (fração de erros na janela)
    /// - `novelty ∈ [0, 1]` (novidade dos estados recentes)
    /// - `latency_ms` (tempo objetivo **da iteração** — per-iteration, não
    ///   cumulativo; o acumulado em `objective_ms` é derivado aqui)
    ///
    /// A atualização também acumula `objective_ms` e `proper_time_ms`:
    /// a contabilidade de tempo do loop nunca regride (Gravity-1).
    pub fn update(&mut self, error_rate: f64, novelty: f64, latency_ms: u64) {
        // 1. Variância cognitiva (Var_cog)
        let latency_normalized = (latency_ms as f64 / 1000.0).min(1.0);
        self.cognitive_variance = 0.4 * error_rate.clamp(0.0, 1.0)
            + 0.3 * novelty.clamp(0.0, 1.0)
            + 0.3 * latency_normalized;

        // 2. Dilatação nova (τ_eff_new)
        let new_dilation = 1.0
            / (1.0
                + constants::EPSILON
                    * constants::K
                    * constants::B
                    * self.cognitive_variance);

        // 3. EMA para suavizar
        self.dilation = constants::EMA_ALPHA * self.dilation
            + (1.0 - constants::EMA_ALPHA) * new_dilation;

        // 4. Clamp para invariante
        self.dilation = self.dilation.clamp(constants::MIN_DILATION, constants::MAX_DILATION);

        // 5. Acumulação monotônica de tempo objetivo + tempo próprio
        self.objective_ms = self.objective_ms.saturating_add(latency_ms);
        self.proper_time_ms += latency_ms as f64 * self.dilation * (1.0 - self.curvature);

        debug!(
            variance = %self.cognitive_variance,
            dilation = %self.dilation,
            "TemporalField updated"
        );
    }

    /// Detecta loop pela curvatura do campo.
    ///
    /// Curvatura = fração de ocorrências **anteriores** de `state_signature`
    /// dentro da janela de histórico, normalizada pelo tamanho da janela.
    ///
    /// - A primeira ocorrência de um estado nunca é loop (curvatura 0).
    /// - Curvatura alta = espaço-tempo do loop "dobrando" sobre si mesmo.
    ///
    /// Retorna `true` quando a curvatura cruza [`constants::CURVATURE_CRITICAL`].
    pub fn detect_loop(&mut self, state_signature: &str) -> bool {
        // Repetições da assinatura ANTES de inserir a ocorrência atual:
        // a primeira ocorrência de um estado não é, por si só, um loop.
        let repetitions = self
            .history
            .iter()
            .filter(|s| s.as_str() == state_signature)
            .count();

        if self.history.len() >= constants::HISTORY_WINDOW {
            self.history.pop_front();
        }
        self.history.push_back(state_signature.to_string());

        self.curvature = repetitions as f64 / constants::HISTORY_WINDOW as f64;

        if self.curvature >= constants::CURVATURE_CRITICAL {
            warn!(
                curvature = %self.curvature,
                signature = %state_signature,
                "Temporal curvature CRITICAL — loop infinite detected"
            );
            return true;
        }

        if self.curvature >= constants::CURVATURE_WARNING {
            debug!(
                curvature = %self.curvature,
                "Temporal curvature rising"
            );
        }

        false
    }

    /// Calcula o "tempo próprio" da tarefa no instante `objective_ms`.
    ///
    /// ```text
    /// τ_proper = objective_ms · τ_eff · (1 - curvature)
    /// ```
    ///
    /// Quando a curvatura se aproxima de 1, o tempo próprio **colapsa** —
    /// o loop está gastando tempo sem sair do lugar (isomorfismo
    /// Penrose–Diósi: `T ~ ℏ / E_Δ`, colapso quando o tempo útil → 0).
    pub fn proper_time(&self, objective_ms: u64) -> f64 {
        objective_ms as f64 * self.dilation * (1.0 - self.curvature)
    }

    /// Calcula a "distância" percorrida (progresso).
    ///
    /// ```text
    /// dℓ = ln(1 + completed_steps) / ln(1 + total_steps)
    /// ```
    ///
    /// Logarítmica porque o progresso é **decelerante** — cada passo
    /// adicional contribui menos para o avanço real.
    pub fn distance(&self, completed_steps: u64, total_steps: u64) -> f64 {
        if total_steps == 0 {
            return 0.0;
        }
        let num = (1.0 + completed_steps as f64).ln();
        let den = (1.0 + total_steps as f64).ln();
        if den == 0.0 {
            0.0
        } else {
            num / den
        }
    }

    /// **A Equação de Kronos.**
    ///
    /// Decide se deve continuar ou colapsar baseado na geodésica.
    ///
    /// ```text
    /// Continuar se dτ_eff / dℓ > 0
    /// ```
    ///
    /// A razão `dτ/dℓ` é expressa em **segundos de tempo objetivo por
    /// unidade de progresso normalizada** (`τ` de `proper_time` convertido
    /// para segundos; `ℓ` de `distance` em `[0, ·)`), o que torna os limiares
    /// `UNIT_RATE · 1.5` e `UNIT_RATE · 2.0` dimensionalmente legíveis:
    ///
    /// | Condição | Decisão | Ação |
    /// |:---|:---|:---|
    /// | `curvature ≥ 0.8` | Collapse | `TerminateLoop` |
    /// | `dτ/dℓ > 2.0` | Collapse | `Fallback` |
    /// | `1.5 < dτ/dℓ ≤ 2.0` | Warning | (log) |
    /// | `dτ/dℓ ≤ 1.5` | Continue | — |
    ///
    /// Sem divergência de objetivos (`E_Δ = 0`, crítica de Carlip) o campo
    /// é degenerado: a curvatura não sobe e o loop simplesmente termina.
    /// O colapso temporal requer [`crate::objective_field::ObjectiveField`].
    pub fn geodesic_decision(
        &self,
        objective_ms: u64,
        completed_steps: u64,
        total_steps: u64,
    ) -> GeodesicDecision {
        let tau_secs = self.proper_time(objective_ms) / constants::MS_PER_SEC;
        let ell = self.distance(completed_steps, total_steps);

        if ell <= 0.0 {
            return GeodesicDecision::Continue {
                reason: "No progress yet, continue".to_string(),
            };
        }

        let dtau_dell = tau_secs / ell;

        // Referência: sem dilatação e sem curvatura, dτ/dℓ = 1
        const UNIT_RATE: f64 = constants::UNIT_RATE;

        if self.curvature >= constants::CURVATURE_CRITICAL {
            return GeodesicDecision::Collapse {
                reason: format!("Curvature critical ({:.2})", self.curvature),
                action: CollapseAction::TerminateLoop,
            };
        }

        if dtau_dell > UNIT_RATE * constants::COLLAPSE_THRESHOLD {
            return GeodesicDecision::Collapse {
                reason: format!(
                    "Temporal expansion too fast: dτ/dℓ = {:.2} s/progress",
                    dtau_dell
                ),
                action: CollapseAction::Fallback,
            };
        }

        if dtau_dell > UNIT_RATE * constants::WARNING_THRESHOLD {
            return GeodesicDecision::Warning {
                reason: format!(
                    "Temporal expansion accelerating: dτ/dℓ = {:.2} s/progress",
                    dtau_dell
                ),
            };
        }

        GeodesicDecision::Continue {
            reason: format!("Temporal expansion nominal: dτ/dℓ = {:.2}", dtau_dell),
        }
    }

    /// Consome energia (fuel, tokens, iterações).
    ///
    /// `energy` é monotonicamente crescente — a contabilidade de gastos do
    /// loop nunca regride (invariante do campo).
    pub fn consume(&mut self, amount: f64) {
        self.energy += amount;
    }

    /// Retorna o "tempo subjetivo" percebido pelo agente.
    pub fn subjective_time(&self, objective_ms: u64) -> u64 {
        (objective_ms as f64 * self.dilation) as u64
    }

    /// Retorna o estado atual como snapshot imutável.
    pub fn snapshot(&self) -> TemporalFieldSnapshot {
        TemporalFieldSnapshot {
            dilation: self.dilation,
            cognitive_variance: self.cognitive_variance,
            curvature: self.curvature,
            energy: self.energy,
            proper_time_ms: self.proper_time_ms,
            objective_ms: self.objective_ms,
        }
    }
}

/// Snapshot imutável do campo temporal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalFieldSnapshot {
    /// Dilatação subjetiva no instante do snapshot.
    pub dilation: f64,
    /// Variância cognitiva no instante do snapshot.
    pub cognitive_variance: f64,
    /// Curvatura do campo no instante do snapshot.
    pub curvature: f64,
    /// Energia acumulada (nunca decresce).
    pub energy: f64,
    /// Tempo próprio acumulado (`∫ τ_eff dt`).
    pub proper_time_ms: f64,
    /// Tempo objetivo acumulado (ms).
    pub objective_ms: u64,
}

/// Decisão geodésica do loop.
#[derive(Debug, Clone, PartialEq)]
pub enum GeodesicDecision {
    /// Continue — o campo está nominal.
    Continue {
        /// Racional da decisão (trilha auditável).
        reason: String,
    },
    /// Aviso — o campo está acelerando.
    Warning {
        /// Racional da decisão (trilha auditável).
        reason: String,
    },
    /// Colapse — o campo dobrou sobre si.
    Collapse {
        /// Racional do colapso (trilha auditável).
        reason: String,
        /// Ação de colapso a executar.
        action: CollapseAction,
    },
}

/// Ação de colapso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollapseAction {
    /// Terminar o loop imediatamente.
    TerminateLoop,
    /// Tentar fallback (modelo mais barato, tool mais simples).
    Fallback,
    /// Solicitar intervenção humana.
    Escalate,
}

// ─────────────────────────────────────────────────────────────────────────────
// TESTES
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dilation_bounds() {
        let mut field = TemporalField::default();
        for _ in 0..1000 {
            field.update(1.0, 1.0, 5000);
        }
        assert!(field.dilation >= constants::MIN_DILATION);
        assert!(field.dilation <= constants::MAX_DILATION);
    }

    #[test]
    fn test_first_occurrence_is_not_loop() {
        let mut field = TemporalField::default();
        // A primeira ocorrência de um estado NÃO é loop (curvatura 0).
        assert!(!field.detect_loop("first_state"));
        assert_eq!(field.curvature, 0.0);
    }

    #[test]
    fn test_curvature_detection() {
        let mut field = TemporalField::default();
        for _ in 0..30 {
            field.detect_loop("same_state");
        }
        assert!(field.curvature >= constants::CURVATURE_CRITICAL);
    }

    #[test]
    fn test_oscillating_states_only_warn() {
        // Oscilação A→B→A→B...: nenhuma assinatura excede 50% da janela.
        let mut field = TemporalField::default();
        for i in 0..40 {
            let sig = if i % 2 == 0 { "A" } else { "B" };
            field.detect_loop(sig);
        }
        assert!(field.curvature < constants::CURVATURE_CRITICAL);
        assert!(field.curvature >= constants::CURVATURE_WARNING);
    }

    #[test]
    fn test_geodesic_continue() {
        let field = TemporalField::default();
        let decision = field.geodesic_decision(1000, 5, 10);
        assert!(matches!(decision, GeodesicDecision::Continue { .. }));
    }

    #[test]
    fn test_geodesic_warning() {
        let field = TemporalField::default();
        // 1.5s de tempo objetivo a 90% de progresso → 1.535 s/progresso.
        let decision = field.geodesic_decision(1_500, 90, 100);
        assert!(matches!(decision, GeodesicDecision::Warning { .. }));
    }

    #[test]
    fn test_geodesic_collapse_on_curvature() {
        let mut field = TemporalField::default();
        for _ in 0..30 {
            field.detect_loop("stuck");
        }
        let decision = field.geodesic_decision(5000, 1, 100);
        assert!(matches!(decision, GeodesicDecision::Collapse { .. }));
    }

    #[test]
    fn test_geodesic_collapse_on_temporal_expansion() {
        // Longo tempo objetivo com progresso modesto → Fallback.
        let field = TemporalField::default();
        let decision = field.geodesic_decision(60_000, 5, 10);
        assert!(matches!(
            decision,
            GeodesicDecision::Collapse {
                action: CollapseAction::Fallback,
                ..
            }
        ));
    }

    #[test]
    fn test_proper_time_collapse() {
        let field = TemporalField {
            curvature: 1.0,
            ..Default::default()
        };
        let tau = field.proper_time(1000);
        assert_eq!(tau, 0.0);
    }

    #[test]
    fn test_distance_logarithmic() {
        let field = TemporalField::default();
        let d1 = field.distance(1, 10);
        let d2 = field.distance(5, 10);
        let d3 = field.distance(10, 10);
        assert!(d1 < d2);
        assert!(d2 < d3);
        assert!((d3 - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_update_accumulates_objective_and_proper_time() {
        let mut field = TemporalField::default();
        let before = field.objective_ms;
        let pt_before = field.proper_time_ms;
        field.update(0.0, 0.0, 250);
        assert_eq!(field.objective_ms, before + 250);
        assert!(field.proper_time_ms > pt_before);
    }

    #[test]
    fn test_energy_monotonic() {
        let mut field = TemporalField::default();
        field.consume(2.0);
        field.consume(0.5);
        field.consume(3.0);
        assert_eq!(field.energy, 5.5);
    }

    #[test]
    fn test_snapshot_roundtrip() {
        let mut field = TemporalField::default();
        field.update(0.5, 0.5, 400);
        let snap = field.snapshot();
        assert_eq!(snap.dilation, field.dilation);
        assert_eq!(snap.curvature, field.curvature);
        assert_eq!(snap.objective_ms, field.objective_ms);
    }
}