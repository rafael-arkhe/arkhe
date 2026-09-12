//! `sweep` — parameter sweep for P1 (K vs sigma_w) and P3 (K vs alpha).
//!
//! ```text
//! cargo run --example sweep --release
//! ```
//!
//! Emits `xy_sweep_p1.csv` and `xy_sweep_p3.csv` into the working directory.
//! Every point follows the 5-seed protocol (mean/std reported).

extern crate csv;
extern crate rayon;

use arkhe_xy_simulator::p1_kuramoto::{FreqDist, KuramotoConfig, KuramotoSim};
use arkhe_xy_simulator::p3_dmi::{ChiralConfig, run_dmi};
use rayon::prelude::*;

#[derive(serde::Serialize)]
struct P1Rec {
    k: f64,
    sigma_w: f64,
    r_mean: f64,
    r_std: f64,
    kc_all: f64,
}

#[derive(serde::Serialize)]
struct P3Rec {
    k: f64,
    alpha: f64,
    drive_order: f64,
    pitch_measured: f64,
    relax_order_2d: f64,
}

fn main() -> anyhow::Result<()> {
    // P1: K in 0.2..2.0, sigma in {0.2, 0.5, 1.0}.
    let p1_grid: Vec<(f64, f64)> = (4..=20)
        .map(|ki| ki as f64 * 0.1)
        .flat_map(|k| [0.2, 0.5, 1.0].map(move |sigma| (k, sigma)))
        .collect();
    let p1: Vec<P1Rec> = p1_grid
        .par_iter()
        .map(|&(k, sigma)| {
            let cfg = KuramotoConfig {
                n: 8,
                dt: 0.02,
                k,
                dist: FreqDist::Gaussian { sigma },
                seed: 7,
            };
            let res = KuramotoSim::run(&cfg, 6_000, 400);
            P1Rec {
                k,
                sigma_w: sigma,
                r_mean: res.final_order_mean,
                r_std: res.final_order_std,
                kc_all: res.kc_all_to_all,
            }
        })
        .collect();

    let mut w1 = csv::Writer::from_path("xy_sweep_p1.csv")?;
    for r in &p1 {
        w1.serialize(r)?;
    }
    w1.flush()?;
    println!("P1 sweep: {} points -> xy_sweep_p1.csv", p1.len());

    // P3: K in 0.4..1.6, alpha in 0.1..0.8 (driving + energy relaxation).
    let p3_grid: Vec<(f64, f64)> = (4..=16)
        .map(|ki| ki as f64 * 0.1)
        .flat_map(|k| {
            [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8].map(move |alpha| (k, alpha))
        })
        .collect();
    let p3: Vec<P3Rec> = p3_grid
        .par_iter()
        .map(|&(k, alpha)| {
            let cfg = ChiralConfig {
                n: 16,
                k,
                alpha,
                dt: 0.02,
                seed: 7,
            };
            let res = run_dmi(&cfg, 3_000, 8_000);
            P3Rec {
                k,
                alpha,
                drive_order: res.drive_order,
                pitch_measured: res.pitch_measured,
                relax_order_2d: res.relax_order_2d,
            }
        })
        .collect();

    let mut w3 = csv::Writer::from_path("xy_sweep_p3.csv")?;
    for r in &p3 {
        w3.serialize(r)?;
    }
    w3.flush()?;
    println!("P3 sweep: {} points -> xy_sweep_p3.csv", p3.len());

    Ok(())
}