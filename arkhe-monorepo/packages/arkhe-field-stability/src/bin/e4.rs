//! Experimento E4 — Estabilidade sob ruído (Fase 3).
//!
//! Injeta jitter na qualidade do handover (amplitude em {0, 5, 10, 15, 20}%)
//! e observa a variância de `Φ`. Critério de sucesso: para `jitter < 20%`, a
//! média de `Φ` permanece dentro de 1 desvio padrão do valor sem ruído.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use arkhe_field_stability::experiment::{artifact_root, mean, std, write_json, write_text, Srng};
use arkhe_field_stability::{
    phi_from_field_stability, FieldStability, QualityReport, LATENCY_TOLERANCE_MS,
    WEIGHT_LATENCY, WEIGHT_STABILITY, WEIGHT_SUCCESS_RATE,
};

const WINDOWS: usize = 40;
const SAMPLES_PER_WINDOW: usize = 50;
const BASE_QUALITY: f64 = 0.90;
const BASE_AMPLITUDE: f64 = 0.03;
const JITTERS: [f64; 5] = [0.0, 0.05, 0.10, 0.15, 0.20];
const SEED_ZERO: u64 = 1000;

fn phi_for_jitter(level: usize, jitter: f64) -> (Vec<f64>, Vec<f64>, String) {
    let mut rng = Srng::new(SEED_ZERO + level as u64);
    let mut phis = Vec::with_capacity(WINDOWS);
    let mut overalls = Vec::with_capacity(WINDOWS);
    let mut csv = String::from("jitter,window,phi,overall\n");

    for window in 0..WINDOWS {
        let history: Vec<f64> = (0..SAMPLES_PER_WINDOW)
            .map(|_| {
                let noise = BASE_AMPLITUDE * rng.uniform(-1.0, 1.0)
                    + jitter * rng.uniform(-1.0, 1.0);
                (BASE_QUALITY + noise).clamp(0.0, 1.0)
            })
            .collect();
        let latency_ms = rng.uniform(8.0, 25.0);
        let fs = FieldStability::new(history, latency_ms, LATENCY_TOLERANCE_MS);
        let phi = phi_from_field_stability(&fs);
        let report = QualityReport::from_field_stability(
            &fs,
            WEIGHT_STABILITY,
            WEIGHT_SUCCESS_RATE,
            WEIGHT_LATENCY,
        );
        phis.push(phi);
        overalls.push(report.overall);
        csv.push_str(&format!("{jitter:.2},{window},{phi:.6},{:.6}\n", report.overall));
    }

    (phis, overalls, csv)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let root = artifact_root("e4");
    let mut csv_all = String::from("jitter,window,phi,overall\n");
    let mut levels = Vec::new();

    for (i, jitter) in JITTERS.iter().enumerate() {
        let (phis, overalls, csv_level) = phi_for_jitter(i, *jitter);
        csv_all.push_str(&csv_level);

        let phi_mean = mean(&phis);
        let phi_std = std(&phis);
        levels.push(serde_json::json!({
            "jitter": jitter,
            "phi_mean": phi_mean,
            "phi_std": phi_std,
            "overall_mean": mean(&overalls)
        }));

        tracing::info!(experiment = "e4", jitter = jitter, phi_mean, phi_std);
    }

    write_text(&root.join("e4_noise.csv"), &csv_all)?;

    // Valor sem ruído = nível jitter 0.
    let base = &levels[0];
    let base_mean = base["phi_mean"].as_f64().unwrap_or(f64::NAN);
    let base_std = base["phi_std"].as_f64().unwrap_or(f64::NAN);

    let mut all_within = true;
    for level in levels.iter_mut().skip(1) {
        // Critério da especificação: jitter < 20% → usa apenas os primeiros 4
        // níveis (0.05, 0.10, 0.15). O nível 0.20 é reportado sem exigência.
        let jitter = level["jitter"].as_f64().unwrap_or(0.0);
        let mean_level = level["phi_mean"].as_f64().unwrap_or(f64::NAN);
        let within = jitter < 0.20 && (mean_level - base_mean).abs() <= base_std;
        if jitter < 0.20 {
            all_within = all_within && within;
        }
        level["within_1_sigma_of_noise_free"] = serde_json::json!(within);
    }

    let summary = serde_json::json!({
        "experiment": "e4",
        "criterion": "Phi permanece dentro de 1 desvio padrao do valor sem ruido para jitter < 20%",
        "noise_free": { "phi_mean": base_mean, "phi_std": base_std },
        "levels": levels,
        "success": all_within
    });
    write_json(&root.join("e4_summary.json"), &summary)?;

    println!("========================================");
    println!("E4 Estabilidade sob ruído");
    for (idx, level) in levels.iter().enumerate() {
        let jitter = level["jitter"].as_f64().unwrap_or(0.0);
        let mean_level = level["phi_mean"].as_f64().unwrap_or(f64::NAN);
        let std_level = level["phi_std"].as_f64().unwrap_or(f64::NAN);
        if idx == 0 {
            println!("  jitter 0.00 (referência): Φ média {mean_level:.4} ± {std_level:.4}");
            continue;
        }
        let within = level["within_1_sigma_of_noise_free"]
            .as_bool()
            .unwrap_or(false);
        println!(
            "  jitter {jitter:.2}: Φ média {mean_level:.4} ± {std_level:.4}  (1-sigma: {within})"
        );
    }
    println!("  Sucesso: {all_within}");
    println!("  Artefatos: {}", root.display());
    println!("========================================");

    Ok(())
}