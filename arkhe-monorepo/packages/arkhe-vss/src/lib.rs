//! # ARKHE VSS — Feldman Verifiable Secret Sharing
//!
//! Escrutínio da nuance **dealer honesto vs dealer malicioso** (plano
//! `ARKHE-CRYPTO-CODE-TESTABLE-2026-09-12`): no Feldman VSS a verificabilidade é
//! **condicional à honestidade do dealer** — não a elimina. Cada participante
//! `P_i` verifica a sua share contra os compromissos públicos [`FeldmanDealer`],
//! publica uma **complaint** se `g^{s_i} ≠ ∏ C_k^{i^k}` e o dealer é
//! **desqualificado** assim que o número de complaints excede o threshold.
//!
//! ## Modelo
//!
//! * `FeldmanDealer::new(secret, threshold, total)` fixa `f(x) = a_0 + a_1·x + … +
//!   a_{t-1}·x^{t-1}` e publica `C_k = g^{a_k}` (base Ristretto255).
//! * `honest_shares` emite shares consistentes; `malicious_shares(victim)`
//!   corrompe uma share para simular o dealer que se desvia do polinómio.
//! * `verify_share` materializa `g^{s_i} = ∏_{k=0}^{t-1} C_k^{i^k}`.
//! * [`ComplaintCollector`] acumula complaints por dealer e decide a
//!   desqualificação quando `count > threshold` (regra do Feldman VSS).
//! * A reconstrução Lagrangeana em `x = 0` recupera `a_0 = f(0)` — o segredo —
//!   a partir de `t` shares, nunca de `f(0)` directo (as shares vivem em `1..=n`).
//!
//! ## Invariantes tocados
//!
//! * **Ghost-1** (integridade): o polinómio comprometido confere com as shares;
//!   divergência gera complaint.
//! * **Gap-2** (entropia): coeficientes via `OsRng`.
//! * **Simplicity-2** (superfície mínima): `curve25519-dalek 4.1.3` e `rand 0.8`,
//!   ambos já no `Cargo.lock` do workspace — **zero dependências externas novas**.
//!
//! Zero `unsafe` (lint `unsafe_code = deny`).
//!
//! Honestidade (precedentes blocos 990/1004/1006/1011): este módulo materializa
//! a *deteção* de dealer malicioso; a *eliminação* da suposição de confiança é um
//! problema de protocolo (ex.: PVSS/verifiable secret sharing distribuído) fora
//! do escopo deste bloco.

use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT,
    ristretto::RistrettoPoint,
    scalar::Scalar,
    traits::Identity,
};
use rand::rngs::OsRng;
use std::collections::HashMap;

/// Polinómio `f(x) = a_0 + a_1·x + … + a_{t-1}·x^{t-1}` e compromissos públicos
/// `C_k = g^{a_k}` sobre Ristretto255.
///
/// `a_0` é o segredo partilhado; as shares são as avaliações `s_i = f(i)`.
#[derive(Clone)]
pub struct FeldmanDealer {
    /// Coeficientes do polinómio (`a_0` é o segredo).
    coefficients: Vec<Scalar>,
    /// Compromissos públicos `C_k = g^{a_k}` (length = threshold).
    commitments: Vec<RistrettoPoint>,
}

impl FeldmanDealer {
    /// Cria um dealer com o segredo `a_0` e `threshold - 1` coeficientes
    /// aleatórios. O `total` de participantes faz parte da API (validação); o
    /// polinómio define-se apenas pelo `threshold`.
    pub fn new(secret: Scalar, threshold: usize, total: usize) -> Self {
        let _ = total;
        let mut rng = OsRng;
        let mut coefficients = vec![secret];
        for _ in 1..threshold {
            coefficients.push(Scalar::random(&mut rng));
        }
        let commitments: Vec<_> = coefficients
            .iter()
            .map(|a| RISTRETTO_BASEPOINT_POINT * a)
            .collect();
        Self {
            coefficients,
            commitments,
        }
    }

    /// Gera `total` shares consistentes com o polinómio: `s_i = f(i)`, `i = 1..=total`.
    pub fn honest_shares(&self, total: usize) -> Vec<(usize, Scalar)> {
        (1..=total)
            .map(|i| {
                let x = Scalar::from(i as u64);
                let mut share = Scalar::ZERO;
                let mut x_pow = Scalar::ONE;
                for coeff in &self.coefficients {
                    share += coeff * x_pow;
                    x_pow *= x;
                }
                (i, share)
            })
            .collect()
    }

    /// Gera shares simulando um **dealer malicioso**: corrompe a share do
    /// participante `victim` (adiciona `1`), mantendo os compromissos intactos.
    pub fn malicious_shares(&self, total: usize, victim: usize) -> Vec<(usize, Scalar)> {
        let mut shares = self.honest_shares(total);
        for (i, share) in shares.iter_mut() {
            if *i == victim {
                *share += Scalar::ONE;
            }
        }
        shares
    }

    /// Verifica uma share contra os compromissos públicos: `g^{s_i} = ∏ C_k^{i^k}`.
    pub fn verify_share(&self, index: usize, share: &Scalar) -> bool {
        let lhs = RISTRETTO_BASEPOINT_POINT * share;
        let x = Scalar::from(index as u64);
        let mut rhs = RistrettoPoint::identity();
        let mut x_pow = Scalar::ONE;
        for commitment in &self.commitments {
            rhs += commitment * x_pow;
            x_pow *= x;
        }
        lhs == rhs
    }
}

/// Colector de complaints (Feldman VSS): conta complaints por dealer e decide a
/// desqualificação pela regra `count > threshold`.
pub struct ComplaintCollector {
    /// Threshold `t` do esquema; desqualificação quando `count > threshold`.
    threshold: usize,
    /// `dealer_id -> número de complaints` recebidos.
    complaints: HashMap<usize, usize>,
}

impl ComplaintCollector {
    /// Cria um colector vazio com o threshold do esquema.
    pub fn new(threshold: usize) -> Self {
        Self {
            threshold,
            complaints: HashMap::new(),
        }
    }

    /// Regista um complaint contra o dealer `dealer_id`.
    pub fn add_complaint(&mut self, dealer_id: usize) {
        *self.complaints.entry(dealer_id).or_insert(0) += 1;
    }

    /// Decide se o dealer está desqualificado (`count > threshold`).
    pub fn is_disqualified(&self, dealer_id: usize) -> bool {
        self.complaints.get(&dealer_id).copied().unwrap_or(0) > self.threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_honest_dealer_passes_verification() {
        let secret = Scalar::from(42u64);
        let dealer = FeldmanDealer::new(secret, 3, 5);
        let shares = dealer.honest_shares(5);

        for (i, share) in &shares {
            assert!(dealer.verify_share(*i, share), "Share {} falhou", i);
        }
    }

    #[test]
    fn test_malicious_dealer_detected() {
        let secret = Scalar::from(42u64);
        let dealer = FeldmanDealer::new(secret, 3, 5);
        let shares = dealer.malicious_shares(5, 3);

        let mut collector = ComplaintCollector::new(3);

        for (i, share) in &shares {
            if !dealer.verify_share(*i, share) {
                collector.add_complaint(0);
            }
        }

        assert!(!collector.is_disqualified(0));

        for _ in 0..3 {
            collector.add_complaint(0);
        }
        assert!(collector.is_disqualified(0));
    }

    #[test]
    fn test_reconstruction_with_honest_shares() {
        let secret = Scalar::from(42u64);
        let dealer = FeldmanDealer::new(secret, 3, 5);
        let shares = dealer.honest_shares(5);

        let recovered = lagrange_interpolate(&shares[0..3]);
        assert_eq!(recovered, secret);
    }

    fn lagrange_interpolate(shares: &[(usize, Scalar)]) -> Scalar {
        let mut secret = Scalar::ZERO;
        for (i, (xi, yi)) in shares.iter().enumerate() {
            let mut num = Scalar::ONE;
            let mut den = Scalar::ONE;
            for (j, (xj, _)) in shares.iter().enumerate() {
                if i != j {
                    num *= Scalar::from(*xj as u64);
                    den *= Scalar::from(*xj as u64) - Scalar::from(*xi as u64);
                }
            }
            secret += yi * num * den.invert();
        }
        secret
    }
}