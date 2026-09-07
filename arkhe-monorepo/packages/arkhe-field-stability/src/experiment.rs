//! Infraestrutura dos experimentos E1–E4 (feature `experiments`).
//!
//! Fornece o RNG determinístico (Xorshift64*), as estatísticas compartilhadas
//! (média, desvio padrão, correlação de Pearson) e o sink de artefatos
//! (CSV + JSON em `mission_output/<experimento>_results/`). Nada aqui é
//! fachada: cada binário `e1`–`e4` compõe seus dados e resumos com estas
//! primitivas, garantindo reprodutibilidade (semente fixa).

use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Gerador determinístico Xorshift64* — mesma semente ⇒ mesma sequência.
#[derive(Debug, Clone)]
pub struct Srng {
    state: u64,
}

const SEED_DEFAULT: u64 = 0x9E37_79B9_7F4A_7C15;
const MUL: u128 = 0x2545_F491_4F6C_DD1D;

impl Default for Srng {
    fn default() -> Self {
        Self::new(SEED_DEFAULT)
    }
}

impl Srng {
    /// Cria um gerador com semente explícita (0 vira 1; nunca fica no estado
    /// fixo que degeneraria a sequência Xorshift).
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    /// Próximo valor pseudoaleatório uniforme em `[0, 1)`.
    #[must_use]
    pub fn next_f64(&mut self) -> f64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        let top = ((x as u128 * MUL) >> 64) as u64;
        top as f64 / (u64::MAX as f64 + 1.0)
    }

    /// Valor uniforme em `[lo, hi]`.
    #[must_use]
    pub fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }
}

/// Média aritmética; 0 para vetor vazio.
#[must_use]
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

/// Desvio padrão populacional; 0 para vetores com menos de 2 elementos.
#[must_use]
pub fn std(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    let variance = values.iter().map(|x| (x - m).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}

/// Correlação de Pearson entre duas séries de igual comprimento.
///
/// Retorna 0 quando as séries têm comprimento < 2 ou quando qualquer variância
/// é nula (divisão por zero evitada).
#[must_use]
pub fn pearson(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() || x.len() < 2 {
        return 0.0;
    }
    let n = x.len() as f64;
    let sum_x = x.iter().sum::<f64>();
    let sum_y = y.iter().sum::<f64>();
    let sum_xx = x.iter().map(|v| v * v).sum::<f64>();
    let sum_yy = y.iter().map(|v| v * v).sum::<f64>();
    let sum_xy = x.iter().zip(y).map(|(a, b)| a * b).sum::<f64>();
    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = ((n * sum_xx - sum_x * sum_x) * (n * sum_yy - sum_y * sum_y)).sqrt();
    if denominator <= 1e-300 {
        return 0.0;
    }
    (numerator / denominator).clamp(-1.0, 1.0)
}

/// Diretório de artefatos de um experimento
/// (`mission_output/<nome>_results/`), criado se necessário.
#[must_use]
pub fn artifact_root(experiment: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("mission_output")
        .join(format!("{experiment}_results"));
    fs::create_dir_all(&root).expect("criar diretório de artefatos do experimento");
    root
}

/// Grava conteúdo de texto (CSV) em `path`.
pub fn write_text(path: &Path, content: &str) -> io::Result<()> {
    fs::write(path, content)
}

/// Grava um valor serializável (JSON) em `path`, formatado e com quebra final.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(path, format!("{text}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rng_deterministic_same_seed() {
        let mut a = Srng::new(42);
        let mut b = Srng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_f64(), b.next_f64());
        }
    }

    #[test]
    fn test_rng_different_seeds_differ() {
        let mut a = Srng::new(1);
        let mut b = Srng::new(2);
        let mut equal = true;
        for _ in 0..64 {
            if a.next_f64() != b.next_f64() {
                equal = false;
                break;
            }
        }
        assert!(!equal, "sementes distintas produziram sequência idêntica");
    }

    #[test]
    fn test_rng_output_in_unit_interval() {
        let mut rng = Srng::default();
        for _ in 0..10_000 {
            let v = rng.next_f64();
            assert!((0.0..1.0).contains(&v));
        }
    }

    #[test]
    fn test_mean_std() {
        let values = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        assert!((mean(&values) - 5.0).abs() < 1e-12);
        assert!((std(&values) - 2.0).abs() < 1e-12);
        assert_eq!(mean(&[]), 0.0);
        assert_eq!(std(&[1.0]), 0.0);
    }

    #[test]
    fn test_pearson_perfect_correlation() {
        let x = [1.0, 2.0, 3.0, 4.0];
        let y = [2.0, 4.0, 6.0, 8.0];
        assert!((pearson(&x, &y) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_pearson_anti_correlation() {
        let x = [1.0, 2.0, 3.0, 4.0];
        let y = [4.0, 3.0, 2.0, 1.0];
        assert!((pearson(&x, &y) + 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_pearson_degenerates_to_zero() {
        assert_eq!(pearson(&[1.0], &[2.0]), 0.0);
        assert_eq!(pearson(&[1.0, 2.0], &[5.0, 5.0]), 0.0);
        let x = [1.0, 2.0, 3.0];
        let y = [9.0, 8.0, 7.0, 6.0];
        assert_eq!(pearson(&x, &y), 0.0);
    }

    #[test]
    fn test_artifact_root_prepends_manifest_dir() {
        let root = artifact_root("zz_test");
        assert!(root.ends_with(std::path::Path::new("mission_output/zz_test_results")));
    }
}