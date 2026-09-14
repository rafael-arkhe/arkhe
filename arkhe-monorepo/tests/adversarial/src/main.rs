//! Standalone runner for the adversarial corpus.
//!
//! Executes every declarative case against `arkhe-safe-manifold`, prints the
//! per-case verdict table and exits non-zero if any case deviates from its
//! declared expectations.
//!
//! ```text
//! cd arkhe-monorepo
//! cargo run -p arkhe-adversarial-corpus
//! ```
//!
//! The runner performs **no** side effects: it only reads `cases/*.json`
//! (inside this crate) and calls the pure `arkhe-safe-manifold` API.

#![forbid(unsafe_code)]

use std::process::ExitCode;

use arkhe_adversarial_corpus::{
    corpus_dir, evaluate, invariant_ids, load_corpus, render_report, run_case, CaseKind,
};

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("adversarial corpus runner: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let dir = corpus_dir();
    let cases = load_corpus()?;
    println!("corpus dir: {}\n", dir.display());

    let mut outcomes = Vec::with_capacity(cases.len());
    let mut failures: Vec<String> = Vec::new();

    for case in &cases {
        let outcome = run_case(case)?;
        failures.extend(
            evaluate(case, &outcome)
                .into_iter()
                .map(|mismatch| format!("[{}] {mismatch}", case.id)),
        );
        outcomes.push(outcome);
    }

    print!("{}", render_report(&outcomes));

    println!("\nper-case detail");
    println!("{}", "-".repeat(118));
    for (case, outcome) in cases.iter().zip(&outcomes) {
        println!(
            "[{}] {} ({}) -> target {}",
            case.kind.as_str(),
            case.id,
            case.target_invariant,
            case.title
        );
        println!("  mechanism        : {}", case.mechanism);
        println!(
            "  detected         : {:?} (violation_count = {})",
            invariant_ids(&outcome.detected),
            outcome.violation_count
        );
        println!(
            "  SafeState::new   : {}",
            if outcome.safe_state_rejected {
                format!("rejected ({:?})", outcome.safe_state_error)
            } else {
                "accepted".to_string()
            }
        );
        println!(
            "  neron_model      : {}",
            if outcome.neron_residual.is_empty() {
                format!(
                    "repaired -> {:?}; SafeState::new accepted = {}",
                    invariant_ids(&outcome.neron_residual),
                    outcome.neron_output_accepted
                )
            } else {
                format!(
                    "NOT repaired -> still blocked on {:?}; SafeState::new accepted = {}",
                    invariant_ids(&outcome.neron_residual),
                    outcome.neron_output_accepted
                )
            }
        );
        match (&outcome.neutralized_violations, outcome.neutralized_accepted) {
            (None, _) => println!("  neutralization   : n/a (benign control)"),
            (Some(violations), accepted) => println!(
                "  neutralization   : violations = {:?}; SafeState::new accepted = {:?}",
                invariant_ids(violations),
                accepted
            ),
        }
        let witnesses: Vec<String> = outcome
            .witnesses
            .iter()
            .filter(|(_, values)| !values.is_empty())
            .map(|(key, values)| format!("{key}={values:?}"))
            .collect();
        println!(
            "  witnesses        : {}",
            if witnesses.is_empty() {
                "(none)".to_string()
            } else {
                witnesses.join(", ")
            }
        );
        if case.kind == CaseKind::Adversarial {
            println!("  rationale        : {}", case.rationale);
        }
        println!();
    }

    if failures.is_empty() {
        println!(
            "OK: {} cases evaluated; every case matched its declared expectations.",
            outcomes.len()
        );
        Ok(ExitCode::SUCCESS)
    } else {
        eprintln!("FAIL: {} expectation mismatch(es):", failures.len());
        for failure in &failures {
            eprintln!("  {failure}");
        }
        Ok(ExitCode::FAILURE)
    }
}
