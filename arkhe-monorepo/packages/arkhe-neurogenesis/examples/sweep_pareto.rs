//! `sweep_pareto` — exhaustive sweep of the Neurogenesis parameter space.
//!
//! ```text
//! cargo run --example sweep_pareto --release
//! ```
//!
//! Emits two CSVs into the working directory:
//!   - `sweep_full.csv`      : every configuration evaluated
//!   - `pareto_frontier.csv` : the strictly non-dominated points
//!
//! Objectives (strict dominance, no aggregation): minimize `peak_drift`
//! (I18 spectral fidelity) and maximize `branches` (I16 morphological
//! complexity).
//!
//! Invariants:
//!
//! - I-S1 Reproducibility: deterministic RNG seeded per run, no thread_rng.
//! - I-S2 Fidelity: uses the crate's `SimulationConfig` / `simulate`.
//! - I-S3 Fixed references: the I16 and I18 baselines are always present and
//!   validated.
//! - I-S4 Pareto without aggregation: strict dominance filter.

use std::path::Path;

use arkhe_neurogenesis::{simulate, SimulationConfig, SimulationResult};
use rayon::prelude::*;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
struct SweepRecord {
    // Parameters.
    pub seed: u64,
    pub threshold: f64,
    pub n: usize,
    pub t_max: f64,
    pub dt: f64,
    // Output metrics.
    pub peak_drift: f64,
    pub final_drift: f64,
    pub peak_time: f64,
    pub branches: usize,
    pub passed_i16: bool,
    pub passed_i18: bool,
    // Reference marker.
    pub reference: String,
    // Unique id.
    pub run_id: String,
}

impl SweepRecord {
    fn from_result(config: &SimulationConfig, result: &SimulationResult, reference: &str, run_id: usize) -> Self {
        Self {
            seed: config.seed,
            threshold: config.threshold,
            n: config.n,
            t_max: config.t_max,
            dt: config.dt,
            peak_drift: result.peak_drift,
            final_drift: result.final_drift,
            peak_time: result.peak_time,
            branches: result.branches,
            passed_i16: result.branches >= 4,
            passed_i18: result.peak_drift < 0.03,
            reference: reference.to_string(),
            run_id: format!("{run_id:06}"),
        }
    }
}

/// Strict-dominance Pareto filter over `(peak_drift, branches)`.
///
/// A point dominates another when it is no worse on both objectives and
/// strictly better on at least one. With the two-objective skyline this is
/// computed in O(n log n): sort by `peak_drift` ascending, then keep a point
/// whenever it improves on the best `branches` seen so far.
fn pareto_frontier_indices(records: &[SweepRecord]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..records.len()).collect();
    indices.sort_by(|&a, &b| {
        records[a]
            .peak_drift
            .partial_cmp(&records[b].peak_drift)
            .expect("peak_drift is finite")
            .then(records[b].branches.cmp(&records[a].branches))
    });

    let mut frontier = Vec::new();
    let mut best_branches: isize = -1;
    for &i in &indices {
        if records[i].branches as isize > best_branches {
            frontier.push(i);
            best_branches = records[i].branches as isize;
        }
    }
    frontier
}

fn main() -> anyhow::Result<()> {
    let seeds: Vec<u64> = (0..20).collect();
    let thresholds = [0.05, 0.10, 0.15, 0.20, 0.25];
    let ns = [4, 6, 8, 12, 16];
    let t_maxs = [0.3, 0.5, 1.0, 3.0];
    let dts = [0.005, 0.01, 0.02];

    // Fixed I16/I18 baselines, always present regardless of the grid.
    let reference_configs = [
        (7u64, 0.3f64, 16usize, 3.0f64, 0.01f64, "I16"),
        (8u64, 0.3f64, 8usize, 0.3f64, 0.01f64, "I18"),
    ];

    let mut all_configs: Vec<(u64, f64, usize, f64, f64, String)> = Vec::new();
    for (seed, th, n, t_max, dt, label) in reference_configs {
        all_configs.push((seed, th, n, t_max, dt, label.to_string()));
    }

    for &seed in &seeds {
        for &th in &thresholds {
            for &n in &ns {
                for &t_max in &t_maxs {
                    for &dt in &dts {
                        let is_ref = reference_configs.iter().any(|(s, th2, n2, t2, dt2, _)| {
                            *s == seed
                                && (th2 - th).abs() < 1e-12
                                && *n2 == n
                                && (t2 - t_max).abs() < 1e-12
                                && (dt2 - dt).abs() < 1e-12
                        });
                        if !is_ref {
                            all_configs.push((seed, th, n, t_max, dt, "candidate".to_string()));
                        }
                    }
                }
            }
        }
    }

    println!("total configurations to simulate: {}", all_configs.len());

    let records: Vec<SweepRecord> = all_configs
        .par_iter()
        .enumerate()
        .map(|(idx, (seed, th, n, t_max, dt, label))| {
            let config = SimulationConfig {
                seed: *seed,
                threshold: *th,
                n: *n,
                t_max: *t_max,
                dt: *dt,
                ..SimulationConfig::default()
            };
            let result = simulate(&config);
            SweepRecord::from_result(&config, &result, label, idx)
        })
        .collect();

    let full_path = Path::new("sweep_full.csv");
    let mut wtr = csv::Writer::from_path(full_path)?;
    for rec in &records {
        wtr.serialize(rec)?;
    }
    wtr.flush()?;
    println!("full records written to {}", full_path.display());

    let pareto_indices = pareto_frontier_indices(&records);
    let pareto_path = Path::new("pareto_frontier.csv");
    let mut wtr_p = csv::Writer::from_path(pareto_path)?;
    for &idx in &pareto_indices {
        wtr_p.serialize(&records[idx])?;
    }
    wtr_p.flush()?;
    println!("pareto frontier written to {}", pareto_path.display());

    // I-S3: validate the fixed references.
    let i16_ref = records.iter().find(|r| r.reference == "I16");
    match i16_ref {
        Some(r) => {
            assert_eq!(r.branches, 4, "I16 baseline must yield exactly 4 branches");
            assert!(r.passed_i16, "I16 baseline must pass I16");
            println!("I16 baseline verified: seed={}, branches={}", r.seed, r.branches);
        }
        None => eprintln!("warning: I16 baseline missing from sweep"),
    }

    let i18_ref = records.iter().find(|r| r.reference == "I18");
    match i18_ref {
        Some(r) => {
            assert!(r.peak_drift < 0.03, "I18 baseline peak_drift must be < 0.03");
            println!("I18 baseline verified: seed={}, peak_drift={:.4}", r.seed, r.peak_drift);
        }
        None => eprintln!("warning: I18 baseline missing from sweep"),
    }

    println!();
    println!("first 5 Pareto frontier points:");
    for (i, &idx) in pareto_indices.iter().take(5).enumerate() {
        let r = &records[idx];
        println!(
            "  {}. seed={:2}, th={:.2}, n={:2}, t_max={:.1}, dt={:.3} -> drift={:.4}, branches={:2}",
            i + 1, r.seed, r.threshold, r.n, r.t_max, r.dt, r.peak_drift, r.branches
        );
    }

    Ok(())
}
