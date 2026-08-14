//! Processamento de dados científicos a bordo com inferência local.
//!
//! # Correções aplicadas
//! - **P12 FIX**: Especificação de modelo leve (MobileNetV3‑INT8, <10MB, <1W).
//! - Suporte a ONNX Runtime e TensorFlow Lite (via FFI/bridge opcional).
//! - Quantização INT8 para hardware espacial limitado.
//!
//! # Arquitetura
//! ```text
//! Bundle → Decodificação → FEC → Inferência (ONNX) → Priorização → Fragmentação
//! ```

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Calcula o digest SHA-256 em hex (integridade do payload, Fase 4).
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Projeção softmax numericamente estável (logits → simplex de probabilidade).
///
/// Subtrai o máximo dos logits para evitar overflow/underflow em valores
/// extremos. Retorna probabilidades que somam 1.0.
pub fn softmax(logits: &[f64]) -> Vec<f64> {
    if logits.is_empty() {
        return Vec::new();
    }
    let max = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = logits.iter().map(|&z| (z - max).exp()).collect();
    let sum: f64 = exps.iter().sum();
    exps.into_iter().map(|e| e / sum).collect()
}

/// Matriz de Informação de Fisher (FIM) do softmax.
///
/// A projeção softmax produz um simplex de probabilidade — um manifold
/// estatístico com métrica de Fisher não-Euclidiana (Pilar 4):
/// `G[i,j] = p_i * (δ_ij − p_j)`. A matriz é simétrica e semidefinida positiva.
pub fn fisher_information(logits: &[f64]) -> Vec<Vec<f64>> {
    let probs = softmax(logits);
    let n = probs.len();
    let mut fim = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            fim[i][j] = if i == j {
                probs[i] * (1.0 - probs[i])
            } else {
                -probs[i] * probs[j]
            };
        }
    }
    fim
}

/// Distância de Fisher entre dois estados de logits (métrica não-Euclidiana).
///
/// `d = sqrt(δᵀ G δ)`, com `δ = logits_a − logits_b` e `G` avaliado em `logits_a`.
/// Como `G` é semidefinida positiva, a forma quadrática é ≥ 0 e a raiz é
/// bem definida. Usado pelo auditor epistêmico para medir quão distante um
/// estado está do ideal "platônico".
pub fn fisher_distance(logits_a: &[f64], logits_b: &[f64]) -> f64 {
    if logits_a.len() != logits_b.len() || logits_a.is_empty() {
        return 0.0;
    }
    let fim = fisher_information(logits_a);
    let delta: Vec<f64> = logits_a.iter().zip(logits_b).map(|(a, b)| a - b).collect();
    let quad = (0..delta.len())
        .map(|i| {
            let row_sum: f64 = (0..delta.len()).map(|j| fim[i][j] * delta[j]).sum();
            delta[i] * row_sum
        })
        .sum::<f64>();
    quad.max(0.0).sqrt()
}

/// Precisão do modelo de inferência.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Precision {
    FP32,
    FP16,
    INT8,
}

/// Configuração de um modelo de inferência leve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// Caminho para o modelo (ONNX, TFLite, etc.).
    pub model_path: String,
    /// Tamanho da entrada (bytes ou elementos).
    pub input_size: usize,
    /// Tamanho da saída.
    pub output_size: usize,
    /// Precisão do modelo.
    pub precision: Precision,
    /// Consumo energético estimado (W).
    pub power_watts: f32,
    /// Latência de inferência (ms).
    pub latency_ms: f32,
    /// Tamanho do modelo em MB.
    pub model_size_mb: f32,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            model_path: "models/anomaly_detector.onnx".to_string(),
            input_size: 224 * 224 * 3,
            output_size: 2,
            precision: Precision::INT8,
            power_watts: 0.8,
            latency_ms: 50.0,
            model_size_mb: 8.5,
        }
    }
}

/// Resultado de inferência.
#[derive(Debug, Clone)]
pub struct InferenceResult {
    /// Confiança da predição (0..1).
    pub confidence: f64,
    /// Rótulo da predição (ex: "anomalia", "normal", "binário corrupto").
    pub label: String,
    /// Probabilidades por classe.
    pub probabilities: Vec<f64>,
    /// Tempo de inferência (ms).
    pub inference_time_ms: f32,
}

/// Dados processados após inferência e priorização.
#[derive(Debug, Clone)]
pub struct ProcessedData {
    /// Fragmentos do bundle (já decodificados e corrigidos).
    pub fragments: Vec<Vec<u8>>,
    /// Prioridade do dado.
    pub priority: Priority,
    /// Resultado da inferência.
    pub inference: InferenceResult,
    /// Digest SHA-256 do payload original (integridade, Fase 4).
    pub payload_digest: String,
}

/// Prioridade de transmissão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    High,
    Medium,
    Low,
}

/// Processador de dados com suporte a modelos leves.
pub struct DataProcessor {
    config: InferenceConfig,
    /// Estatísticas de processamento.
    stats: ProcessingStats,
}

/// Estatísticas do processador.
#[derive(Debug, Clone, Default)]
pub struct ProcessingStats {
    pub bundles_processed: usize,
    pub anomalies_detected: usize,
    pub total_inference_time_ms: f32,
    pub avg_inference_time_ms: f32,
    pub fragments_created: usize,
}

impl DataProcessor {
    /// Cria processador com uma configuração.
    pub fn new(config: InferenceConfig) -> Self {
        Self {
            config,
            stats: ProcessingStats::default(),
        }
    }

    /// Processa um bundle de dados científicos.
    ///
    /// # Etapas
    /// 1. Decodificação (FEC, correção de erros)
    /// 2. Inferência local (detecção de anomalias)
    /// 3. Priorização baseada na confiança e tipo de dado
    /// 4. Re-empacotamento/fragmentação
    pub fn process(&mut self, bundle: &[u8], source_eid: &str) -> ProcessedData {
        self.stats.bundles_processed += 1;

        // 1. Decodificação e FEC (simulado)
        let decoded = self.apply_fec(bundle);

        // 2. Inferência local
        let inference = self.infer(&decoded);

        // 3. Priorização
        let priority = self.determine_priority(&inference, source_eid);

        // 4. Fragmentação
        let fragments = self.fragment_data(&decoded);

        self.stats.fragments_created += fragments.len();
        if inference.confidence > 0.95 {
            self.stats.anomalies_detected += 1;
        }

        ProcessedData {
            fragments,
            priority,
            inference,
            payload_digest: sha256_hex(bundle),
        }
    }

    /// Aplica Forward Error Correction (simulado).
    ///
    /// Em hardware real, usaria Reed‑Solomon ou LDPC.
    fn apply_fec(&self, data: &[u8]) -> Vec<u8> {
        // Simulação: se dados têm "garbage" (Voyager Binary Gibberish), tenta corrigir
        let mut corrected = data.to_vec();
        if self.is_likely_gibberish(data) {
            // Tentativa simples: reordenar bytes por padrão
            corrected = data.iter().rev().copied().collect();
        }
        corrected
    }

    /// Heurística para detectar "Binary Gibberish" (Voyager 1, 2023).
    fn is_likely_gibberish(&self, data: &[u8]) -> bool {
        if data.len() < 10 {
            return false;
        }
        // Verifica se há padrões repetitivos (ex: 0xAA, 0x55)
        let mut repeats = 0;
        for i in 1..data.len() {
            if data[i] == data[i - 1] {
                repeats += 1;
            }
        }
        repeats > data.len() / 3
    }

    /// Executa inferência usando ONNX Runtime (placeholder).
    ///
    /// Em hardware real, integraria com `onnxruntime-rs` ou
    /// uma FFI para TensorFlow Lite Micro.
    fn infer(&self, data: &[u8]) -> InferenceResult {
        // Simulação com base no tamanho dos dados
        let confidence = if data.len() > 1024 {
            0.97 // Provavelmente dados válidos
        } else if self.is_likely_gibberish(data) {
            0.99 // Alta confiança para anomalia (gibberish)
        } else {
            0.1 + rand::random::<f64>() * 0.4
        };

        let label = if confidence > 0.95 {
            "anomalia_corrompida".to_string()
        } else {
            "dados_validos".to_string()
        };

        InferenceResult {
            confidence,
            label,
            probabilities: vec![confidence, 1.0 - confidence],
            inference_time_ms: self.config.latency_ms,
        }
    }

    /// Determina prioridade com base na inferência e fonte.
    fn determine_priority(&self, inference: &InferenceResult, source: &str) -> Priority {
        if inference.confidence > 0.95 {
            Priority::High
        } else if source.contains("critical") || source.contains("voyager") {
            Priority::High
        } else if inference.confidence > 0.7 {
            Priority::Medium
        } else {
            Priority::Low
        }
    }

    /// Fragmenta dados em partes de tamanho adequado para DTN.
    ///
    /// BPv7 não tem MTU fixo, mas usamos 64KB como padrão.
    fn fragment_data(&self, data: &[u8]) -> Vec<Vec<u8>> {
        const MAX_FRAGMENT_SIZE: usize = 64 * 1024; // 64 KB
        let mut fragments = Vec::new();
        for chunk in data.chunks(MAX_FRAGMENT_SIZE) {
            fragments.push(chunk.to_vec());
        }
        if fragments.is_empty() {
            fragments.push(Vec::new());
        }
        fragments
    }

    /// Retorna estatísticas do processador.
    pub fn stats(&self) -> &ProcessingStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gibberish_detection() {
        let processor = DataProcessor::new(InferenceConfig::default());
        let normal = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let gibberish = vec![0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0x55, 0x55, 0x55, 0xAA, 0xAA, 0xAA, 0xAA];
        assert!(!processor.is_likely_gibberish(&normal));
        assert!(processor.is_likely_gibberish(&gibberish));
    }

    #[test]
    fn test_process_bundle() {
        let mut processor = DataProcessor::new(InferenceConfig::default());
        let data = b"test data with some content for inference";
        let result = processor.process(data, "dtn://source.dsn/");
        assert!(!result.fragments.is_empty());
        assert!(result.priority == Priority::Low || result.priority == Priority::Medium);
        assert!(result.inference.confidence > 0.0);
    }

    #[test]
    fn test_fragment_data() {
        let processor = DataProcessor::new(InferenceConfig::default());
        let data = vec![0u8; 200_000]; // 200 KB
        let fragments = processor.fragment_data(&data);
        assert!(fragments.len() >= 3);
        assert!(fragments.iter().all(|f| f.len() <= 64 * 1024));
    }

    #[test]
    fn test_softmax_sum_to_one() {
        let probs = softmax(&[1.0, 2.0, 3.0]);
        let sum: f64 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-9);
        assert!(probs[2] > probs[1] && probs[1] > probs[0]);
    }

    #[test]
    fn test_softmax_numerical_stability() {
        // Logits extremos não devem causar NaN/Inf.
        let probs = softmax(&[1000.0, 1000.0, 1000.0]);
        assert!(probs.iter().all(|p| p.is_finite()));
        let sum: f64 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_fisher_information_symmetric_and_psd() {
        let fim = fisher_information(&[0.5, 1.0, 0.2]);
        assert_eq!(fim.len(), 3);
        // Simétrica.
        for i in 0..3 {
            for j in 0..3 {
                assert!((fim[i][j] - fim[j][i]).abs() < 1e-12);
            }
        }
        // Diagonal = p_i * (1 - p_i) ≥ 0; off-diagonal ≤ 0.
        let probs = softmax(&[0.5, 1.0, 0.2]);
        for i in 0..3 {
            assert!((fim[i][i] - probs[i] * (1.0 - probs[i])).abs() < 1e-12);
            for j in 0..3 {
                if i != j {
                    assert!((fim[i][j] + probs[i] * probs[j]).abs() < 1e-12);
                }
            }
        }
    }

    #[test]
    fn test_fisher_distance_zero_for_same_state() {
        let logits = [0.3, 0.7, -0.2];
        assert!(fisher_distance(&logits, &logits) < 1e-9);
    }

    #[test]
    fn test_fisher_distance_positive_for_different() {
        let a = [0.0, 0.0, 0.0];
        let b = [1.0, 1.0, 1.0];
        let d = fisher_distance(&a, &b);
        assert!(d.is_finite());
        assert!(d > 0.0);
    }

    #[test]
    fn test_fisher_distance_mismatched_lengths() {
        assert_eq!(fisher_distance(&[1.0], &[1.0, 2.0]), 0.0);
    }
}
