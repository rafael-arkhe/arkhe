//! Experimento E1 — Baseline: Φ em regime estacionário (Fase 3).
//!
//! Mede o par `(overall, Φ)` em estado estacionário sob handovers de
//! qualidade controlada (janelas de 50 amostras, base 0.90 ± 0.05) e latência
//! dentro da tolerância.
//!
//! Critério de sucesso: Φ médio > 0,70 em regime **e** correlação Q–Φ
//! mensurável (`|r_pearson| > 0,5`).
//!
//! Desde a Fase 4 (bloco 994), cada janela grava também uma [`CoherenceEntry`]
//! no [`CoherenceLedger`] — a **cadeia de dados** encadeada por SHA3-256 (o
//! Journal bloco_987..993 registra decisões; o ledger registra métricas).
//!
//! Desde a Fase 7 (bloco 1005), cada janela consolida também um
//! [`CoherenceReport`] — Φ canônico quadrático **mais** os eixos de garantia
//! ortogonais (SemanticValidity com quórum estrito > 2/3 e Loopseal de
//! aelíclicidade). Estes eixos **não compõem** Φ; qualificam a confiança do
//! dado num envelope de auditoria.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use arkhe_field_stability::experiment::{
    artifact_root, mean, pearson, std, write_json, write_text, Srng,
};
use arkhe_field_stability::{
    aggregate_validity, phi_from_field_stability, ChainLink, CoherenceEntry, CoherenceLedger,
    CoherenceReport, FieldStability, GAP1_INFERIOR, IntegrityStatus, LoopSeal, LoopStatus,
    MIN_VALIDATORS, QualityReport, SemanticValidity, Validator, Verdict, LATENCY_TOLERANCE_MS,
    WEIGHT_LATENCY, WEIGHT_STABILITY, WEIGHT_SUCCESS_RATE,
};

const WINDOWS: usize = 200;
const SAMPLES_PER_WINDOW: usize = 50;
const BASE_QUALITY: f64 = 0.90;
const BASE_AMPLITUDE: f64 = 0.05;
const PHI_FLOOR: f64 = 0.70;
/// Âncora temporal determinística da cadeia (2026-09-06, epoch Unix).
/// Os timestamps do ledger são `BASE_TIMESTAMP + window`: ticks lógicos
/// monotônicos — simulação honesta, não medição de relógio de parede.
const BASE_TIMESTAMP: u64 = 1_788_736_030;

/// Validador nominal — aprova entradas íntegras.
struct NominalValidator;
impl Validator for NominalValidator {
    fn id(&self) -> &str {
        "e1_nominal"
    }
    fn verify(&self, entry: &CoherenceEntry) -> Verdict {
        if entry.hash == entry.compute_hash() && entry.phi > 0.5 {
            Verdict::Approve
        } else {
            Verdict::Reject
        }
    }
}

/// Validador de piso — aprova apenas Φ acima do piso constitucional.
struct FloorValidator;
impl Validator for FloorValidator {
    fn id(&self) -> &str {
        "e1_floor"
    }
    fn verify(&self, entry: &CoherenceEntry) -> Verdict {
        if entry.phi > GAP1_INFERIOR {
            Verdict::Approve
        } else {
            Verdict::Reject
        }
    }
}

/// Validador de aelíclicidade — aprova entradas cujo hash é novo.
struct NoveltyValidator {
    seal: LoopSeal,
}
impl Validator for NoveltyValidator {
    fn id(&self) -> &str {
        "e1_novelty"
    }
    fn verify(&self, entry: &CoherenceEntry) -> Verdict {
        if self.seal.seen_hashes().contains(&entry.hash) {
            Verdict::Reject
        } else {
            Verdict::Approve
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let mut rng = Srng::default();
    let mut ledger = CoherenceLedger::new();
    let mut novelty = LoopSeal::new();
    let mut sound_total = 0usize;
    let mut acceptable_total = 0usize;

    let mut phi_series = Vec::with_capacity(WINDOWS);
    let mut overall_series = Vec::with_capacity(WINDOWS);
    let mut csv = String::from("window,phi,overall,stability,success_rate,latency_score\n");

    for window in 0..WINDOWS {
        let history: Vec<f64> = (0..SAMPLES_PER_WINDOW)
            .map(|_| (BASE_QUALITY + rng.uniform(-1.0, 1.0) * BASE_AMPLITUDE).clamp(0.0, 1.0))
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
        phi_series.push(phi);
        overall_series.push(report.overall);
        csv.push_str(&format!(
            "{0},{1:.6},{2:.6},{3:.6},{4:.6},{5:.6}\n",
            window, phi, report.overall, fs.stability, fs.success_rate, fs.latency_score
        ));

        // Fase 4: uma entrada no ledger por janela (Gravity-1 + encadeamento).
        let entry = CoherenceEntry::from_parts(
            window as u64,
            BASE_TIMESTAMP + window as u64,
            phi,
            fs.stability,
            fs.success_rate,
            fs.latency_score,
            ledger.last_hash(),
        );
        ledger.push(entry).expect("ledger: gravity-1 + chaining");

        // Fase 7: eixos de garantia ortogonais — SemanticValidity + Loopseal.
        let validators: [&dyn Validator; MIN_VALIDATORS] = [
            &NominalValidator,
            &FloorValidator,
            &NoveltyValidator { seal: novelty.clone() },
        ];
        let semantic = aggregate_validity(&validators, ledger.entries().last().unwrap())
            .unwrap_or_else(|_| SemanticValidity::new(0, 0));
        // Incorpora o hash na cadeia de novidade (Loopseal) na ordem de ingesta.
        let last = ledger.entries().last().unwrap();
        novelty.push(&ChainLink {
            hash: last.hash.clone(),
            previous_hash: last.previous_hash.clone(),
        });
        let integrity = ledger.verify_integrity();
        let coherence = CoherenceReport::new(
            fs.stability,
            fs.success_rate,
            fs.latency_score,
            integrity,
            semantic,
            LoopStatus::New {
                previous_hash: Some(last.previous_hash.clone()),
            },
        );
        if coherence.assurance.is_sound() {
            sound_total += 1;
        }
        if coherence.acceptable {
            acceptable_total += 1;
        }
    }

    let root = artifact_root("e1");
    let csv_path = root.join("e1_phi_series.csv");
    write_text(&csv_path, &csv)?;
    tracing::info!(experiment = "e1", phase = "artifacts", path = ?csv_path);

    // Cadeia de dados (Fase 4): JSON encadeado + relatório autogerado.
    let ledger_json = ledger.to_json_pretty();
    let ledger_path = root.join("e1_coherence_ledger.json");
    write_text(&ledger_path, &ledger_json)?;
    tracing::info!(experiment = "e1", phase = "ledger", entries = ledger.entries().len());

    let integrity_status = ledger.verify_integrity();
    let report_path = root.join("e1_ledger_report.md");
    write_text(&report_path, &ledger.generate_report())?;
    tracing::info!(experiment = "e1", phase = "ledger_report", path = ?report_path);

    let phi_mean = mean(&phi_series);
    let phi_std = std(&phi_series);
    let phi_min = phi_series.iter().cloned().fold(f64::INFINITY, f64::min);
    let phi_max = phi_series.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let above_floor = phi_series.iter().filter(|&&p| p > PHI_FLOOR).count();
    let above_pct = above_floor as f64 / phi_series.len() as f64 * 100.0;
    let corr = pearson(&overall_series, &phi_series);
    let success = phi_mean > PHI_FLOOR && corr.abs() > 0.5;

    let summary = serde_json::json!({
        "experiment": "e1",
        "windows": WINDOWS,
        "samples_per_window": SAMPLES_PER_WINDOW,
        "base_quality": BASE_QUALITY,
        "criteria": { "phi_mean_above": PHI_FLOOR, "correlation_significant": "|r| > 0.5" },
        "phi": {
            "mean": phi_mean,
            "std": phi_std,
            "min": phi_min,
            "max": phi_max,
            "above_floor_pct": above_pct
        },
        "overall": { "mean": mean(&overall_series), "std": std(&overall_series) },
        "pearson_q_phi": corr,
        "success": success,
        "fase7_assurance": {
            "windows_sound_pct": sound_total as f64 / WINDOWS as f64 * 100.0,
            "windows_acceptable_pct": acceptable_total as f64 / WINDOWS as f64 * 100.0,
            "note": "SemanticValidity/Loopseal sao eixos ortogonais; nao compoem Phi"
        }
    });
    write_json(&root.join("e1_summary.json"), &summary)?;

    println!("========================================");
    println!("E1 Baseline — estado estacionário");
    println!(
        "  Φ: média {phi_mean:.4}  std {phi_std:.4}  min {phi_min:.4}  max {phi_max:.4}"
    );
    println!(
        "  Acima de {PHI_FLOOR:.2} (piso): {above_pct:.1}% das janelas"
    );
    println!("  overall médio: {:.4}", mean(&overall_series));
    println!("  Correlação Q–Φ (Pearson): {corr:.4}");
    println!("  Sucesso: {success}");
    let integrity = match &integrity_status {
        IntegrityStatus::Ok => "OK (dentro do horizonte formal TLC/Lean)".into(),
        IntegrityStatus::Broken { window_ids } => {
            let ids = window_ids.iter().map(u64::to_string).collect::<Vec<_>>().join(", ");
            format!("QUEBRADA (window_ids: {ids})")
        }
        IntegrityStatus::BeyondHorizon { len, max_windows } => format!(
            "OK (dados) — além do horizonte formal TLC/Lean (len={len} > MaxWindows={max_windows})"
        ),
    };
    println!(
        "  Ledger: {} entradas, média Φ {:.4}, integridade {integrity}",
        ledger.entries().len(),
        ledger.mean_phi()
    );
    println!(
        "  Fase 7: {:.1}% janelas com garantia sólida, {:.1}% aceitáveis (eixos ortogonais)",
        sound_total as f64 / WINDOWS as f64 * 100.0,
        acceptable_total as f64 / WINDOWS as f64 * 100.0
    );
    println!("  Artefatos: {}", root.display());
    println!("========================================");

    Ok(())
}