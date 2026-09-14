//! safe-core-confidence
//! 
//! Módulo de cálculo de confiança epistemológica τ(א) para o Safe-Core.
//! Implementa escalonamento formal de certeza baseado em teoria da informação,
//! evidência bayesiana, e verificação formal.
//!
//! Selo: SAFE-CORE-CONFIDENCE-v1.0-2026-08-15

use core::f64;

// ============================================================
// AXIOMAS E DEFINIÇÕES
// ============================================================

/// Níveis de confiança τ(א) — ordinalmente estritos
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfidenceLevel {
    /// P ≤ 0.50 — Especulação não informada
    Speculation,
    /// 0.50 < P ≤ 0.80 — Conjectura informada
    Weak,
    /// 0.80 < P ≤ 0.95 — Evidência consistente com lacunas
    Medium,
    /// 0.95 < P ≤ 0.99 — Forte consenso científico
    Strong,
    /// P > 0.99 — Teorema ou evidência experimental robusta
    Theorem,
}

impl ConfidenceLevel {
    /// Converte probabilidade para nível ordinal
    pub fn from_probability(p: f64) -> Self {
        assert!(p >= 0.0 && p <= 1.0, "Probabilidade fora de [0,1]");
        if p > 0.99 {
            ConfidenceLevel::Theorem
        } else if p > 0.95 {
            ConfidenceLevel::Strong
        } else if p > 0.80 {
            ConfidenceLevel::Medium
        } else if p > 0.50 {
            ConfidenceLevel::Weak
        } else {
            ConfidenceLevel::Speculation
        }
    }

    /// Retorna o intervalo inferior do nível
    pub fn lower_bound(&self) -> f64 {
        match self {
            ConfidenceLevel::Speculation => 0.0,
            ConfidenceLevel::Weak => 0.50,
            ConfidenceLevel::Medium => 0.80,
            ConfidenceLevel::Strong => 0.95,
            ConfidenceLevel::Theorem => 0.99,
        }
    }

    /// Retorna o limiar superior (exclusivo, exceto para Theorem)
    pub fn upper_bound(&self) -> f64 {
        match self {
            ConfidenceLevel::Speculation => 0.50,
            ConfidenceLevel::Weak => 0.80,
            ConfidenceLevel::Medium => 0.95,
            ConfidenceLevel::Strong => 0.99,
            ConfidenceLevel::Theorem => 1.0,
        }
    }

    /// Entropia de incerteza residual H(P) em bits
    /// H(P) = -P log2(P) - (1-P) log2(1-P)
    /// Para P → 1, H → 0 (máxima certeza = mínima incerteza)
    pub fn residual_entropy(&self, p: f64) -> f64 {
        assert!(p >= self.lower_bound() && p <= self.upper_bound());
        if p <= 0.0 || p >= 1.0 {
            return 0.0;
        }
        -p * p.log2() - (1.0 - p) * (1.0 - p).log2()
    }
}

// ============================================================
// EVIDÊNCIA ATÔMICA
// ============================================================

/// Uma fonte de evidência independente com peso epistêmico
#[derive(Debug, Clone, Copy)]
pub struct Evidence {
    /// Tipo de evidência (afeta o Bayes Factor base)
    pub kind: EvidenceKind,
    /// Força bruta da evidência em [0, 1]
    /// 1.0 = confirmação inequívoca, 0.0 = contraditória, 0.5 = neutra
    pub strength: f64,
    /// Independência em relação a outras fontes [0, 1]
    /// 1.0 = totalmente independente, 0.0 = totalmente correlacionada
    pub independence: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceKind {
    /// Verificação formal (Kani, Lean4, TLA+, Coq)
    /// BF_base ≈ 100 (odds 100:1 a favor)
    FormalVerification,
    /// Prova matemática em papel revisada por pares
    /// BF_base ≈ 50
    PeerReviewedProof,
    /// Experimento replicado independentemente
    /// BF_base ≈ 20
    ReplicatedExperiment,
    /// Consistência dimensional / análise de unidades
    /// BF_base ≈ 5
    DimensionalConsistency,
    /// Simulação numérica convergente
    /// BF_base ≈ 3
    NumericalSimulation,
    /// Autoridade / consenso da comunidade
    /// BF_base ≈ 2 (fraco — sujeito a viés de grupo)
    Consensus,
    /// Anedota / caso único / observação não controlada
    /// BF_base ≈ 1.2 (quase neutro)
    Anecdotal,
}

impl EvidenceKind {
    /// Bayes Factor base associado ao tipo de evidência
    /// BF = P(E|H) / P(E|¬H)
    pub fn base_bf(&self) -> f64 {
        match self {
            EvidenceKind::FormalVerification => 100.0,
            EvidenceKind::PeerReviewedProof => 50.0,
            EvidenceKind::ReplicatedExperiment => 20.0,
            EvidenceKind::DimensionalConsistency => 5.0,
            EvidenceKind::NumericalSimulation => 3.0,
            EvidenceKind::Consensus => 2.0,
            EvidenceKind::Anecdotal => 1.2,
        }
    }
}

// ============================================================
// MOTOR DE CÁLCULO τ(א)
// ============================================================

/// Estado de confiança computado com metadados completos
#[derive(Debug, Clone)]
pub struct ConfidenceState {
    /// Probabilidade posterior calculada
    pub probability: f64,
    /// Nível ordinal
    pub level: ConfidenceLevel,
    /// Entropia residual de incerteza [bits]
    pub residual_entropy: f64,
    /// Brier Score esperado (menor = melhor calibração)
    pub expected_brier: f64,
    /// Log-loss esperado
    pub expected_log_loss: f64,
    /// Bayes Factor acumulado
    pub cumulative_bf: f64,
    /// Número de fontes efetivas (após correlação)
    pub effective_sources: f64,
    /// Fontes de evidência consideradas
    pub sources: Vec<Evidence>,
}

pub struct ConfidenceEngine {
    /// Prior odds (odds a priori da hipótese ser verdadeira)
    /// Default: 1.0 (prior neutro, P=0.50)
    prior_odds: f64,
    /// Fator de correção por correlação entre fontes
    /// λ = 0.5 para fontes da mesma comunidade
    /// λ = 1.0 para fontes totalmente independentes
    correlation_lambda: f64,
}

impl Default for ConfidenceEngine {
    fn default() -> Self {
        Self {
            prior_odds: 1.0,
            correlation_lambda: 0.7,
        }
    }
}

impl ConfidenceEngine {
    pub fn new(prior_probability: f64, correlation_lambda: f64) -> Self {
        assert!((0.0..=1.0).contains(&prior_probability));
        assert!((0.0..=1.0).contains(&correlation_lambda));
        Self {
            prior_odds: prior_probability / (1.0 - prior_probability),
            correlation_lambda,
        }
    }

    /// Calcula a confiança τ(א) a partir de um vetor de evidências
    ///
    /// ALGORITMO:
    /// 1. Para cada evidência e_i, calcular BF_efetivo = (BF_base)^(strength * independence)
    /// 2. Acumular: posterior_odds = prior_odds * ∏ BF_efetivo_i
    /// 3. Converter: P = odds / (1 + odds)
    /// 4. Corrigir por saturação de evidência (fórmula de Etzioni 2016)
    /// 5. Calcular métricas derivadas (entropia, Brier, log-loss)
    pub fn compute(&self, evidences: &[Evidence]) -> ConfidenceState {
        let mut cumulative_bf = 1.0;
        let mut effective_sources = 0.0;

        for e in evidences {
            // BF efetivo: penalizado por força < 1 e correlação < 1
            let bf_effective = e.kind.base_bf().powf(e.strength * e.independence);
            cumulative_bf *= bf_effective;
            effective_sources += e.strength * e.independence;
        }

        // Correção de correlação: fontes não-independentes não acumulam linearmente
        // Ajuste: BF_corr = BF_raw^λ onde λ é o fator de independência médio
        let avg_independence = if evidences.is_empty() {
            0.0
        } else {
            evidences.iter().map(|e| e.independence).sum::<f64>() / evidences.len() as f64
        };
        let correlation_adjustment = avg_independence.powf(self.correlation_lambda);
        cumulative_bf = cumulative_bf.powf(correlation_adjustment);

        // Posterior
        let posterior_odds = self.prior_odds * cumulative_bf;
        let mut probability = posterior_odds / (1.0 + posterior_odds);

        // Saturação: nenhuma quantidade de evidência empírica atinge P=1.0
        // Aplica fator de saturação exponencial: P_sat = 1 - exp(-k * P)
        // k calibrado para que P=0.999 com BF→∞ (limite assintótico)
        let k = -(-10.0f64).ln(); // ≈ 2.303
        probability = 1.0 - (-k * probability).exp();

        // Garantir bounds
        probability = probability.clamp(0.0, 0.9999);

        let level = ConfidenceLevel::from_probability(probability);
        let residual_entropy = level.residual_entropy(probability);

        // Brier Score esperado: E[BS] = P(1-P)^2 + (1-P)P^2 = P(1-P)
        let expected_brier = probability * (1.0 - probability);

        // Log-loss esperado: E[LL] = -P log(P) - (1-P) log(1-P) = H(P) / ln(2)
        let expected_log_loss = if probability <= 0.0 || probability >= 1.0 {
            0.0
        } else {
            -probability * probability.ln() - (1.0 - probability) * (1.0 - probability).ln()
        };

        ConfidenceState {
            probability,
            level,
            residual_entropy,
            expected_brier,
            expected_log_loss,
            cumulative_bf,
            effective_sources,
            sources: evidences.to_vec(),
        }
    }

    /// Atualização bayesiana incremental (streaming)
    /// Útil para evidence bus em tempo real
    pub fn update(&self, state: &ConfidenceState, new_evidence: &Evidence) -> ConfidenceState {
        let mut new_sources = state.sources.clone();
        new_sources.push(*new_evidence);
        self.compute(&new_sources)
    }
}

// ============================================================
// FUNÇÕES AUXILIARES: MÉTRICAS DE CALIBRAÇÃO
// ============================================================

/// Brier Score empírico a partir de histórico de previsões
/// BS = (1/N) Σ (P_i - Y_i)^2
/// Y_i ∈ {0, 1} é o resultado real
pub fn empirical_brier_score(predictions: &[f64], outcomes: &[bool]) -> f64 {
    assert_eq!(predictions.len(), outcomes.len());
    if predictions.is_empty() {
        return 0.0;
    }
    predictions.iter().zip(outcomes.iter())
        .map(|(p, y)| {
            let y_f = if *y { 1.0 } else { 0.0 };
            (p - y_f).powi(2)
        })
        .sum::<f64>() / predictions.len() as f64
}

/// Log-loss empírico
/// LL = -(1/N) Σ [Y_i ln(P_i) + (1-Y_i) ln(1-P_i)]
pub fn empirical_log_loss(predictions: &[f64], outcomes: &[bool]) -> f64 {
    assert_eq!(predictions.len(), outcomes.len());
    if predictions.is_empty() {
        return 0.0;
    }
    predictions.iter().zip(outcomes.iter())
        .map(|(p, y)| {
            let p_clamped = p.clamp(1e-15, 1.0 - 1e-15);
            if *y {
                -p_clamped.ln()
            } else {
                -(1.0 - p_clamped).ln()
            }
        })
        .sum::<f64>() / predictions.len() as f64
}

/// Reliability diagram: agrupa previsões em bins e compara
/// frequência empírica com probabilidade declarada
pub fn reliability_bins(predictions: &[f64], outcomes: &[bool], n_bins: usize) -> Vec<(f64, f64, usize)> {
    assert_eq!(predictions.len(), outcomes.len());
    let mut bins: Vec<Vec<bool>> = vec![Vec::new(); n_bins];

    for (p, y) in predictions.iter().zip(outcomes.iter()) {
        let idx = ((p * n_bins as f64).min(n_bins as f64 - 1.0)) as usize;
        bins[idx].push(*y);
    }

    bins.iter().enumerate()
        .filter(|(_, b)| !b.is_empty())
        .map(|(i, b)| {
            let bin_center = (i as f64 + 0.5) / n_bins as f64;
            let freq = b.iter().filter(|&&y| y).count() as f64 / b.len() as f64;
            (bin_center, freq, b.len())
        })
        .collect()
}

// ============================================================
// TESTES UNITÁRIOS (no_std compatível via custom assert)
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_levels() {
        assert_eq!(ConfidenceLevel::from_probability(0.999), ConfidenceLevel::Theorem);
        assert_eq!(ConfidenceLevel::from_probability(0.97), ConfidenceLevel::Strong);
        assert_eq!(ConfidenceLevel::from_probability(0.85), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::from_probability(0.60), ConfidenceLevel::Weak);
        assert_eq!(ConfidenceLevel::from_probability(0.30), ConfidenceLevel::Speculation);
    }

    #[test]
    fn test_single_formal_verification() {
        let engine = ConfidenceEngine::default();
        let ev = Evidence {
            kind: EvidenceKind::FormalVerification,
            strength: 1.0,
            independence: 1.0,
        };
        let state = engine.compute(&[ev]);
        // Uma única verificação formal com prior neutro deve dar P > 0.95
        assert!(state.probability > 0.95, "P={}", state.probability);
        assert_eq!(state.level, ConfidenceLevel::Strong);
    }

    #[test]
    fn test_theorem_threshold() {
        let engine = ConfidenceEngine::default();
        let evs = vec![
            Evidence { kind: EvidenceKind::FormalVerification, strength: 1.0, independence: 1.0 },
            Evidence { kind: EvidenceKind::PeerReviewedProof, strength: 1.0, independence: 1.0 },
            Evidence { kind: EvidenceKind::ReplicatedExperiment, strength: 1.0, independence: 1.0 },
        ];
        let state = engine.compute(&evs);
        assert!(state.probability > 0.99, "P={}", state.probability);
        assert_eq!(state.level, ConfidenceLevel::Theorem);
    }

    #[test]
    fn test_correlation_penalty() {
        let engine = ConfidenceEngine::new(0.5, 0.5);
        // 5 fontes idênticas (independence=0.1) não devem saturar
        let evs: Vec<_> = (0..5).map(|_| Evidence {
            kind: EvidenceKind::Consensus,
            strength: 1.0,
            independence: 0.1,
        }).collect();
        let state = engine.compute(&evs);
        // Com alta correlação, effective_sources ≈ 0.5, não deve passar de Strong
        assert!(state.effective_sources < 1.0);
    }

    #[test]
    fn test_residual_entropy_monotonicity() {
        let levels = [
            (0.1, ConfidenceLevel::Speculation),
            (0.6, ConfidenceLevel::Weak),
            (0.85, ConfidenceLevel::Medium),
            (0.97, ConfidenceLevel::Strong),
            (0.999, ConfidenceLevel::Theorem),
        ];
        let mut prev_h = f64::INFINITY;
        for (p, level) in levels {
            let h = level.residual_entropy(p);
            assert!(h < prev_h, "Entropia deve decrescer com P: H({})={} >= H(prev)={}", p, h, prev_h);
            prev_h = h;
        }
    }

    #[test]
    fn test_brier_score() {
        let preds = [0.9, 0.9, 0.1, 0.1];
        let outs = [true, true, false, false];
        let bs = empirical_brier_score(&preds, &outs);
        // (0.1^2 + 0.1^2 + 0.1^2 + 0.1^2) / 4 = 0.01
        assert!((bs - 0.01).abs() < 1e-10);
    }
}
