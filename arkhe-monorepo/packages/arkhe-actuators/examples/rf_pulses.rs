//! RF cell modulator demo (US10064941B2).
//!
//! Runs a repeated-pulse train, guards it with the I18 calcium invariant and
//! prints the resulting evidence bundle. Deterministic — no RNG.

use arkhe_actuators::{
    I18CalciumGuard, RfCellModulator, RfStimulationConfig, evidence_stays_in_firewall,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // In-vivo Insulin protocol: Fe₃O₄ @ 465 kHz, 50 kA/m, 10 min.
    let modulator = RfCellModulator::new(RfStimulationConfig {
        in_vivo: true,
        ..RfStimulationConfig::default()
    });

    println!("== RF cell modulator (US10064941B2) ==");
    println!("config: {:?}\n", modulator.config().nanoparticle);

    let pulses = modulator.simulate_pulses(5)?;
    let mut guard = I18CalciumGuard::new(100.0, 800.0, 0.5);

    for (i, result) in pulses.iter().enumerate() {
        let step = guard.observe(i as f64 * 10.0, result.calcium_peak_nm);
        let bundle = modulator.evidence_for(result, &step);
        println!(
            "pulse {i}: Ca2+={:.0} nM  fold={:.2}x  within_bound={} within_uptick={}  cert={:?}",
            result.calcium_peak_nm,
            result.gene_expression_fold,
            step.within_bound,
            step.within_uptick,
            bundle.certification,
        );
    }

    // Confirm the Z1→Z2→Z3 evidence respects the epistemic firewall.
    evidence_stays_in_firewall()?;
    println!("\nfirewall: Z2 (Ca²⁺) → Z3 (glycemia) via TRANSLATES_TO_PRIMITIVE: OK");
    Ok(())
}