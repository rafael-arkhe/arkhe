//! Backend abstraction for Prolog-based invariant checking.

use std::collections::BTreeMap;
use crate::invariants::SystemState;
use crate::prolog_bridge::PrologError;

/// Trait for backends that can check Prolog safety rules.
pub trait PrologBackend {
    fn check_invariants(&mut self, state: &SystemState) -> Result<bool, PrologError>;
    fn list_active_checks(&mut self) -> Result<Vec<(String, u32)>, PrologError>;
    fn check_constitutional_safeguards(&mut self) -> Result<bool, PrologError>;
    fn rollback_last(&mut self) -> Result<(), PrologError>;
    fn rsi_step(&mut self, state: &SystemState) -> Result<SystemState, PrologError>;
    fn rsi_loop(&mut self, state: &SystemState, max_steps: usize) -> Result<SystemState, PrologError>;
    fn missing_invariants(&mut self) -> Result<Vec<String>, PrologError>;
}

const ALL_INVARIANT_IDS: &[&str] = &[
    "i01", "i02", "i03", "i04", "i05", "i06", "i07", "i08",
    "i09", "i10", "i11", "i12", "i13", "i14", "i15", "i16",
];

const DEFAULT_PRIORITY: u32 = 10;
const CRITICAL_PRIORITY: u32 = 100;

/// Pure-Rust Prolog backend for testing (16 invariants).
#[derive(Clone)]
pub struct MockProlog {
    active_checks: BTreeMap<String, u32>,
    backup: Option<BTreeMap<String, u32>>,
    best_score: f64,
    improvement_count: usize,
}

impl Default for MockProlog {
    fn default() -> Self { Self::new() }
}

impl MockProlog {
    pub fn new() -> Self {
        let mut active_checks = BTreeMap::new();
        for id in ALL_INVARIANT_IDS {
            let priority = if matches!(*id, "i05" | "i06" | "i09" | "i13") {
                CRITICAL_PRIORITY
            } else {
                DEFAULT_PRIORITY
            };
            active_checks.insert(id.to_string(), priority);
        }
        Self { active_checks, backup: None, best_score: 0.0, improvement_count: 0 }
    }

    pub fn empty() -> Self {
        Self { active_checks: BTreeMap::new(), backup: None, best_score: 0.0, improvement_count: 0 }
    }

    pub fn add_check(&mut self, id: impl Into<String>, priority: u32) {
        self.active_checks.insert(id.into(), priority);
    }

    pub fn remove_check(&mut self, id: &str) -> Option<u32> {
        self.active_checks.remove(id)
    }

    pub fn best_score(&self) -> f64 { self.best_score }
    pub fn improvement_count(&self) -> usize { self.improvement_count }
    pub fn has_rule(&self, id: &str) -> bool { self.active_checks.contains_key(id) }
    pub fn add_rule(&mut self, id: impl Into<String>, priority: u32) {
        self.active_checks.insert(id.into(), priority);
    }

    pub fn get_rules_snapshot(&self) -> String {
        let mut pairs: Vec<_> = self.active_checks.iter().collect();
        pairs.sort_by_key(|(k, _)| (*k).clone());
        format!("{:?}", pairs)
    }

    pub fn query_bool(&mut self, _goal: &str, state: &SystemState) -> bool {
        state.check_all() && !self.active_checks.is_empty()
    }

    pub fn query_count(&self) -> usize { self.active_checks.len() }

    pub fn run_rsi_step(&mut self, state: &SystemState) -> Result<SystemState, PrologError> {
        self.rsi_step(state)
    }

    pub fn run_rsi_loop(&mut self, state: &SystemState, max_steps: usize) -> Result<SystemState, PrologError> {
        self.rsi_loop(state, max_steps)
    }
}

impl PrologBackend for MockProlog {
    fn check_invariants(&mut self, state: &SystemState) -> Result<bool, PrologError> {
        if self.active_checks.is_empty() { return Ok(true); }
        Ok(state.check_all())
    }

    fn list_active_checks(&mut self) -> Result<Vec<(String, u32)>, PrologError> {
        Ok(self.active_checks.iter().map(|(k, &v)| (k.clone(), v)).collect())
    }

    fn check_constitutional_safeguards(&mut self) -> Result<bool, PrologError> {
        let critical_ids = ["i05", "i06", "i09", "i13"];
        Ok(critical_ids.iter().all(|id| {
            self.active_checks.contains_key(*id)
                && self.active_checks.get(*id).copied().unwrap_or(0) >= CRITICAL_PRIORITY
        }))
    }

    fn rollback_last(&mut self) -> Result<(), PrologError> {
        if let Some(prev) = self.backup.take() {
            self.active_checks = prev;
        }
        Ok(())
    }

    fn rsi_step(&mut self, state: &SystemState) -> Result<SystemState, PrologError> {
        self.improvement_count += 1;
        let score = if state.check_all() { 100.0 } else { 0.0 };
        if score > self.best_score { self.best_score = score; }
        Ok(state.clone())
    }

    fn rsi_loop(&mut self, state: &SystemState, max_steps: usize) -> Result<SystemState, PrologError> {
        for _ in 0..max_steps { let _ = self.rsi_step(state)?; }
        Ok(state.clone())
    }

    fn missing_invariants(&mut self) -> Result<Vec<String>, PrologError> {
        Ok(ALL_INVARIANT_IDS
            .iter()
            .filter(|&&id| !self.active_checks.contains_key(id))
            .map(|id| (*id).to_string())
            .collect())
    }
}
