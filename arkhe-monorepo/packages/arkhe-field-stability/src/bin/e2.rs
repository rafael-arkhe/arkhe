//! Experimento E2 — Refinamento determinístico V2 → V1 (Fase 3).
//!
//! Executa o `IterativeRefiner` a partir de dois estados degradados e verifica
//! o critério de sucesso: atinge `Φ > 0,95` em `≤ 1000` iterações com
//! convergência monotônica. Estado canônico: V2-deep `(0.60, 0.55, 0.50)`
//! com `Φ ≈ 0,5584`; referência: V2-canônico `(0.80, 0.75, 0.50)`
//! com `Φ ≈ 0,6983`. Alvo: V1 (`Φ ≈ 0,9646`).

#![deny(unsafe_code)]
#![warn(missing_docs)]

use arkhe_field_stability::experiment::{artifact_root, write_json, write_text};
use arkhe_field_stability::{phi_default, IterativeRefiner, RefinerOutcome};

const RUNS: [(&str, [f64; 3]); 2] = [
    ("v2_deep_phi055", [0.60, 0.55, 0.50]),
    ("v2_canonical_phi070", [0.80, 0.75, 0.50]),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let refiner = IterativeRefiner::default();
    let root = artifact_root("e2");
    let mut runs_summary = Vec::new();

    for (name, start) in RUNS {
        let trace = refiner.refine_traced(start);
        let start_phi = phi_default(start);

        let mut csv = String::from("iteration,phi,overall,omega,sigma,lambda\n");
        let mut monotonic = true;
        let mut prev_phi = f64::NEG_INFINITY;
        for step in &trace.steps {
            csv.push_str(&format!(
                "{0},{1:.6},{2:.6},{3:.6},{4:.6},{5:.6}\n",
                step.iteration,
                step.phi,
                step.overall,
                step.components[0],
                step.components[1],
                step.components[2]
            ));
            if prev_phi.is_finite() && step.phi < prev_phi - 1e-9 {
                monotonic = false;
            }
            prev_phi = step.phi;
        }

        let reached = trace.result.outcome == RefinerOutcome::ReachedTarget;
        let success = reached
            && trace.result.phi > 0.95
            && trace.result.iterations <= 1000
            && monotonic;

        write_text(&root.join(format!("e2_{name}.csv")), &csv)?;
        tracing::info!(experiment = "e2", run = name, outcome = ?trace.result.outcome,
                       iterations = trace.result.iterations, success);

        runs_summary.push(serde_json::json!({
            "run": name,
            "start_components": start,
            "start_phi": start_phi,
            "iterations": trace.result.iterations,
            "end_phi": trace.result.phi,
            "end_overall": trace.result.overall,
            "end_components": trace.result.components,
            "outcome": format!("{:?}", trace.result.outcome),
            "monotonic": monotonic,
            "success": success
        }));

        println!(
            "  [E2] {name}: Φ {:.4} → {:.4} em {} iterações (monotônico: {monotonic})",
            start_phi, trace.result.phi, trace.result.iterations
        );
    }

    let all_ok = runs_summary.iter().all(|r| r["success"].as_bool() == Some(true));
    let summary = serde_json::json!({
        "experiment": "e2",
        "criterion": "refiner atinge Φ > 0.95 em <= 1000 iteracoes com convergencia monotonica",
        "target": "V1 (Phi ~ 0.9646)",
        "runs": runs_summary,
        "success": all_ok
    });
    write_json(&root.join("e2_summary.json"), &summary)?;
    println!(
        "========================================\nE2 Refinamento V2→V1 — sucesso: {all_ok}\n  Artefatos: {}",
        root.display()
    );

    Ok(())
}