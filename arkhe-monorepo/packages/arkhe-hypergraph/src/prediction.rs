//! Numerai-style prediction-and-reputation layer for the hypergraph (S03 lane).
//!
//! # Honesty notes
//! The proposal's `linfa`/`burn`/`tch-rs`/`lightgbm`/`xgboost` pipelines drag in
//! native C++/CUDA toolchains and are **not** part of this crate's pure-Rust,
//! no-C dependency floor; they are not wired in here. What IS implemented and
//! testable is the pure-math substrate those frameworks would sit on:
//!
//! - [`StakingManager`]: reward/burn staking that moves each agent's stake and
//!   focal influence by measured performance.
//! - [`correlation`]: Pearson correlation between agent predictions and field
//!   outcomes, with a NaN- and zero-variance guard (Numerai's CORR).
//! - [`MetaModel::consensus`]: a focal-weight-weighted combination of agent
//!   predictions (Numerai's Meta Model → Arkhe consensus).
//!
//! No statistical framing here is security-critical by itself: "staking
//! stabilizes Sybil resistance" is only true when the stake is a real,
//! collator-restricted, slashable asset — an entropy signal alone does not
//! stop Sybil (see the PoPD critique elsewhere in the repo).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::AgentId;

/// A stake ledger tracking each agent's nominal balance and focal influence.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct StakingManager {
    pub stakes: HashMap<AgentId, f64>,
    pub focal_weights: HashMap<AgentId, f64>,
    /// Fraction of predicted loss burned each bad era.
    pub burn_rate: f64,
    /// Reward multiplier applied to positive performance.
    pub reward_rate: f64,
    /// Floor below which a stake is not allowed to fall.
    pub min_stake: f64,
}

impl StakingManager {
    pub fn new(burn_rate: f64, reward_rate: f64, min_stake: f64) -> Self {
        Self {
            stakes: HashMap::new(),
            focal_weights: HashMap::new(),
            burn_rate,
            reward_rate,
            min_stake,
        }
    }

    pub fn register(&mut self, agent: AgentId) {
        self.stakes.entry(agent).or_insert(1.0);
        self.focal_weights.entry(agent).or_insert(agent.focal_weight());
    }

    /// Apply a performance score `perf` (positive = good) to an agent.
    pub fn update_stake(&mut self, agent: AgentId, performance: f64) {
        self.register(agent);
        let stake = self.stakes.get_mut(&agent).unwrap();
        if performance > 0.0 {
            *stake += performance * self.reward_rate;
        } else {
            *stake -= performance.abs() * self.burn_rate;
        }
        *stake = stake.max(self.min_stake);
        *self.focal_weights.get_mut(&agent).unwrap() =
            self.computed_focal(agent);
    }

    /// Default influence: clamped between the initial focal weight and a floor.
    pub fn computed_focal(&self, agent: AgentId) -> f64 {
        let base = agent.focal_weight();
        let stake = self.stakes.get(&agent).copied().unwrap_or(1.0);
        (base * stake).clamp(0.0, 1.0)
    }
}

/// Pearson correlation between prediction and outcome, with guards.
pub fn correlation(predictions: &[f64], actual: &[f64]) -> Option<f64> {
    if predictions.len() != actual.len() || predictions.len() < 2 {
        return None;
    }
    let n = predictions.len() as f64;
    let mean_p = predictions.iter().sum::<f64>() / n;
    let mean_a = actual.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den_p = 0.0;
    let mut den_a = 0.0;
    for (&p, &a) in predictions.iter().zip(actual.iter()) {
        let dp = p - mean_p;
        let da = a - mean_a;
        num += dp * da;
        den_p += dp * dp;
        den_a += da * da;
    }
    let den = (den_p * den_a).sqrt();
    if den <= f64::MIN_POSITIVE || !num.is_finite() {
        return None;
    }
    Some(num / den)
}

/// A single agent's prediction along some scalar trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    pub agent: AgentId,
    pub value: f64,
}

/// Focal-weighted combination of several predictions into a consensus value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaModel {
    pub predictions: Vec<Prediction>,
    pub weights: HashMap<AgentId, f64>,
}

impl MetaModel {
    pub fn new(weights: HashMap<AgentId, f64>) -> Self {
        Self { predictions: Vec::new(), weights }
    }

    pub fn add(&mut self, p: Prediction) {
        self.predictions.push(p);
    }

    /// Weighted average; agents we have no weight for contribute nothing.
    pub fn consensus(&self) -> Option<f64> {
        let mut weighted = 0.0;
        let mut total = 0.0;
        for p in &self.predictions {
            if let Some(w) = self.weights.get(&p.agent) {
                weighted += *w * p.value;
                total += *w;
            }
        }
        if total <= 0.0 {
            None
        } else {
            Some(weighted / total)
        }
    }
}

/// Shred the "MMC"-style unique contribution: how much the agent uniquely
/// adds versus the consensus, on average over `perf` measurements.
pub fn meta_contribution(agent_prediction: f64, consensus: f64, actual: f64) -> f64 {
    // Signed improvement signal: correct independent predictions land the
    // same sign as the outcome when they beat the consensus.
    (agent_prediction - consensus).signum() * actual
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stake_rewards_and_burns() {
        let mut sm = StakingManager::new(0.2, 0.1, 0.1);
        sm.register(AgentId::S01);
        sm.update_stake(AgentId::S01, 1.0);
        assert!((sm.stakes[&AgentId::S01] - 1.1).abs() < 1e-9);
        sm.update_stake(AgentId::S01, -2.0);
        assert!((sm.stakes[&AgentId::S01] - 0.7).abs() < 1e-9);
        // Burn can never push below the floor.
        for _ in 0..100 {
            sm.update_stake(AgentId::S01, -10.0);
        }
        assert!((sm.stakes[&AgentId::S01] - 0.1).abs() < 1e-9);
    }

    #[test]
    fn corr_perfect_and_nan_guard() {
        let p = [1.0, 2.0, 3.0, 4.0];
        assert!((correlation(&p, &p).unwrap() - 1.0).abs() < 1e-9);
        assert_eq!(correlation(&p, &[0.0, 0.0, 0.0, 0.0]), None, "zero variance -> None");
    }

    #[test]
    fn metamodel_weighted_consensus() {
        let mut weights = HashMap::new();
        weights.insert(AgentId::S01, 0.2);
        weights.insert(AgentId::S07, 0.8);
        let mut mm = MetaModel::new(weights);
        mm.add(Prediction { agent: AgentId::S01, value: 1.0 });
        mm.add(Prediction { agent: AgentId::S07, value: 2.0 });
        // 0.2*1 + 0.8*2 = 1.8
        assert!((mm.consensus().unwrap() - 1.8).abs() < 1e-9);
    }

    #[test]
    fn mm_empty_is_none() {
        let mm = MetaModel::new(HashMap::new());
        assert!(mm.consensus().is_none());
    }
}