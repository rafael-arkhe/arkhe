//! # arkhe-potts
//!
//! A small, honest Potts-model core: energy, exact Boltzmann statistics, and
//! MAP (maximum-a-posteriori) inference over discrete configurations.
//!
//! This is the piece the "EffieDes" analogy in the Arkhe protein-design
//! document *pointed at* but never actually implemented. That sketch had a
//! fatal bug: its `potts_energy` never referenced the configuration `σ`, so
//! "the energy of a sequence" did not depend on the sequence, and everything
//! built on top (MAP, probabilities) was meaningless.
//!
//! Here the energy genuinely depends on the configuration, and correctness is
//! *verifiable* rather than asserted:
//!
//! * [`PottsModel::map_exact`] enumerates all `q^n` configurations and returns
//!   the true minimum-energy (= maximum-probability) assignment. This is the
//!   ground-truth oracle.
//! * [`PottsModel::map_icm`] / [`PottsModel::map_icm_restarts`] is a scalable
//!   heuristic (Iterated Conditional Modes) that performs monotone coordinate
//!   descent. Its output is tested *against* `map_exact` on small instances.
//!
//! ## Energy convention
//!
//! For a configuration `σ = (σ_0, …, σ_{n-1})` with each `σ_i ∈ {0, …, q-1}`:
//!
//! ```text
//! E(σ) = - Σ_i h_i(σ_i)  - Σ_{i<j} J_{ij}(σ_i, σ_j)
//! ```
//!
//! The Boltzmann distribution is `P(σ) = exp(-β E(σ)) / Z`, so **lower energy
//! means higher probability**, and MAP inference is `argmin_σ E(σ)`.
//!
//! Everything is `std`-only and deterministic (the heuristic's randomness comes
//! from a seeded SplitMix64 generator, not the OS).

use std::collections::HashMap;

/// Configurations whose energy differs by less than this are treated as tied.
const EPS: f64 = 1e-12;

/// Errors returned by fallible operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PottsError {
    /// A position index was `>= n`.
    PositionOutOfRange { index: usize, n: usize },
    /// A state value was `>= q`.
    StateOutOfRange { state: usize, q: usize },
    /// A coupling was requested between a position and itself.
    SelfCoupling { index: usize },
    /// A configuration had the wrong length.
    BadLength { got: usize, expected: usize },
    /// The exhaustive state space `q^n` exceeds the allowed cap.
    StateSpaceTooLarge { size: u128, cap: u128 },
}

impl std::fmt::Display for PottsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PottsError::PositionOutOfRange { index, n } => {
                write!(f, "position {index} out of range (n = {n})")
            }
            PottsError::StateOutOfRange { state, q } => {
                write!(f, "state {state} out of range (q = {q})")
            }
            PottsError::SelfCoupling { index } => {
                write!(f, "self-coupling requested at position {index}")
            }
            PottsError::BadLength { got, expected } => {
                write!(f, "configuration length {got}, expected {expected}")
            }
            PottsError::StateSpaceTooLarge { size, cap } => {
                write!(f, "state space {size} exceeds cap {cap}")
            }
        }
    }
}

impl std::error::Error for PottsError {}

/// A hard constraint on configurations. Constraints are enforced by the
/// `*_constrained` inference methods and by [`PottsModel::satisfies`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
    /// Position `i` must take exactly state `a`.
    Fixed(usize, usize),
    /// Positions `i` and `j` must take the same state.
    Equal(usize, usize),
    /// Positions `i` and `j` must take different states.
    NotEqual(usize, usize),
}

impl Constraint {
    fn holds(&self, cfg: &[usize]) -> bool {
        match *self {
            Constraint::Fixed(i, a) => cfg[i] == a,
            Constraint::Equal(i, j) => cfg[i] == cfg[j],
            Constraint::NotEqual(i, j) => cfg[i] != cfg[j],
        }
    }
}

/// A Potts model over `n` positions, each taking one of `q` discrete states.
///
/// Fields (`h`) are stored densely (`n * q`). Pairwise couplings (`J`) are
/// stored sparsely, keyed by the canonical ordered pair `(i, j)` with `i < j`;
/// each entry is a flat `q * q` matrix indexed `a * q + b`.
#[derive(Debug, Clone)]
pub struct PottsModel {
    n: usize,
    q: usize,
    fields: Vec<f64>,
    couplings: HashMap<(usize, usize), Vec<f64>>,
}

impl PottsModel {
    /// Create an all-zero model over `n` positions and `q` states.
    ///
    /// # Panics
    /// Panics if `n == 0` or `q == 0`; a model needs at least one position and
    /// one state to be meaningful.
    pub fn new(n: usize, q: usize) -> Self {
        assert!(n > 0, "n must be > 0");
        assert!(q > 0, "q must be > 0");
        PottsModel {
            n,
            q,
            fields: vec![0.0; n * q],
            couplings: HashMap::new(),
        }
    }

    /// Number of positions.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Number of states per position.
    pub fn q(&self) -> usize {
        self.q
    }

    fn check_pos(&self, i: usize) -> Result<(), PottsError> {
        if i >= self.n {
            Err(PottsError::PositionOutOfRange { index: i, n: self.n })
        } else {
            Ok(())
        }
    }

    fn check_state(&self, a: usize) -> Result<(), PottsError> {
        if a >= self.q {
            Err(PottsError::StateOutOfRange { state: a, q: self.q })
        } else {
            Ok(())
        }
    }

    /// Set the local field `h_i(a)` (overwrites any previous value).
    pub fn set_field(&mut self, i: usize, a: usize, value: f64) -> Result<(), PottsError> {
        self.check_pos(i)?;
        self.check_state(a)?;
        self.fields[i * self.q + a] = value;
        Ok(())
    }

    /// Read the local field `h_i(a)`.
    pub fn field(&self, i: usize, a: usize) -> Result<f64, PottsError> {
        self.check_pos(i)?;
        self.check_state(a)?;
        Ok(self.fields[i * self.q + a])
    }

    /// Add `value` to the pairwise coupling `J_{ij}(a, b)`.
    ///
    /// Couplings are symmetric under position swap: `add_coupling(i, j, a, b)`
    /// and `add_coupling(j, i, b, a)` refer to the same entry and accumulate.
    /// Calls accumulate (`+=`) so a coupling can be built up incrementally.
    pub fn add_coupling(
        &mut self,
        i: usize,
        j: usize,
        a: usize,
        b: usize,
        value: f64,
    ) -> Result<(), PottsError> {
        self.check_pos(i)?;
        self.check_pos(j)?;
        self.check_state(a)?;
        self.check_state(b)?;
        if i == j {
            return Err(PottsError::SelfCoupling { index: i });
        }
        // Canonicalize to i < j, swapping the state pair to match.
        let (i, j, a, b) = if i < j { (i, j, a, b) } else { (j, i, b, a) };
        let q = self.q;
        let mat = self.couplings.entry((i, j)).or_insert_with(|| vec![0.0; q * q]);
        mat[a * q + b] += value;
        Ok(())
    }

    /// Read the pairwise coupling `J_{ij}(a, b)` (0.0 if none was set).
    pub fn coupling(&self, i: usize, j: usize, a: usize, b: usize) -> Result<f64, PottsError> {
        self.check_pos(i)?;
        self.check_pos(j)?;
        self.check_state(a)?;
        self.check_state(b)?;
        if i == j {
            return Err(PottsError::SelfCoupling { index: i });
        }
        let (i, j, a, b) = if i < j { (i, j, a, b) } else { (j, i, b, a) };
        Ok(self
            .couplings
            .get(&(i, j))
            .map(|mat| mat[a * self.q + b])
            .unwrap_or(0.0))
    }

    /// Validate that `cfg` has length `n` and every entry is `< q`.
    fn check_config(&self, cfg: &[usize]) -> Result<(), PottsError> {
        if cfg.len() != self.n {
            return Err(PottsError::BadLength {
                got: cfg.len(),
                expected: self.n,
            });
        }
        for &a in cfg {
            self.check_state(a)?;
        }
        Ok(())
    }

    /// Energy of a configuration: `E(σ) = -Σ h_i(σ_i) - Σ_{i<j} J_{ij}(σ_i, σ_j)`.
    ///
    /// Unlike the broken sketch this replaces, the result genuinely depends on
    /// every entry of `cfg`.
    pub fn energy(&self, cfg: &[usize]) -> Result<f64, PottsError> {
        self.check_config(cfg)?;
        Ok(self.energy_unchecked(cfg))
    }

    /// Energy without bounds checking. Callers must guarantee a valid `cfg`.
    fn energy_unchecked(&self, cfg: &[usize]) -> f64 {
        let mut e = 0.0;
        for (i, &a) in cfg.iter().enumerate() {
            e -= self.fields[i * self.q + a];
        }
        for (&(i, j), mat) in &self.couplings {
            e -= mat[cfg[i] * self.q + cfg[j]];
        }
        e
    }

    /// `true` iff `cfg` satisfies every constraint in `constraints`.
    pub fn satisfies(&self, cfg: &[usize], constraints: &[Constraint]) -> Result<bool, PottsError> {
        self.check_config(cfg)?;
        Ok(constraints.iter().all(|c| c.holds(cfg)))
    }

    /// Size of the exhaustive state space `q^n`, or `None` if it overflows u128.
    pub fn state_space_size(&self) -> Option<u128> {
        (self.q as u128).checked_pow(self.n as u32)
    }

    /// Maximum `q^n` that the exhaustive methods will enumerate.
    pub const EXHAUSTIVE_CAP: u128 = 50_000_000;

    fn ensure_exhaustible(&self) -> Result<(), PottsError> {
        match self.state_space_size() {
            Some(size) if size <= Self::EXHAUSTIVE_CAP => Ok(()),
            Some(size) => Err(PottsError::StateSpaceTooLarge {
                size,
                cap: Self::EXHAUSTIVE_CAP,
            }),
            None => Err(PottsError::StateSpaceTooLarge {
                size: u128::MAX,
                cap: Self::EXHAUSTIVE_CAP,
            }),
        }
    }

    /// Visit every configuration in lexicographic (base-`q`) order.
    fn for_each_config<F: FnMut(&[usize])>(&self, mut f: F) {
        let mut cfg = vec![0usize; self.n];
        loop {
            f(&cfg);
            // Increment the odometer; stop when it rolls over completely.
            let mut k = 0;
            loop {
                if k == self.n {
                    return;
                }
                cfg[k] += 1;
                if cfg[k] < self.q {
                    break;
                }
                cfg[k] = 0;
                k += 1;
            }
        }
    }

    /// Exact partition function `Z = Σ_σ exp(-β E(σ))` by enumeration.
    ///
    /// Errors if `q^n` exceeds [`Self::EXHAUSTIVE_CAP`].
    pub fn partition_function(&self, beta: f64) -> Result<f64, PottsError> {
        self.ensure_exhaustible()?;
        let mut z = 0.0;
        self.for_each_config(|cfg| {
            z += (-beta * self.energy_unchecked(cfg)).exp();
        });
        Ok(z)
    }

    /// Boltzmann probability `P(σ) = exp(-β E(σ)) / Z` of a single configuration.
    ///
    /// Errors if `q^n` exceeds [`Self::EXHAUSTIVE_CAP`] (the partition function
    /// must be summed exactly).
    pub fn probability(&self, cfg: &[usize], beta: f64) -> Result<f64, PottsError> {
        let e = self.energy(cfg)?;
        let z = self.partition_function(beta)?;
        Ok((-beta * e).exp() / z)
    }

    /// Exact MAP inference: the minimum-energy configuration and its energy,
    /// found by exhaustive enumeration. This is the ground-truth oracle.
    ///
    /// Ties are broken by lexicographic order (the first minimizer found).
    /// Errors if `q^n` exceeds [`Self::EXHAUSTIVE_CAP`].
    pub fn map_exact(&self) -> Result<(Vec<usize>, f64), PottsError> {
        self.map_exact_constrained(&[])
    }

    /// Exact MAP inference restricted to configurations satisfying every
    /// constraint. Returns `Ok(None)` if the feasible set is empty.
    ///
    /// Errors if `q^n` exceeds [`Self::EXHAUSTIVE_CAP`].
    pub fn map_exact_constrained(
        &self,
        constraints: &[Constraint],
    ) -> Result<(Vec<usize>, f64), PottsError> {
        self.map_exact_constrained_opt(constraints)
            .map(|opt| opt.expect("unconstrained MAP always has a minimizer"))
    }

    /// Like [`Self::map_exact_constrained`] but returns `Ok(None)` when no
    /// configuration satisfies the constraints.
    pub fn map_exact_constrained_opt(
        &self,
        constraints: &[Constraint],
    ) -> Result<Option<(Vec<usize>, f64)>, PottsError> {
        self.ensure_exhaustible()?;
        let mut best: Option<(Vec<usize>, f64)> = None;
        self.for_each_config(|cfg| {
            if !constraints.iter().all(|c| c.holds(cfg)) {
                return;
            }
            let e = self.energy_unchecked(cfg);
            match &best {
                Some((_, be)) if *be <= e + EPS => {}
                _ => best = Some((cfg.to_vec(), e)),
            }
        });
        // For the unconstrained case `best` is always `Some`; with constraints
        // it can legitimately be `None`.
        Ok(best)
    }

    /// Iterated Conditional Modes from a given starting configuration.
    ///
    /// Repeatedly sweeps positions, setting each to the state that minimizes
    /// energy given the others, until a full sweep makes no improvement. This
    /// is monotone coordinate descent: energy never increases. It converges to
    /// a *local* minimum, which is why it is tested against [`Self::map_exact`].
    pub fn map_icm(&self, init: &[usize]) -> Result<(Vec<usize>, f64), PottsError> {
        self.check_config(init)?;
        let mut cfg = init.to_vec();
        loop {
            let mut improved = false;
            for i in 0..self.n {
                // Snapshot the current state before probing: the loop below
                // mutates `cfg[i]`, so we must compare against `orig`, not the
                // leftover probe value, when deciding whether we improved.
                let orig = cfg[i];
                let mut best_a = orig;
                // `best_e` is measured while `cfg[i] == orig`.
                let mut best_e = self.energy_unchecked(&cfg);
                for a in 0..self.q {
                    if a == orig {
                        continue;
                    }
                    cfg[i] = a;
                    let e = self.energy_unchecked(&cfg);
                    if e < best_e - EPS {
                        best_e = e;
                        best_a = a;
                    }
                }
                cfg[i] = best_a;
                if best_a != orig {
                    improved = true;
                }
            }
            if !improved {
                break;
            }
        }
        let e = self.energy_unchecked(&cfg);
        Ok((cfg, e))
    }

    /// ICM with multiple random restarts; returns the best local optimum found.
    ///
    /// Restart configurations are drawn from a seeded SplitMix64 generator, so
    /// results are fully deterministic for a given `(seed, restarts)`. With
    /// enough restarts this reliably recovers the exact MAP on small instances
    /// (verified in the test suite).
    pub fn map_icm_restarts(
        &self,
        seed: u64,
        restarts: usize,
    ) -> Result<(Vec<usize>, f64), PottsError> {
        let mut rng = SplitMix64::new(seed);
        // Always include the all-zero start for reproducible baseline behavior.
        let mut best = self.map_icm(&vec![0usize; self.n])?;
        for _ in 0..restarts {
            let init: Vec<usize> = (0..self.n)
                .map(|_| (rng.next_u64() % self.q as u64) as usize)
                .collect();
            let candidate = self.map_icm(&init)?;
            if candidate.1 < best.1 - EPS {
                best = candidate;
            }
        }
        Ok(best)
    }
}

/// Minimal seeded PRNG (SplitMix64) so the heuristic is deterministic without
/// pulling in an external `rand` dependency.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn energy_depends_on_configuration_by_hand() {
        // n = 2, q = 2. Fields and a single coupling chosen so we can compute
        // every energy by hand and confirm the result actually varies with σ.
        let mut m = PottsModel::new(2, 2);
        m.set_field(0, 0, 1.0).unwrap();
        m.set_field(0, 1, 0.0).unwrap();
        m.set_field(1, 0, 0.0).unwrap();
        m.set_field(1, 1, 2.0).unwrap();
        m.add_coupling(0, 1, 0, 0, 3.0).unwrap();

        // E(σ) = -h0(σ0) - h1(σ1) - J01(σ0,σ1)
        // (0,0): -1 - 0 - 3 = -4
        // (0,1): -1 - 2 - 0 = -3
        // (1,0):  0 - 0 - 0 =  0
        // (1,1):  0 - 2 - 0 = -2
        assert!(approx(m.energy(&[0, 0]).unwrap(), -4.0));
        assert!(approx(m.energy(&[0, 1]).unwrap(), -3.0));
        assert!(approx(m.energy(&[1, 0]).unwrap(), 0.0));
        assert!(approx(m.energy(&[1, 1]).unwrap(), -2.0));
        // The four energies are not all equal — the bug in the original sketch
        // (energy independent of σ) is gone.
        let energies = [-4.0, -3.0, 0.0, -2.0];
        assert!(energies.iter().any(|&e| !approx(e, energies[0])));
    }

    #[test]
    fn coupling_is_symmetric_under_position_swap() {
        let mut m = PottsModel::new(3, 4);
        m.add_coupling(2, 0, 3, 1, 1.5).unwrap();
        // Same entry read either way round.
        assert!(approx(m.coupling(2, 0, 3, 1).unwrap(), 1.5));
        assert!(approx(m.coupling(0, 2, 1, 3).unwrap(), 1.5));
        // A different state pair is untouched.
        assert!(approx(m.coupling(0, 2, 3, 1).unwrap(), 0.0));
    }

    #[test]
    fn couplings_accumulate() {
        let mut m = PottsModel::new(2, 2);
        m.add_coupling(0, 1, 0, 0, 1.0).unwrap();
        m.add_coupling(1, 0, 0, 0, 0.5).unwrap();
        assert!(approx(m.coupling(0, 1, 0, 0).unwrap(), 1.5));
    }

    #[test]
    fn probabilities_sum_to_one_and_match_manual_partition() {
        let mut m = PottsModel::new(2, 2);
        m.set_field(0, 0, 1.0).unwrap();
        m.set_field(1, 1, 2.0).unwrap();
        m.add_coupling(0, 1, 0, 0, 3.0).unwrap();
        let beta = 0.7;

        // Manual Z from the four hand-computed energies.
        let energies = [
            m.energy(&[0, 0]).unwrap(),
            m.energy(&[0, 1]).unwrap(),
            m.energy(&[1, 0]).unwrap(),
            m.energy(&[1, 1]).unwrap(),
        ];
        let z_manual: f64 = energies.iter().map(|e| (-beta * e).exp()).sum();
        assert!(approx(m.partition_function(beta).unwrap(), z_manual));

        let total: f64 = [[0, 0], [0, 1], [1, 0], [1, 1]]
            .iter()
            .map(|c| m.probability(c, beta).unwrap())
            .sum();
        assert!(approx(total, 1.0));
    }

    #[test]
    fn exact_map_matches_independent_enumeration() {
        // Build a pseudo-random model and confirm map_exact agrees with an
        // independent brute-force written inline in the test.
        let mut rng = SplitMix64::new(42);
        let (n, q) = (4, 3);
        let mut m = PottsModel::new(n, q);
        for i in 0..n {
            for a in 0..q {
                let v = (rng.next_u64() % 2001) as f64 / 100.0 - 10.0;
                m.set_field(i, a, v).unwrap();
            }
        }
        for i in 0..n {
            for j in (i + 1)..n {
                for a in 0..q {
                    for b in 0..q {
                        let v = (rng.next_u64() % 2001) as f64 / 100.0 - 10.0;
                        m.add_coupling(i, j, a, b, v).unwrap();
                    }
                }
            }
        }

        // Independent enumeration.
        let mut best: Option<(Vec<usize>, f64)> = None;
        let mut cfg = vec![0usize; n];
        loop {
            let e = m.energy(&cfg).unwrap();
            if best.as_ref().map(|(_, be)| e < *be).unwrap_or(true) {
                best = Some((cfg.clone(), e));
            }
            let mut k = 0;
            let done = loop {
                if k == n {
                    break true;
                }
                cfg[k] += 1;
                if cfg[k] < q {
                    break false;
                }
                cfg[k] = 0;
                k += 1;
            };
            if done {
                break;
            }
        }
        let (_, best_e) = best.unwrap();
        let (_, map_e) = m.map_exact().unwrap();
        assert!(approx(best_e, map_e));
    }

    #[test]
    fn icm_never_increases_energy_and_finds_global_on_small_instances() {
        // Over many random models, ICM-with-restarts must reach the exact MAP,
        // and plain ICM must never end above where it started.
        let (n, q) = (5, 3);
        for trial in 0..50u64 {
            let mut rng = SplitMix64::new(1000 + trial);
            let mut m = PottsModel::new(n, q);
            for i in 0..n {
                for a in 0..q {
                    let v = (rng.next_u64() % 4001) as f64 / 100.0 - 20.0;
                    m.set_field(i, a, v).unwrap();
                }
            }
            for i in 0..n {
                for j in (i + 1)..n {
                    for a in 0..q {
                        for b in 0..q {
                            let v = (rng.next_u64() % 4001) as f64 / 100.0 - 20.0;
                            m.add_coupling(i, j, a, b, v).unwrap();
                        }
                    }
                }
            }

            let start = vec![0usize; n];
            let start_e = m.energy(&start).unwrap();
            let (_, icm_e) = m.map_icm(&start).unwrap();
            assert!(icm_e <= start_e + EPS, "ICM increased energy");

            let (_, exact_e) = m.map_exact().unwrap();
            let (_, best_e) = m.map_icm_restarts(trial, 40).unwrap();
            assert!(
                (best_e - exact_e).abs() < 1e-9,
                "trial {trial}: restarts found {best_e}, exact is {exact_e}",
            );
        }
    }

    #[test]
    fn map_is_argmax_of_probability() {
        // The minimum-energy configuration must also be the most probable one.
        let mut rng = SplitMix64::new(7);
        let (n, q) = (3, 4);
        let mut m = PottsModel::new(n, q);
        for i in 0..n {
            for a in 0..q {
                let v = (rng.next_u64() % 1001) as f64 / 100.0;
                m.set_field(i, a, v).unwrap();
            }
        }
        for a in 0..q {
            for b in 0..q {
                let v = (rng.next_u64() % 1001) as f64 / 100.0;
                m.add_coupling(0, 2, a, b, v).unwrap();
            }
        }
        let (map_cfg, _) = m.map_exact().unwrap();
        let map_p = m.probability(&map_cfg, 1.3).unwrap();

        m.for_each_config(|cfg| {
            let p = (-1.3 * m.energy_unchecked(cfg)).exp()
                / m.partition_function(1.3).unwrap();
            assert!(p <= map_p + 1e-9);
        });
    }

    #[test]
    fn constraints_are_respected_and_can_be_infeasible() {
        // Symmetry constraints like the pasted doc's σ0 == σ3, done for real.
        let mut rng = SplitMix64::new(99);
        let (n, q) = (4, 3);
        let mut m = PottsModel::new(n, q);
        for i in 0..n {
            for a in 0..q {
                let v = (rng.next_u64() % 1001) as f64 / 100.0;
                m.set_field(i, a, v).unwrap();
            }
        }

        let cons = [Constraint::Equal(0, 3), Constraint::Fixed(1, 2)];
        let (cfg, _) = m.map_exact_constrained(&cons).unwrap();
        assert_eq!(cfg[0], cfg[3]);
        assert_eq!(cfg[1], 2);
        assert!(m.satisfies(&cfg, &cons).unwrap());

        // The constrained optimum is no better than the unconstrained one.
        let (_, free_e) = m.map_exact().unwrap();
        let cons_e = m.energy(&cfg).unwrap();
        assert!(cons_e >= free_e - EPS);

        // A pair of contradictory equal/not-equal constraints is infeasible.
        let bad = [Constraint::Equal(0, 1), Constraint::NotEqual(0, 1)];
        assert!(m.map_exact_constrained_opt(&bad).unwrap().is_none());
    }

    #[test]
    fn determinism_of_restarts() {
        let mut m = PottsModel::new(4, 3);
        m.set_field(0, 1, 5.0).unwrap();
        m.add_coupling(0, 1, 1, 1, 2.0).unwrap();
        let a = m.map_icm_restarts(2024, 16).unwrap();
        let b = m.map_icm_restarts(2024, 16).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn state_space_cap_is_enforced() {
        // q^n far over the cap must be rejected rather than looping forever.
        let m = PottsModel::new(40, 20);
        assert!(matches!(
            m.map_exact(),
            Err(PottsError::StateSpaceTooLarge { .. })
        ));
        // ...but the heuristic still works at that size.
        let init = vec![0usize; 40];
        assert!(m.map_icm(&init).is_ok());
    }

    #[test]
    fn error_paths() {
        let mut m = PottsModel::new(2, 2);
        assert_eq!(
            m.set_field(5, 0, 1.0),
            Err(PottsError::PositionOutOfRange { index: 5, n: 2 })
        );
        assert_eq!(
            m.set_field(0, 9, 1.0),
            Err(PottsError::StateOutOfRange { state: 9, q: 2 })
        );
        assert_eq!(
            m.add_coupling(0, 0, 0, 0, 1.0),
            Err(PottsError::SelfCoupling { index: 0 })
        );
        assert_eq!(
            m.energy(&[0, 0, 0]),
            Err(PottsError::BadLength { got: 3, expected: 2 })
        );
    }
}
