//! RF cell modulator — cellular actuation by RF + nanoparticles (US10064941B2).
//!
//! The patent maps onto the ARKHE hypergraph as a biological actuator:
//!
//! * **Z1** — the external RF command (the tool actuating the cell);
//! * **Z2** — the continuous physiological response ([Ca²⁺]ᵢ, gene expression);
//! * **Z3** — the discrete clinical outcome (glycemia) certified as evidence.
//!
//! The model is deterministic (no RNG) and reproduces the patent's *in vitro*
//! and *in vivo* dose–response (Fe₃O₄ @ 465 kHz, 50 kA/m, 30 min → ~5× Ca²⁺
//! peak and 2–5× gene expression). The [`I18CalciumGuard`] enforces the I18
//! invariance — that a bounded continuous variable stays within a safe window
//! and never runs away over repeated pulses — mirroring the same relative-drift
//! guarantee that `arkhe-neurogenesis` tracks on its PT-symmetric amplitude.

use arkhe_buzz_bridge::{
    CertificationStatus, DivergenceReport, EdgeType, EvidenceBundle, Zone,
    validate_hyperedge_firewall,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Nanoparticle mediating the RF-to-heat-to-ion-channel conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NanoparticleType {
    /// Iron oxide (Fe₃O₄) — 465 kHz carrier.
    IronOxide,
    /// Gold colloid — 13.5 MHz carrier.
    Gold,
    /// Endogenous ferritin (expressed by the target cell).
    Ferritin,
}

impl NanoparticleType {
    fn label(&self) -> &'static str {
        match self {
            NanoparticleType::IronOxide => "iron_oxide",
            NanoparticleType::Gold => "gold",
            NanoparticleType::Ferritin => "ferritin",
        }
    }
}

/// Gene target whose expression the actuator drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetGene {
    Insulin,
    Glp1,
    Pdx1,
    Reporter,
}

impl TargetGene {
    fn label(&self) -> &'static str {
        match self {
            TargetGene::Insulin => "insulin",
            TargetGene::Glp1 => "GLP-1",
            TargetGene::Pdx1 => "PDX-1",
            TargetGene::Reporter => "reporter",
        }
    }
}

/// Stimulation prescription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfStimulationConfig {
    pub frequency_hz: f64,
    pub field_strength_ka_m: f64,
    pub duration_ms: u64,
    pub nanoparticle: NanoparticleType,
    pub target_gene: TargetGene,
    /// I18 bound — peak [Ca²⁺]ᵢ above this value marks an excitotoxic runaway.
    pub calcium_peak_threshold_nm: f64,
    /// Whether the trace models an in-vivo outcome (reports glycemia).
    pub in_vivo: bool,
}

impl Default for RfStimulationConfig {
    fn default() -> Self {
        Self {
            frequency_hz: 465_000.0,
            field_strength_ka_m: 50.0,
            duration_ms: 600_000,
            nanoparticle: NanoparticleType::IronOxide,
            target_gene: TargetGene::Insulin,
            calcium_peak_threshold_nm: 800.0,
            in_vivo: false,
        }
    }
}

/// Result of a single stimulation (deterministic function of the config).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StimulationResult {
    pub calcium_peak_nm: f64,
    pub calcium_time_series: Vec<(f64, f64)>,
    pub gene_expression_fold: f64,
    pub glucose_mg_dl: Option<f64>,
    pub duration_ms: u64,
    pub timestamp: String,
}

/// Errors raised by the actuator.
#[derive(Debug, thiserror::Error)]
pub enum ActuatorError {
    #[error("zero-duration stimulation rejected")]
    ZeroDuration,
    #[error("field strength {0:.1} kA/m outside the patent-supported 5..110 range")]
    FieldOutOfRange(f64),
    #[error("I18 runaway: {0}")]
    I18Runaway(&'static str),
    #[error("firewall rejected the evidence hyperedge: {0}")]
    Firewall(String),
}

/// Outcome of a single I18 guard observation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct I18Step {
    pub time_s: f64,
    pub peak_nm: f64,
    pub within_bound: bool,
    pub within_uptick: bool,
}

/// Aggregate verdict produced by the I18 calcium guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I18Verdict {
    Proceed,
    Abort,
}

/// Aggregate record from running the I18 guard over a stimulation train.
#[derive(Debug, Clone)]
pub struct I18Audit {
    pub verdict: I18Verdict,
    pub steps: Vec<I18Step>,
}

/// I18 continuous-variable guard for the [Ca²⁺] trace.
///
/// Two rules keep the signal inside its safe window:
/// 1. **Bound** — every peak must stay under the configured toxicity threshold.
/// 2. **Runaway** — once a response plateau is established, a later pulse may not
///    push the peak more than `max_upticks_fraction` above that plateau (guards
///    against the cumulative excitotoxic drift the patent's repeated-pulse
///    protocol must avoid).
///
/// Basal readings (at or below `baseline_nm`) do not disturb the plateau, and
/// the first genuine response peak merely *establishes* it — so the basal→peak
/// jump is a lawful response, not a runaway.
pub struct I18CalciumGuard {
    baseline_nm: f64,
    bound_nm: f64,
    max_upticks_fraction: f64,
    plateau_nm: Option<f64>,
}

impl I18CalciumGuard {
    pub fn new(baseline_nm: f64, bound_nm: f64, max_upticks_fraction: f64) -> Self {
        Self {
            baseline_nm,
            bound_nm,
            max_upticks_fraction,
            plateau_nm: None,
        }
    }

    pub fn observe(&mut self, time_s: f64, peak_nm: f64) -> I18Step {
        let within_bound = peak_nm <= self.bound_nm;
        let within_uptick = if peak_nm <= self.baseline_nm {
            // Basal reading: no plateau disturbance.
            true
        } else {
            match self.plateau_nm {
                // First genuine response: establishes the plateau, always allowed.
                None => {
                    self.plateau_nm = Some(peak_nm);
                    true
                }
                Some(plateau) => {
                    let allowed = plateau * (1.0 + self.max_upticks_fraction);
                    let within = peak_nm <= allowed;
                    if within {
                        // Within tolerance: ratchet up to the new level.
                        self.plateau_nm = Some(plateau.max(peak_nm));
                    }
                    within
                }
            }
        };
        I18Step {
            time_s,
            peak_nm,
            within_bound,
            within_uptick,
        }
    }

    pub fn run(&mut self, peaks: &[(f64, f64)]) -> I18Audit {
        let mut steps = Vec::with_capacity(peaks.len());
        for &(t, peak) in peaks {
            let step = self.observe(t, peak);
            steps.push(step);
            if !step.within_bound || !step.within_uptick {
                return I18Audit {
                    verdict: I18Verdict::Abort,
                    steps,
                };
            }
        }
        I18Audit {
            verdict: I18Verdict::Proceed,
            steps,
        }
    }
}

/// Deterministic RF cell modulator bound to a stimulation config.
#[derive(Debug, Clone)]
pub struct RfCellModulator {
    config: RfStimulationConfig,
}

impl RfCellModulator {
    pub fn new(config: RfStimulationConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &RfStimulationConfig {
        &self.config
    }

    fn validate(&self) -> Result<(), ActuatorError> {
        if self.config.duration_ms == 0 {
            return Err(ActuatorError::ZeroDuration);
        }
        if !(5.0..=110.0).contains(&self.config.field_strength_ka_m) {
            return Err(ActuatorError::FieldOutOfRange(self.config.field_strength_ka_m));
        }
        Ok(())
    }

    /// Patent-model calcium peak (nM) for the chosen particle and field.
    fn calcium_peak(&self) -> f64 {
        let base = match self.config.nanoparticle {
            NanoparticleType::IronOxide => 550.0,
            NanoparticleType::Gold => 480.0,
            NanoparticleType::Ferritin => 350.0,
        };
        // Linear field response centered on the 50 kA/m patent reference (±25 nM per 10 kA/m).
        let boost = ((self.config.field_strength_ka_m - 50.0) / 10.0).clamp(-5.0, 5.0) * 25.0;
        base + boost
    }

    /// Deterministic gene-expression fold change from the calcium response.
    fn gene_expression_fold(&self) -> f64 {
        let peak = self.calcium_peak();
        // Monotone: 1.0x at basal, ~2.7x at the 550 nM in-vitro peak.
        1.0 + (peak - 100.0) / (500.0 - 100.0) * 1.5
    }

    /// Run one stimulation and produce a deterministic result.
    pub fn stimulate(&self) -> Result<StimulationResult, ActuatorError> {
        self.validate()?;
        let peak = self.calcium_peak();
        let fold = self.gene_expression_fold();

        // 10-point synthetic trace: rise to peak by t=300 s, then decay.
        let mut series = Vec::with_capacity(10);
        for i in 0..10 {
            let t = i as f64 * 60.0;
            let ca = if t <= 300.0 {
                100.0 + (peak - 100.0) * (t / 300.0)
            } else {
                100.0 + (peak - 100.0) * ((1.0 - (t - 300.0) / 300.0).max(0.0))
            };
            series.push((t, ca));
        }

        let glucose = if self.config.in_vivo {
            // Patent: ~30% glycemia reduction correlated with the gene lift.
            let reduction = 0.30 * ((fold - 1.0) / 1.5).clamp(0.0, 1.0);
            Some(120.0 * (1.0 - reduction))
        } else {
            None
        };

        Ok(StimulationResult {
            calcium_peak_nm: peak,
            calcium_time_series: series,
            gene_expression_fold: fold,
            glucose_mg_dl: glucose,
            duration_ms: self.config.duration_ms,
            timestamp: Utc::now().to_rfc3339(),
        })
    }

    /// Run a train of `n` pulses and return the per-pulse results.
    pub fn simulate_pulses(&self, n: u32) -> Result<Vec<StimulationResult>, ActuatorError> {
        (0..n).map(|_| self.stimulate()).collect()
    }

    /// Convert a simulated result into the repository-standard evidence bundle.
    pub fn evidence_for(&self, result: &StimulationResult, step: &I18Step) -> EvidenceBundle {
        let supported = step.within_bound
            && step.within_uptick
            && result.gene_expression_fold >= 1.5;
        let certification = if supported {
            CertificationStatus::Supported
        } else {
            CertificationStatus::Inconclusive
        };
        let mut violations = Vec::new();
        if !step.within_bound {
            violations.push("I18_calcium_bound".to_string());
        }
        if !step.within_uptick {
            violations.push("I18_calcium_runaway".to_string());
        }

        let mut observations_forward = vec![
            format!("Ca2+_peak={:.1} nM", result.calcium_peak_nm),
            format!("gene_expr={:.2}x", result.gene_expression_fold),
        ];
        if let Some(g) = result.glucose_mg_dl {
            observations_forward.push(format!("glucose={:.1} mg/dL", g));
        }

        EvidenceBundle {
            id: format!(
                "rf-patent-{}-t{:.0}s",
                self.config.nanoparticle.label(),
                step.time_s
            ),
            hypothesis: format!(
                "RF {:.0} kHz via {} gates TRPV1 and lifts {} expression",
                self.config.frequency_hz / 1000.0,
                self.config.nanoparticle.label(),
                self.config.target_gene.label(),
            ),
            baseline_hash: format!("{:?}", self.config),
            pump_sequence: vec![
                format!("frequency={:.3}kHz", self.config.frequency_hz / 1000.0),
                format!("field={:.1}kA/m", self.config.field_strength_ka_m),
                format!("duration={}min", self.config.duration_ms / 60_000),
                format!("nanoparticle={}", self.config.nanoparticle.label()),
            ],
            probe: "calcium_peak".into(),
            counterfactual: "No RF or no nanoparticle uptake".into(),
            observations_forward,
            observations_reverse: vec![
                "baseline_Ca2+=100 nM".to_string(),
                "TRPV1_activation_temp=42degC".into(),
            ],
            divergences: DivergenceReport {
                structural: Some(result.calcium_peak_nm - 100.0),
                observational: Some(result.gene_expression_fold - 1.0),
                invariant_violations: violations,
                threshold: self.config.calcium_peak_threshold_nm,
                has_divergence: !supported,
            },
            witness: None,
            certification,
            timestamp: result.timestamp.clone(),
        }
    }
}

/// Confirm the Z1→Z2→Z3 mapping the evidence spans respects the firewall:
/// the continuous trace (Z2) may only translate to the discrete outcome (Z3).
pub fn evidence_stays_in_firewall() -> Result<(), ActuatorError> {
    let nodes = vec![
        ("z2".to_string(), Zone::Z2_Continuous),
        ("z3".to_string(), Zone::Z3_Discrete),
    ];
    let edge = EdgeType::TranslatesToPrimitive.as_str();
    validate_hyperedge_firewall(&nodes, edge).map_err(|e| ActuatorError::Firewall(e.to_string()))
}