//! `IterativeRefiner` — refinamento de coerência com Φ como função objetivo.
//!
//! Executa **gradiente ascendente numérico** sobre o vetor de conformidade
//! `(Ω, Σ, Λ)` maximizando `Φ` (docs/coherence_metric.md §9, Fase 2).
//!
//! Critérios de parada ([`RefinerOutcome`]):
//!
//! * [`RefinerOutcome::ReachedTarget`] — `Φ > REFINER_TARGET_PHI` (0.95).
//! * [`RefinerOutcome::Converged`] — variação de `Φ` inferior a
//!   `REFINER_PHI_EPSILON` (1e-6) entre iterações.
//! * [`RefinerOutcome::MaxIterations`] — limite de
//!   `REFINER_MAX_ITERATIONS` (1000) atingido.

use crate::coherence::phi;
use crate::coherence::WEIGHTS_DEFAULT;
use crate::constants::{
    REFINER_MAX_ITERATIONS, REFINER_PHI_EPSILON, REFINER_TARGET_PHI, WEIGHT_LATENCY,
    WEIGHT_STABILITY, WEIGHT_SUCCESS_RATE,
};

/// Resultado do refinamento: por que a iteração parou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefinerOutcome {
    /// `Φ` ultrapassou o alvo (bandeira de aceitação).
    ReachedTarget,
    /// Variação de `Φ` abaixo do epsilon configurado.
    Converged,
    /// Número máximo de iterações atingido sem convergir.
    MaxIterations,
}

/// Estado final do refinamento.
#[derive(Debug, Clone, PartialEq)]
pub struct RefinerResult {
    /// Motivo da parada.
    pub outcome: RefinerOutcome,
    /// Vetor de conformidade final `(Ω, Σ, Λ)`.
    pub components: [f64; 3],
    /// Coerência final `Φ`.
    pub phi: f64,
    /// `overall` (média ponderada) final — portão de aceitabilidade.
    pub overall: f64,
    /// Número de iterações executadas.
    pub iterations: u64,
}

/// Estado registrado em uma iteração do refinamento (trajetória).
#[derive(Debug, Clone, PartialEq)]
pub struct RefinerTraceStep {
    /// Índice da iteração.
    pub iteration: u64,
    /// `Φ` no estado registrado.
    pub phi: f64,
    /// `overall` no estado registrado.
    pub overall: f64,
    /// Vetor de conformidade no estado registrado.
    pub components: [f64; 3],
}

/// Resultado do refinamento acrescido da trajetória por iteração.
#[derive(Debug, Clone, PartialEq)]
pub struct RefinerTrace {
    /// Resultado terminal (idêntico à última linha relevante da trajetória).
    pub result: RefinerResult,
    /// Séries temporais por iteração (CSV do experimento E2).
    pub steps: Vec<RefinerTraceStep>,
}

/// Configuração do refinador.
#[derive(Debug, Clone, Copy)]
pub struct IterativeRefiner {
    /// Número máximo de iterações.
    pub max_iterations: u64,
    /// Alvo de `Φ` para `ReachedTarget`.
    pub target_phi: f64,
    /// Epsilon de convergência entre iterações.
    pub phi_epsilon: f64,
    /// Passo base do gradiente ascendente.
    pub learning_rate: f64,
}

impl Default for IterativeRefiner {
    fn default() -> Self {
        Self {
            max_iterations: REFINER_MAX_ITERATIONS,
            target_phi: REFINER_TARGET_PHI,
            phi_epsilon: REFINER_PHI_EPSILON,
            learning_rate: 0.05,
        }
    }
}

impl IterativeRefiner {
    /// Cria um refinador com a configuração canônica ([`Default`]).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Refina o vetor de conformidade inicial em direção a `Φ > target`.
    ///
    /// # Argumentos
    ///
    /// * `start` — vetor inicial `(Ω, Σ, Λ)`; componentes são limitados a
    ///   `[0,1]`.
    #[must_use]
    pub fn refine(&self, start: [f64; 3]) -> RefinerResult {
        self.refine_traced(start).result
    }

    /// Como [`Self::refine`], mas retorna também a trajetória por iteração
    /// (para o CSV de séries temporais do experimento E2).
    #[must_use]
    pub fn refine_traced(&self, start: [f64; 3]) -> RefinerTrace {
        let mut components = [
            start[0].clamp(0.0, 1.0),
            start[1].clamp(0.0, 1.0),
            start[2].clamp(0.0, 1.0),
        ];
        let mut steps: Vec<RefinerTraceStep> = Vec::new();

        for iter in 0..self.max_iterations {
            let phi_cur = phi(components, WEIGHTS_DEFAULT);
            steps.push(Self::trace_step(iter, components, phi_cur));
            if phi_cur > self.target_phi {
                let result = self.finish(RefinerOutcome::ReachedTarget, components, phi_cur, iter);
                return RefinerTrace { result, steps };
            }

            let gradient = self.gradient(components);
            let mut step_size = self.learning_rate;
            let mut next = Self::ascend(components, gradient, step_size);
            // Backtracking: reduz o passo enquanto Φ não subir (e o passo ainda
            // for significativo), garantindo ascendência numérica.
            while phi(next, WEIGHTS_DEFAULT) < phi_cur - 1e-12 && step_size > 1e-9 {
                step_size /= 2.0;
                next = Self::ascend(components, gradient, step_size);
            }

            let phi_next = phi(next, WEIGHTS_DEFAULT);
            if (phi_next - phi_cur).abs() < self.phi_epsilon {
                let result = self.finish(RefinerOutcome::Converged, next, phi_next, iter + 1);
                steps.push(Self::trace_step(iter + 1, next, phi_next));
                return RefinerTrace { result, steps };
            }
            components = next;
        }

        let phi_end = phi(components, WEIGHTS_DEFAULT);
        let result = self.finish(
            RefinerOutcome::MaxIterations,
            components,
            phi_end,
            self.max_iterations,
        );
        steps.push(Self::trace_step(self.max_iterations, components, phi_end));
        RefinerTrace { result, steps }
    }

    /// Gradiente numérico por diferença central de `Φ` (pesos canônicos).
    fn gradient(&self, components: [f64; 3]) -> [f64; 3] {
        let h = 1e-5_f64;
        let mut gradient = [0.0; 3];
        for (i, g) in gradient.iter_mut().enumerate() {
            let mut plus = components;
            let mut minus = components;
            plus[i] = (components[i] + h).min(1.0);
            minus[i] = (components[i] - h).max(0.0);
            let step = plus[i] - minus[i];
            *g = if step > 0.0 {
                (phi(plus, WEIGHTS_DEFAULT) - phi(minus, WEIGHTS_DEFAULT)) / step
            } else {
                0.0
            };
        }
        gradient
    }

    /// Um passo de gradiente ascendente com limitação a `[0,1]`.
    fn ascend(components: [f64; 3], gradient: [f64; 3], step_size: f64) -> [f64; 3] {
        let mut next = [0.0; 3];
        for (i, n) in next.iter_mut().enumerate() {
            *n = (components[i] + step_size * gradient[i]).clamp(0.0, 1.0);
        }
        next
    }

    /// Média ponderada canônica sobre um vetor de conformidade.
    fn overall_of(components: [f64; 3]) -> f64 {
        WEIGHT_STABILITY * components[0]
            + WEIGHT_SUCCESS_RATE * components[1]
            + WEIGHT_LATENCY * components[2]
    }

    fn trace_step(iteration: u64, components: [f64; 3], phi_value: f64) -> RefinerTraceStep {
        RefinerTraceStep {
            iteration,
            phi: phi_value,
            overall: Self::overall_of(components),
            components,
        }
    }

    fn finish(
        &self,
        outcome: RefinerOutcome,
        components: [f64; 3],
        phi_value: f64,
        iterations: u64,
    ) -> RefinerResult {
        RefinerResult {
            outcome,
            components,
            phi: phi_value,
            overall: Self::overall_of(components),
            iterations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn test_refiner_reaches_target_from_v2() {
        // Alvo do parecer: aproximar V1 (Φ ≈ 0.9646) a partir do estado
        // degradado V2 (Φ = 0.6983).
        let refiner = IterativeRefiner::default();
        let result = refiner.refine([0.80, 0.75, 0.50]);
        assert_eq!(result.outcome, RefinerOutcome::ReachedTarget);
        assert!(result.phi > 0.95, "Φ final = {}", result.phi);
        assert!(result.overall >= 0.80, "overall final = {}", result.overall);
        assert!(result.iterations < 1000, "iterações = {}", result.iterations);
    }

    #[test]
    fn test_refiner_already_at_target() {
        // V1 está acima do alvo → ReachedTarget na iteração 0, sem mover.
        let refiner = IterativeRefiner::default();
        let result = refiner.refine([0.9517, 1.0, 0.96]);
        assert_eq!(result.outcome, RefinerOutcome::ReachedTarget);
        assert_eq!(result.iterations, 0);
        assert!(near(result.phi, 0.9646, 1e-4));
    }

    #[test]
    fn test_refiner_from_null_floor_climbs() {
        // De (0,0,0) o refinador deve subir até o alvo dentro do limite.
        let refiner = IterativeRefiner::default();
        let result = refiner.refine([0.0, 0.0, 0.0]);
        assert!(result.phi > 0.95);
        assert!(result.iterations < 1000);
    }

    #[test]
    fn test_refiner_converges_with_tiny_step() {
        // Passo infinitesimal → variação de Φ < epsilon → Converged.
        let refiner = IterativeRefiner {
            learning_rate: 1e-12,
            ..Default::default()
        };
        let result = refiner.refine([0.50, 0.50, 0.50]);
        assert_eq!(result.outcome, RefinerOutcome::Converged);
    }

    #[test]
    fn test_refiner_components_stay_in_domain() {
        let refiner = IterativeRefiner::default();
        let result = refiner.refine([0.80, 0.75, 0.50]);
        for c in result.components {
            assert!((0.0..=1.0).contains(&c));
        }
    }

    #[test]
    fn test_refiner_monotonic_increases_phi() {
        // Φ final ≥ Φ inicial (ascendência garantida pelo backtracking).
        let refiner = IterativeRefiner::default();
        let start = [0.80, 0.75, 0.50];
        let start_phi = phi(start, WEIGHTS_DEFAULT);
        let result = refiner.refine(start);
        assert!(result.phi >= start_phi - 1e-12);
    }

    #[test]
    fn test_refiner_traced_matches_result_and_is_monotone() {
        let refiner = IterativeRefiner::default();
        let start = [0.60, 0.55, 0.50]; // V2-deep, Φ ≈ 0.5584
        let trace = refiner.refine_traced(start);
        assert_eq!(trace.result.outcome, RefinerOutcome::ReachedTarget);
        assert!(!trace.steps.is_empty());

        // A última linha da trajetória coincide com o resultado terminal.
        let last = trace.steps.last().expect("trajetória não vazia");
        assert!((last.phi - trace.result.phi).abs() < 1e-12);
        assert_eq!(last.components, trace.result.components);

        // Monotonia: Φ nunca decresce (tolerância numérica 1e-9).
        for w in trace.steps.windows(2) {
            assert!(
                w[1].phi >= w[0].phi - 1e-9,
                "Φ decresceu: {} → {} na iteração {}",
                w[0].phi,
                w[1].phi,
                w[1].iteration
            );
        }
    }
}