//! Constantes heurísticas para o cálculo de qualidade.
//!
//! Os pesos abaixo foram definidos empiricamente e podem ser ajustados
//! conforme a calibração do sistema em produção.

/// Peso da estabilidade no cálculo do overall (recomendado: 0.4).
pub const WEIGHT_STABILITY: f64 = 0.4;

/// Peso da taxa de sucesso no cálculo do overall (recomendado: 0.4).
pub const WEIGHT_SUCCESS_RATE: f64 = 0.4;

/// Peso da latência no cálculo do overall (recomendado: 0.2).
pub const WEIGHT_LATENCY: f64 = 0.2;

/// Limiar mínimo de qualidade para considerar o sistema saudável.
pub const QUALITY_THRESHOLD: f64 = 0.80;

/// Tolerância de latência para cálculo da pontuação (ms).
pub const LATENCY_TOLERANCE_MS: f64 = 50.0;

/// Alvo de coerência do `IterativeRefiner` (Φ > 0.95 = bandeira de aceitação).
pub const REFINER_TARGET_PHI: f64 = 0.95;

/// Número máximo de iterações do `IterativeRefiner`.
pub const REFINER_MAX_ITERATIONS: u64 = 1000;

/// Variação mínima de Φ entre iterações (critério de convergência).
pub const REFINER_PHI_EPSILON: f64 = 1e-6;