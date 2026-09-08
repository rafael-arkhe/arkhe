//! Prolog bridge — Full integration with Scryer Prolog and RSI engine.
//!
//! This bridge connects the SafeManifold Rust types to an embedded Scryer
//! Prolog engine, enabling invariant checking via evolved rule sets and
//! recursive self-improvement (RSI).
//!
//! **Important**: `Machine` is `!Send` + `!Sync`, so `PrologBridge` and
//! `PrologClient` are single-threaded. For multi-threaded access, wrap
//! at a higher level.
//!
//! **Module stripping**: Scryer Prolog's `run_query` has bugs with
//! module-qualified calls (`module:goal`). To work around this, we strip
//! `:- module(...)` and `:- use_module(...)` directives at load time, and
//! call all predicates unqualified. The `.pl` files remain valid ISO Prolog.

use scryer_prolog::{LeafAnswer, Machine, MachineBuilder};

use crate::invariants::SystemState;
use thiserror::Error;

/// Errors from the Prolog bridge.
#[derive(Error, Debug)]
pub enum PrologError {
    /// Failed to initialize the Prolog engine.
    #[error("Prolog init failed: {0}")]
    Init(String),
    /// A Prolog query failed or returned an exception.
    #[error("Query failed: {0}")]
    Query(String),
    /// Could not parse a Prolog term as the expected Rust type.
    #[error("Type mismatch: {0}")]
    TypeMismatch(String),
    /// The RSI loop did not converge within the step limit.
    #[error("RSI did not converge after {0} steps")]
    RsiNotConverged(usize),
}

// ═════════════════════════════════════════════════════════════════════════════
// PrologBridge — owns the Scryer Machine
// ═════════════════════════════════════════════════════════════════════════════

/// Bridge to the embedded Scryer Prolog engine.
///
/// Not `Send` or `Sync` (Scryer's `Machine` is neither).
pub struct PrologBridge {
    machine: Machine,
}

/// Drain an iterator of `Result<LeafAnswer, Term>`, collecting only the
/// successful `LeafAnswer` values.
///
/// **IMPORTANT**: Scryer's `QueryState::drop` unconditionally calls
/// `trust_me()` which corrupts the trail for subsequent queries. We use
/// `mem::forget` to prevent this buggy drop after consuming all answers.
fn drain_answers(mut iter: impl Iterator<Item = Result<LeafAnswer, scryer_prolog::Term>>) -> Vec<LeafAnswer> {
    let mut results = Vec::new();
    loop {
        match iter.next() {
            Some(Ok(answer)) => results.push(answer),
            Some(Err(_)) => {}
            None => break,
        }
    }
    // Prevent QueryState::drop from corrupting internal state.
    // This leaks the QueryState (~64 bytes) which is negligible.
    std::mem::forget(iter);
    results
}

/// Strip Scryer module/dynamic directives that are incompatible with
/// `load_module_string` + `run_query`.
///
/// `:- dynamic` poisons the entire `load_module_string` call: predicates
/// loaded alongside a `:- dynamic` directive become invisible to subsequent
/// `run_query` calls. We strip all three directive types and rely on
/// `run_query("assertz(...)")` to implicitly create dynamic predicates.
///
/// Removes:
/// - `:- module(Name, [Exports...]).`
/// - `:- use_module(Name, [Imports...]).`
/// - `:- dynamic Name/Arity.`
///
/// Keeps all other directives and clause definitions.
fn strip_module_directives(src: &str) -> String {
    let mut result = String::with_capacity(src.len());
    let mut skip_until_period = false;

    for line in src.lines() {
        let trimmed = line.trim();
        if skip_until_period {
            if trimmed.ends_with('.') {
                skip_until_period = false;
            }
            continue;
        }
        if trimmed.starts_with(":- module(")
            || trimmed.starts_with(":- use_module(")
            || trimmed.starts_with(":- dynamic ")
        {
            // Check if the directive (before any % comment) ends with '.'
            let before_comment = trimmed.find('%').map(|i| &trimmed[..i]).unwrap_or(trimmed);
            if !before_comment.trim_end().ends_with('.') {
                skip_until_period = true;
            }
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}

impl PrologBridge {
    /// Create a new bridge, loading the base rules and RSI module.
    ///
    /// Module directives are stripped so all predicates are in the flat
    /// `user` namespace and accessible via unqualified calls.
    /// `init_defaults` is executed as part of the source load.
    pub fn new(rules_src: &str, rsi_src: &str) -> Result<Self, PrologError> {
        let mut machine = MachineBuilder::new().build();

        let clean_rules = strip_module_directives(rules_src);
        let clean_rsi = strip_module_directives(rsi_src);

        // Load predicate definitions (without init_defaults in source —
        // appending "init_defaults." to the source is parsed as a fact, not
        // executed as a goal).
        let combined = format!("{}\n{}", clean_rules, clean_rsi);
        machine.load_module_string("arkhe", combined);

        // Execute init_defaults as a goal via run_query.
        // drain_answers uses mem::forget to prevent QueryState::drop from
        // corrupting trail entries from assertz.
        let _ = drain_answers(machine.run_query("init_defaults.".to_string()));

        // Initialize RSI engine state (best_score, improvement_count).
        let _ = drain_answers(machine.run_query("assertz(best_score(0.0)).".to_string()));
        let _ = drain_answers(machine.run_query("assertz(improvement_count(0)).".to_string()));

        Ok(Self { machine })
    }

    /// Convert a `SystemState` to a Prolog term string (16 fields).
    pub fn state_to_term(state: &SystemState) -> String {
        format!(
            "state({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
            state.token_budget,
            state.agent_count,
            state.sandbox_fuel,
            state.entropy_bits,
            state.pii_scrubbed,
            state.signature_valid,
            state.rate_limit_remaining,
            state.model_capability,
            state.pqc_signature_valid,
            state.sbom_verified,
            state.model_hash_validated,
            state.audit_trail_complete,
            state.supply_chain_integrity,
            state.provenance_attested,
            state.bias_score,
            state.explainability_score,
        )
    }

    /// Run a query and return `true` if at least one `True` answer was produced.
    pub fn query_bool(&mut self, goal: &str) -> Result<bool, PrologError> {
        let query = format!("{}.", goal);
        let results = drain_answers(self.machine.run_query(query));
        Ok(results.iter().any(|a| matches!(a, LeafAnswer::True)))
    }

    /// Run a query and collect all leaf answers.
    pub fn query_answers(&mut self, goal: &str) -> Result<Vec<LeafAnswer>, PrologError> {
        let query = format!("{}.", goal);
        Ok(drain_answers(self.machine.run_query(query)))
    }

    // ── Public API ────────────────────────────────────────────────────────

    /// Check whether a state satisfies all active Prolog safety rules.
    pub fn check_invariants(&mut self, state: &SystemState) -> Result<bool, PrologError> {
        let term = Self::state_to_term(state);
        let goal = format!("safe_state({})", term);
        self.query_bool(&goal)
    }

    /// Initialize default active checks in the Prolog engine.
    pub fn init_defaults(&mut self) -> Result<(), PrologError> {
        self.query_bool("init_defaults")?;
        Ok(())
    }

    /// Run one RSI step: generate improvements, apply the first safe one.
    pub fn rsi_step(&mut self, state: &SystemState) -> Result<SystemState, PrologError> {
        let term = Self::state_to_term(state);
        let goal = format!("rsi_step({}, NewState)", term);
        let answers = self.query_answers(&goal)?;

        // RSI step modifies rules, not state. The state is passed through.
        if !answers.is_empty() || self.query_bool("converged")? {
            Ok(state.clone())
        } else {
            Err(PrologError::Query("rsi_step failed".to_string()))
        }
    }

    /// Run the RSI loop until convergence or `max_steps`.
    pub fn rsi_loop(
        &mut self,
        state: &SystemState,
        max_steps: usize,
    ) -> Result<SystemState, PrologError> {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        for _step in 0..max_steps {
            if self.query_bool("converged")? {
                return Ok(state.clone());
            }

            let snapshot = self.check_snapshot()?;
            if !seen.insert(snapshot) {
                return Ok(state.clone());
            }

            let _ = self.rsi_step(state)?;
        }
        if self.query_bool("converged")? {
            Ok(state.clone())
        } else {
            Err(PrologError::RsiNotConverged(max_steps))
        }
    }

    fn check_snapshot(&mut self) -> Result<String, PrologError> {
        let checks = self.list_active_checks()?;
        let mut pairs: Vec<_> = checks;
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(format!("{:?}", pairs))
    }

    /// Check that constitutional safeguards (I-05, I-06) are still active.
    pub fn check_constitutional_safeguards(&mut self) -> Result<bool, PrologError> {
        self.query_bool("constitutional_ok")
    }

    /// Rollback the last rule change.
    pub fn rollback_last(&mut self) -> Result<(), PrologError> {
        self.query_bool("rollback_last")?;
        Ok(())
    }

    /// List all currently active Prolog checks as (Id, Priority) pairs.
    pub fn list_active_checks(&mut self) -> Result<Vec<(String, u32)>, PrologError> {
        let answers = self.query_answers("active_check(Id, Pri)")?;
        let mut checks = Vec::new();
        for answer in &answers {
            if let Some((id, pri)) = extract_check_pair(answer) {
                checks.push((id, pri));
            }
        }
        Ok(checks)
    }

    /// Get missing invariant IDs (not covered by active checks).
    pub fn missing_invariants(&mut self) -> Result<Vec<String>, PrologError> {
        let answers = self.query_answers("missing_invariants(Missing)")?;
        let mut missing = Vec::new();
        for answer in &answers {
            if let Some(ids) = extract_atom_list(answer) {
                missing.extend(ids);
            }
        }
        Ok(missing)
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Helpers for extracting data from LeafAnswer (via Debug representation)
// ═════════════════════════════════════════════════════════════════════════════

/// Extract an atom value for a given key from a LeafAnswer Debug string.
///
/// Parses patterns like `"Id": Atom("i01")` from the BTreeMap Debug output.
fn extract_binding_atom(debug: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\": Atom(\"", key);
    let start = debug.find(&needle)? + needle.len();
    let rest = &debug[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Extract an integer value for a given key from a LeafAnswer Debug string.
///
/// Parses patterns like `"Pri": Integer(10)` from the BTreeMap Debug output.
fn extract_binding_integer(debug: &str, key: &str) -> Option<i64> {
    let needle = format!("\"{}\": Integer(", key);
    let start = debug.find(&needle)? + needle.len();
    let rest = &debug[start..];
    let end = rest.find(')')?;
    rest[..end].parse().ok()
}

fn extract_check_pair(answer: &LeafAnswer) -> Option<(String, u32)> {
    let debug = format!("{:?}", answer);
    let id = extract_binding_atom(&debug, "Id")?;
    let pri = extract_binding_integer(&debug, "Pri")?;
    Some((id, pri as u32))
}

fn extract_atom_list(answer: &LeafAnswer) -> Option<Vec<String>> {
    let debug = format!("{:?}", answer);
    if let Some(start) = debug.find('[') {
        if let Some(end) = debug.find(']') {
            let inner = &debug[start + 1..end];
            let ids: Vec<String> = inner
                .split(',')
                .map(|s| s.trim().trim_matches('\'').trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            return Some(ids);
        }
    }
    None
}

// ═════════════════════════════════════════════════════════════════════════════
// PrologClient — high-level wrapper
// ═════════════════════════════════════════════════════════════════════════════

/// High-level Prolog client for the SafeManifold RSI engine.
///
/// Not `Send` or `Sync` (Scryer's `Machine` is neither).
pub struct PrologClient {
    bridge: PrologBridge,
}

impl PrologClient {
    /// Create a new client with the embedded rules and RSI module.
    pub fn new(rules_src: &str, rsi_src: &str) -> Result<Self, PrologError> {
        let bridge = PrologBridge::new(rules_src, rsi_src)?;
        Ok(Self { bridge })
    }

    /// Check invariants via Prolog.
    pub fn check_invariants(&mut self, state: &SystemState) -> Result<bool, PrologError> {
        self.bridge.check_invariants(state)
    }

    /// Run one RSI step.
    pub fn rsi_step(&mut self, state: &SystemState) -> Result<SystemState, PrologError> {
        self.bridge.rsi_step(state)
    }

    /// Run full RSI loop.
    pub fn rsi_loop(
        &mut self,
        state: &SystemState,
        max_steps: usize,
    ) -> Result<SystemState, PrologError> {
        self.bridge.rsi_loop(state, max_steps)
    }

    /// Check constitutional safeguards.
    pub fn check_constitutional_safeguards(&mut self) -> Result<bool, PrologError> {
        self.bridge.check_constitutional_safeguards()
    }

    /// Rollback last rule change.
    pub fn rollback_last(&mut self) -> Result<(), PrologError> {
        self.bridge.rollback_last()
    }

    /// List active checks.
    pub fn list_active_checks(&mut self) -> Result<Vec<(String, u32)>, PrologError> {
        self.bridge.list_active_checks()
    }

    /// Get missing invariants.
    pub fn missing_invariants(&mut self) -> Result<Vec<String>, PrologError> {
        self.bridge.missing_invariants()
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Embedded Prolog source constants
// ═════════════════════════════════════════════════════════════════════════════

/// Base safety rules (from `src/prolog/rules.pl`).
pub const RULES_SRC: &str = include_str!("prolog/rules.pl");

/// RSI engine (from `src/prolog/rsi.prolog`).
pub const RSI_SRC: &str = include_str!("prolog/rsi.prolog");

// ═════════════════════════════════════════════════════════════════════════════
// ScryerBackend — implements PrologBackend trait over real Scryer Prolog
// ═════════════════════════════════════════════════════════════════════════════

use crate::prolog_backend::PrologBackend;

/// Backend that delegates to a real embedded Scryer Prolog engine.
///
/// Wraps [`PrologBridge`] and implements [`PrologBackend`] so callers
/// can swap between mock and real engines at compile time.
pub struct ScryerBackend {
    bridge: PrologBridge,
}

impl ScryerBackend {
    /// Create a new Scryer backend, loading the given rules and RSI sources.
    pub fn new(rules_src: &str, rsi_src: &str) -> Result<Self, PrologError> {
        let bridge = PrologBridge::new(rules_src, rsi_src)?;
        Ok(Self { bridge })
    }

    /// Access the inner [`PrologBridge`] for low-level operations.
    pub fn inner(&mut self) -> &mut PrologBridge {
        &mut self.bridge
    }
}

impl PrologBackend for ScryerBackend {
    fn check_invariants(&mut self, state: &SystemState) -> Result<bool, PrologError> {
        self.bridge.check_invariants(state)
    }

    fn list_active_checks(&mut self) -> Result<Vec<(String, u32)>, PrologError> {
        self.bridge.list_active_checks()
    }

    fn check_constitutional_safeguards(&mut self) -> Result<bool, PrologError> {
        self.bridge.check_constitutional_safeguards()
    }

    fn rollback_last(&mut self) -> Result<(), PrologError> {
        self.bridge.rollback_last()
    }

    fn rsi_step(&mut self, state: &SystemState) -> Result<SystemState, PrologError> {
        self.bridge.rsi_step(state)
    }

    fn rsi_loop(
        &mut self,
        state: &SystemState,
        max_steps: usize,
    ) -> Result<SystemState, PrologError> {
        self.bridge.rsi_loop(state, max_steps)
    }

    fn missing_invariants(&mut self) -> Result<Vec<String>, PrologError> {
        self.bridge.missing_invariants()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::SystemConfig;

    fn make_bridge() -> PrologBridge {
        PrologBridge::new(RULES_SRC, RSI_SRC).expect("failed to create PrologBridge")
    }

    #[test]
    fn test_minimal_prolog() {
        // Test 1: bare facts with load_module_string
        let mut m1 = MachineBuilder::new().build();
        m1.load_module_string("test", "hello(world).".to_string());
        let r1: Vec<_> = drain_answers(m1.run_query("hello(world).".to_string()));
        eprintln!("Test1 (bare fact): {} answers, any true: {}", r1.len(), r1.iter().any(|a| matches!(a, LeafAnswer::True)));

        // Test 2: dynamic with assertz via run_query
        let mut m2 = MachineBuilder::new().build();
        m2.load_module_string("test2", ":- dynamic db/1.".to_string());
        let r2a: Vec<_> = drain_answers(m2.run_query("assertz(db(hello)).".to_string()));
        eprintln!("Test2a (assertz): {} answers", r2a.len());
        let r2b: Vec<_> = drain_answers(m2.run_query("db(X).".to_string()));
        eprintln!("Test2b (db query): {} answers", r2b.len());
        for a in &r2b {
            eprintln!("  {:?}", a);
        }

        // Test 3: dynamic defined in source, assertz in run_query
        let mut m3 = MachineBuilder::new().build();
        m3.load_module_string("test3", ":- dynamic db/1. db初始(preset).".to_string());
        let r3a: Vec<_> = drain_answers(m3.run_query("db(X).".to_string()));
        eprintln!("Test3a (preset db): {} answers", r3a.len());
        let r3b: Vec<_> = drain_answers(m3.run_query("assertz(db(new)).".to_string()));
        eprintln!("Test3b (assertz): {} answers", r3b.len());
        let r3c: Vec<_> = drain_answers(m3.run_query("db(X).".to_string()));
        eprintln!("Test3c (after assertz): {} answers", r3c.len());
        for a in &r3c {
            eprintln!("  {:?}", a);
        }
    }

    #[test]
    fn test_bridge_init_diagnostics() {
        // Test polyfills in isolation
        let mut m0 = MachineBuilder::new().build();
        let polyfills = r#"
member(X, [X|_]).
member(X, [_|T]) :- member(X, T).
exclude(_, [], []).
exclude(Goal, [H|T], R) :- call(Goal, H) -> exclude(Goal, T, R) ; R = [H|R1], exclude(Goal, T, R1).
forall(Cond, Action) :- \+ (Cond, \+ Action).
"#;
        m0.load_module_string("test", polyfills.to_string());

        let r0a: Vec<_> = drain_answers(m0.run_query("member(A, [a,b,c]).".to_string()));
        eprintln!("[poly] member: {} answers", r0a.len());

        let r0b: Vec<_> = drain_answers(m0.run_query("exclude(=(b), [a,b,c], L).".to_string()));
        eprintln!("[poly] exclude(=(b),[a,b,c],L): {} answers", r0b.len());
        for a in &r0b { eprintln!("  {:?}", a); }

        let r0c: Vec<_> = drain_answers(m0.run_query("forall(member(A,[a,b,c]), A \\= z).".to_string()));
        eprintln!("[poly] forall: {} answers", r0c.len());
        for a in &r0c { eprintln!("  {:?}", a); }

        // Test with full rules + rsi
        let mut m = MachineBuilder::new().build();
        let clean_rules = strip_module_directives(RULES_SRC);
        let clean_rsi = strip_module_directives(RSI_SRC);
        let combined = format!("{}\n{}", clean_rules, clean_rsi);
        m.load_module_string("arkhe", combined);

        let r1: Vec<_> = drain_answers(m.run_query("init_defaults.".to_string()));
        eprintln!("[full] init_defaults: {} answers", r1.len());

        let r2: Vec<_> = drain_answers(m.run_query("constitutional_ok.".to_string()));
        eprintln!("[full] constitutional_ok: {} answers", r2.len());

        let r3: Vec<_> = drain_answers(m.run_query("no_infinite_recursion.".to_string()));
        eprintln!("[full] no_infinite_recursion: {} answers", r3.len());
        for a in &r3 { eprintln!("  {:?}", a); }

        let r4: Vec<_> = drain_answers(m.run_query("findall(Imp, generate_one(state(10,5,100,256,true,true,50,4294967296), Imp), L).".to_string()));
        eprintln!("[full] generate_one: {} answers", r4.len());
        for a in &r4 { eprintln!("  {:?}", a); }

        // Debug: get_coverage step by step
        // Debug measure_performance step by step
        let state = PrologBridge::state_to_term(&SystemState::safe(SystemConfig::default()));

        let r1: Vec<_> = drain_answers(m.run_query(format!("get_coverage({}, C).", state)));
        eprintln!("[mp] get_coverage: {} answers", r1.len());

        let r2: Vec<_> = drain_answers(m.run_query("length([i01,i02,i03,i04,i05,i06,i07,i08], N).".to_string()));
        eprintln!("[mp] length: {} answers, {:?}", r2.len(), r2);

        let r3: Vec<_> = drain_answers(m.run_query("Score is (8 / 8) * 70 + 30.".to_string()));
        eprintln!("[mp] Score is: {} answers, {:?}", r3.len(), r3);

        let r4: Vec<_> = drain_answers(m.run_query("best_score(B).".to_string()));
        eprintln!("[mp] best_score: {} answers", r4.len());

        let r5: Vec<_> = drain_answers(m.run_query("assertz(best_score(0.0)).".to_string()));
        eprintln!("[mp] assertz best_score: {} answers", r5.len());

        let r6: Vec<_> = drain_answers(m.run_query("best_score(B).".to_string()));
        eprintln!("[mp] best_score after: {} answers, {:?}", r6.len(), r6);

        let r7: Vec<_> = drain_answers(m.run_query(format!("measure_performance({}, S).", state)));
        eprintln!("[mp] measure_performance: {} answers, {:?}", r7.len(), r7);

        // Debug: test assertz of best_score
        let r_bs1: Vec<_> = drain_answers(m.run_query("best_score(B).".to_string()));
        eprintln!("[full] best_score before: {} answers", r_bs1.len());
        for a in &r_bs1 { eprintln!("  {:?}", a); }

        let r_bs2: Vec<_> = drain_answers(m.run_query("assertz(best_score(0.0)).".to_string()));
        eprintln!("[full] assertz best_score: {} answers", r_bs2.len());
        for a in &r_bs2 { eprintln!("  {:?}", a); }

        let r_bs3: Vec<_> = drain_answers(m.run_query("best_score(B).".to_string()));
        eprintln!("[full] best_score after assertz: {} answers", r_bs3.len());
        for a in &r_bs3 { eprintln!("  {:?}", a); }

        // Debug: test assertz + retract cycle
        let r_ret: Vec<_> = drain_answers(m.run_query("retract(best_score(X)).".to_string()));
        eprintln!("[full] retract best_score: {} answers", r_ret.len());
        for a in &r_ret { eprintln!("  {:?}", a); }

        let r_bs4: Vec<_> = drain_answers(m.run_query("best_score(B).".to_string()));
        eprintln!("[full] best_score after retract: {} answers", r_bs4.len());

        // Check if best_score is defined at all
        let r_bs5: Vec<_> = drain_answers(m.run_query("current_predicate(best_score/1).".to_string()));
        eprintln!("[full] current_predicate(best_score/1): {} answers", r_bs5.len());
        for a in &r_bs5 { eprintln!("  {:?}", a); }

        let r5d: Vec<_> = drain_answers(m.run_query("safety_budget_ok(100).".to_string()));
        eprintln!("[full] safety_budget_ok(100): {} answers", r5d.len());
        for a in &r5d { eprintln!("  {:?}", a); }

        let r5e: Vec<_> = drain_answers(m.run_query("validate_rules(state(10,5,100,256,true,true,50,4294967296), 100).".to_string()));
        eprintln!("[full] validate_rules: {} answers", r5e.len());
        for a in &r5e { eprintln!("  {:?}", a); }

        let r5: Vec<_> = drain_answers(m.run_query(format!("rsi_step({}, Out).", state)));
        eprintln!("[full] rsi_step: {} answers", r5.len());
        for a in &r5 { eprintln!("  {:?}", a); }
    }

    #[test]
    fn test_strip_module_directives() {
        let input = r#"
:- module(rules, [
    safe_state/1,
    check_invariant/2,
]).
:- dynamic active_check/2.
:- use_module(other, [foo/1]).
safe_state(X) :- true.
"#;
        let cleaned = strip_module_directives(input);
        assert!(!cleaned.contains(":- module("));
        assert!(!cleaned.contains(":- dynamic "));
        assert!(!cleaned.contains(":- use_module("));
        assert!(cleaned.contains("safe_state(X) :- true."));
    }

    #[test]
    fn test_bridge_initializes() {
        let _bridge = make_bridge();
    }

    #[test]
    fn test_safe_state_accepted() {
        let mut bridge = make_bridge();
        let config = SystemConfig::default();
        let state = SystemState::safe(config);
        assert!(bridge.check_invariants(&state).unwrap());
    }

    #[test]
    fn test_unsafe_state_rejected() {
        let mut bridge = make_bridge();
        let config = SystemConfig::default();
        let mut state = SystemState::safe(config);
        state.token_budget = -1;
        assert!(!bridge.check_invariants(&state).unwrap());
    }

    #[test]
    fn test_constitutional_safeguards_initial() {
        let mut bridge = make_bridge();
        assert!(bridge.check_constitutional_safeguards().unwrap());
    }

    #[test]
    fn test_list_active_checks() {
        let mut bridge = make_bridge();
        let checks = bridge.list_active_checks().unwrap();
        assert_eq!(checks.len(), 16);
        let pii = checks.iter().find(|(id, _)| id == "i05");
        assert!(pii.is_some());
        assert_eq!(pii.unwrap().1, 100);
    }

    #[test]
    fn test_rsi_step_preserves_invariants() {
        let mut bridge = make_bridge();
        let config = SystemConfig::default();
        let state = SystemState::safe(config);

        assert!(bridge.check_invariants(&state).unwrap());
        let new_state = bridge.rsi_step(&state).unwrap();
        assert!(bridge.check_invariants(&state).unwrap());
        assert_eq!(state, new_state);
    }

    #[test]
    fn test_rsi_step_preserves_constitutional_safeguards() {
        let mut bridge = make_bridge();
        let config = SystemConfig::default();
        let state = SystemState::safe(config);

        for _ in 0..5 {
            let _ = bridge.rsi_step(&state).unwrap();
            assert!(
                bridge.check_constitutional_safeguards().unwrap(),
                "Constitutional safeguards violated after RSI step"
            );
        }
    }

    #[test]
    fn test_rsi_loop_converges() {
        let mut bridge = make_bridge();
        let config = SystemConfig::default();
        let state = SystemState::safe(config);

        let result = bridge.rsi_loop(&state, 200);
        assert!(result.is_ok());
        assert!(bridge.check_invariants(&state).unwrap());
    }

    #[test]
    fn test_rollback_restores_rules() {
        let mut bridge = make_bridge();
        let config = SystemConfig::default();
        let state = SystemState::safe(config);

        let checks_before = bridge.list_active_checks().unwrap();
        let _ = bridge.rsi_step(&state);
        let _ = bridge.rollback_last();
        let checks_after = bridge.list_active_checks().unwrap();

        assert_eq!(checks_before.len(), checks_after.len());
    }

    #[test]
    fn test_state_to_term_format() {
        let config = SystemConfig::default();
        let state = SystemState::safe(config);
        let term = PrologBridge::state_to_term(&state);
        assert!(term.starts_with("state("));
        assert!(term.ends_with(')'));
    }
}
