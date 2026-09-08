//! MSSP Bridge — Minimum Sufficient Subset of Predicates.

use std::collections::{BTreeMap, HashSet};
use crate::invariants::SystemState;

/// An external invariant source.
pub trait Provider {
    fn name(&self) -> &str;
    fn offered_ids(&self) -> Vec<String>;
    fn evaluate(&self, state: &SystemState) -> bool;
    fn priority_hint(&self) -> u32 { 10 }
}

/// Stabilization metrics for the MSSP bridge.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StabilizationMetrics {
    pub accepted: usize,
    pub deferred: usize,
    pub denied: usize,
    pub providers_queried: usize,
}

/// A mock provider for testing.
pub struct MockProvider {
    name: String,
    ids: Vec<String>,
    pass: bool,
}

impl MockProvider {
    pub fn new(name: impl Into<String>, ids: Vec<String>, pass: bool) -> Self {
        Self { name: name.into(), ids, pass }
    }
}

impl Provider for MockProvider {
    fn name(&self) -> &str { &self.name }
    fn offered_ids(&self) -> Vec<String> { self.ids.clone() }
    fn evaluate(&self, _state: &SystemState) -> bool { self.pass }
}

/// MSSP Bridge — manages multiple providers and tracks stabilization.
pub struct MsspBridge {
    providers: Vec<Box<dyn Provider>>,
    active_rules: BTreeMap<String, u32>,
    already_active: HashSet<String>,
}

impl MsspBridge {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            active_rules: BTreeMap::new(),
            already_active: HashSet::new(),
        }
    }

    pub fn with_active_rules(rules: BTreeMap<String, u32>) -> Self {
        let already_active = rules.keys().cloned().collect();
        Self { providers: Vec::new(), active_rules: rules, already_active }
    }

    pub fn register(&mut self, provider: Box<dyn Provider>) {
        self.providers.push(provider);
    }

    pub fn evaluate(&self, state: &SystemState) -> StabilizationMetrics {
        let mut metrics = StabilizationMetrics {
            providers_queried: self.providers.len(),
            ..Default::default()
        };
        for provider in &self.providers {
            let offered = provider.offered_ids();
            let passed = provider.evaluate(state);
            for id in &offered {
                if self.already_active.contains(id) { continue; }
                if passed { metrics.accepted += 1; } else { metrics.denied += 1; }
            }
        }
        metrics
    }

    pub fn evaluate_and_activate(&mut self, state: &SystemState) -> StabilizationMetrics {
        let mut metrics = StabilizationMetrics {
            providers_queried: self.providers.len(),
            ..Default::default()
        };
        for provider in &self.providers {
            let offered = provider.offered_ids();
            let passed = provider.evaluate(state);
            let priority = provider.priority_hint();
            for id in offered {
                if self.already_active.contains(&id) { continue; }
                if passed {
                    self.active_rules.insert(id.clone(), priority);
                    self.already_active.insert(id);
                    metrics.accepted += 1;
                } else {
                    metrics.deferred += 1;
                }
            }
        }
        metrics
    }

    pub fn active_rules(&self) -> &BTreeMap<String, u32> {
        &self.active_rules
    }

    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    pub fn minimum_sufficient_subset(&self) -> Vec<String> {
        let mut covered = HashSet::new();
        let mut selected = Vec::new();
        let mut rules: Vec<_> = self.active_rules.iter().collect();
        rules.sort_by(|a, b| b.1.cmp(a.1));
        for (id, _pri) in rules {
            if !covered.contains(id) {
                covered.insert(id.clone());
                selected.push(id.clone());
            }
        }
        selected
    }
}

impl Default for MsspBridge {
    fn default() -> Self { Self::new() }
}
