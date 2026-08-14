//! `grow_neuron` — CLI runner for the ARKHE Neurogenesis simulator.
//!
//! ```text
//! cargo run --example grow_neuron -- [--seed N] [--steps N] [--n N] \
//!     [--threshold F] [--dt F] [--pt | --no-pt] [--growth | --no-growth]
//! ```

use arkhe_neurogenesis::{SimulationConfig, VERSION};

struct Args {
    seed: u64,
    steps: usize,
    n: usize,
    threshold: f64,
    dt: f64,
    pt: bool,
    growth: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            seed: 42,
            steps: 300,
            n: 16,
            threshold: 0.3,
            dt: 0.01,
            pt: true,
            growth: true,
        }
    }
}

fn parse<T: std::str::FromStr>(arg: &str) -> Option<T> {
    arg.parse().ok()
}

fn usage() -> ! {
    eprintln!(
        "usage: grow_neuron [--seed N] [--steps N] [--n N] [--threshold F] [--dt F] \
         [--pt | --no-pt] [--growth | --no-growth]"
    );
    std::process::exit(2);
}

fn main() {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--seed" => args.seed = parse(&it.next().unwrap_or_default()).unwrap_or_else(|| usage()),
            "--steps" => args.steps = parse(&it.next().unwrap_or_default()).unwrap_or_else(|| usage()),
            "--n" => args.n = parse(&it.next().unwrap_or_default()).unwrap_or_else(|| usage()),
            "--threshold" => args.threshold = parse(&it.next().unwrap_or_default()).unwrap_or_else(|| usage()),
            "--dt" => args.dt = parse(&it.next().unwrap_or_default()).unwrap_or_else(|| usage()),
            "--pt" => args.pt = true,
            "--no-pt" => args.pt = false,
            "--growth" => args.growth = true,
            "--no-growth" => args.growth = false,
            "--help" | "-h" => usage(),
            _ => usage(),
        }
    }

    let cfg = SimulationConfig {
        n: args.n,
        dt: args.dt,
        t_max: args.dt * args.steps as f64,
        seed: args.seed,
        threshold: args.threshold,
        growth_allowed: args.growth,
        parity_time_sym: args.pt,
    };

    println!("ARKHE Neurogenesis {VERSION}");
    println!(
        "config: n={} dt={} t_max={:.3} seed={} threshold={} growth={} pt={}",
        cfg.n, cfg.dt, cfg.t_max, cfg.seed, cfg.threshold, cfg.growth_allowed, cfg.parity_time_sym
    );

    let res = cfg.run();

    let initial = res.amplitude_series[0];
    let final_amp = res.amplitude_series[res.amplitude_series.len() - 1];
    let peak = res
        .amplitude_series
        .iter()
        .copied()
        .fold(0.0_f64, f64::max);
    println!("amplitude: initial={initial:.6} final={final_amp:.6} peak={peak:.6}");
    println!(
        "neuron: nodes={} branches={} events={}",
        res.neuron.nodes.len(),
        res.neuron.branches.len(),
        res.neuron.events.len()
    );

    for ev in res.neuron.events.iter().take(30) {
        println!(
            "  step={:>4} parent={:>2} -> child={:>2}  at ({:+.3}, {:+.3})",
            ev.step, ev.parent, ev.child, ev.pos.0, ev.pos.1
        );
    }
    if res.neuron.events.len() > 30 {
        println!("  ... ({} more events)", res.neuron.events.len() - 30);
    }
}
