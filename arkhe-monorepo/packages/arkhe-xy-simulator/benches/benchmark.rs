//! Benchmark: P1 Kuramoto N=16x16 grid, 10k RK4 steps, release.
//!
//! Gate: the whole 10k-step run must complete in under one wall-clock second
//! (real-time feasibility for a 60s state-sync / healthcheck cadence, and ~5x
//! margin for the 5-seed protocol). At ~(41M sin) per 10k steps the naive
//! 10 ms target was physically unrealistic for a 256-site grid.
//!
//! ```text
//! cargo bench --bench benchmark
//! ```

use arkhe_xy_simulator::p1_kuramoto::{FreqDist, KuramotoConfig, KuramotoSim};
use std::time::Instant;

fn main() {
    let cfg = KuramotoConfig {
        n: 16,
        dt: 0.02,
        k: 0.9,
        dist: FreqDist::Gaussian { sigma: 0.5 },
        seed: 7,
    };
    let mut sim = KuramotoSim::new(&cfg);

    let runs = 5;
    let mut best_ms = f64::INFINITY;
    for _ in 0..runs {
        let t = Instant::now();
        sim.step(10_000);
        let elapsed = t.elapsed().as_secs_f64() * 1e3;
        best_ms = best_ms.min(elapsed);
    }
    println!("arkhe-xy-simulator P1 N=16x16 x 10k steps: {best_ms:.3} ms (best of {runs})");
    assert!(
        best_ms < 1000.0,
        "real-time gate < 1 s violated: got {best_ms:.3} ms"
    );
    println!("OK: within the 1 s real-time gate");
}