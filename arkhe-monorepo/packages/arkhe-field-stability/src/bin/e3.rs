//! Experimento E3 — Varredura de estados degradados (Fase 3).
//!
//! Mapeia `Φ` em função da degradação estrutural `d ∈ [0,1]` por dois caminhos:
//!
//! * **uniforme** — `Ω = Σ = Λ = 1 − d` (degradação simétrica);
//! * **enviesado** — `Ω = 1 − d`, `Σ = 1 − 0.6·d`, `Λ = 1 − 0.3·d`
//!   (peso da degradação na estabilidade).
//!
//! Identifica os pontos de inflexão: cruzamento do piso constitucional **Gap-1**
//! (`Φ ≤ 1/√3 ≈ 0.577350`) e do piso de aceitabilidade (`overall < 0.80`).

#![deny(unsafe_code)]
#![warn(missing_docs)]

use arkhe_field_stability::experiment::{artifact_root, write_json, write_text};
use arkhe_field_stability::{
    phi_default, QUALITY_THRESHOLD, WEIGHT_LATENCY, WEIGHT_STABILITY, WEIGHT_SUCCESS_RATE,
};

const GAP1: f64 = 0.5773502691896258; // 1/√3 ≈ 0.5773502691896258 (sqrt não é função const)
const STEP: f64 = 0.002;

fn overall(components: [f64; 3]) -> f64 {
    WEIGHT_STABILITY * components[0]
        + WEIGHT_SUCCESS_RATE * components[1]
        + WEIGHT_LATENCY * components[2]
}

struct ScanResult {
    csv: String,
    d_gap1: Option<f64>,
    d_accept: Option<f64>,
}

fn scan(path: &str, components: impl Fn(f64) -> [f64; 3]) -> ScanResult {
    let mut csv = String::from("d,path,phi,overall,below_gap1,below_accept\n");
    let mut d_gap1 = None;
    let mut d_accept = None;

    let mut d = 0.0;
    while d <= 1.0 + 1e-12 {
        let c = components(d);
        let phi = phi_default(c);
        let ov = overall(c);
        let below_gap1 = phi <= GAP1;
        let below_accept = ov < QUALITY_THRESHOLD;
        if below_gap1 && d_gap1.is_none() {
            d_gap1 = Some(d);
        }
        if below_accept && d_accept.is_none() {
            d_accept = Some(d);
        }
        csv.push_str(&format!(
            "{d:.4},{path},{phi:.6},{ov:.6},{below_gap1},{below_accept}\n"
        ));
        d += STEP;
    }

    ScanResult {
        csv,
        d_gap1,
        d_accept,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let root = artifact_root("e3");

    let uniform = scan("uniforme", |d| [1.0 - d, 1.0 - d, 1.0 - d]);
    write_text(&root.join("e3_scan_uniforme.csv"), &uniform.csv)?;

    let biased = scan("enviesado", |d| [1.0 - d, 1.0 - 0.6 * d, 1.0 - 0.3 * d]);
    write_text(&root.join("e3_scan_enviesado.csv"), &biased.csv)?;

    let success = uniform.d_gap1.is_some()
        && biased.d_gap1.is_some()
        && uniform.d_accept.is_some()
        && biased.d_accept.is_some();

    let summary = serde_json::json!({
        "experiment": "e3",
        "criterion": "curva de resposta Phi x degradacao com cruzamentos identificados (Gap-1 e aceitabilidade)",
        "gap1": GAP1,
        "acceptability_floor": QUALITY_THRESHOLD,
        "step": STEP,
        "paths": {
            "uniforme": {
                "d_gap1_first": uniform.d_gap1,
                "d_accept_first": uniform.d_accept,
                "phi_at_gap1": uniform.d_gap1.map(|d| phi_default([1.0 - d, 1.0 - d, 1.0 - d]))
            },
            "enviesado": {
                "d_gap1_first": biased.d_gap1,
                "d_accept_first": biased.d_accept,
                "phi_at_gap1": biased.d_gap1.map(|d| phi_default([1.0 - d, 1.0 - 0.6 * d, 1.0 - 0.3 * d]))
            }
        },
        "success": success
    });
    write_json(&root.join("e3_summary.json"), &summary)?;

    println!("========================================");
    println!("E3 Varredura de degradação (passo {STEP})");
    for (label, res) in [
        ("uniforme", &uniform),
        ("enviesado", &biased),
    ] {
        let (dg, da) = (res.d_gap1, res.d_accept);
        let dg = dg.map(|v| format!("{v:.4}")).unwrap_or_else(|| "N/D".into());
        let da = da.map(|v| format!("{v:.4}")).unwrap_or_else(|| "N/D".into());
        println!("  {label}: cruzamento Gap-1 em d={dg} | aceitabilidade em d={da}");
    }
    println!("  Sucesso: {success}");
    println!("  Artefatos: {}", root.display());
    println!("========================================");

    Ok(())
}