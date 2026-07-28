//! End-to-end pipeline tests for ARKHE-SATURON.
//!
//! These drive the real orchestrator over real channels, with the solver
//! swapped for a [`MockRunner`] so no external process, SymPy, or HPC is
//! required. They validate the architecture: discovery -> dispatch -> verdict
//! -> reduced state -> emitted command.

use std::sync::{Arc, Mutex};

use arkhe_saturon::{
    BridgeError, HypothesisStatus, MockRunner, PhysicalVariable, SaturonCommand, SaturonEvent,
    SaturonOrchestrator, SaturonState, ScriptRunner,
};
use tokio::sync::mpsc;

fn discovery(id: &str) -> SaturonEvent {
    SaturonEvent::ArxivDiscovery {
        hypothesis_id: id.to_string(),
        arxiv_toon_id: "TOON-ARXIV-SATURON-2024".to_string(),
        hypothesis_text: "Saturons saturate the Bekenstein bound via high-entropy microstates."
            .to_string(),
        math_syntax: "S = k_B*c^3*A/(4*G*hbar)".to_string(),
        variables: vec![PhysicalVariable {
            name: "horizon area".to_string(),
            symbol: "A".to_string(),
            unit: "m^2".to_string(),
            description: "area of the black-hole surface".to_string(),
        }],
    }
}

/// Spin up an orchestrator, feed it `events`, wait for the loop to drain, then
/// return the final state and every command it emitted.
async fn drive(
    runner: Arc<dyn ScriptRunner>,
    events: Vec<SaturonEvent>,
) -> (SaturonState, Vec<SaturonCommand>) {
    let state = Arc::new(Mutex::new(SaturonState::new()));
    let (cmd_tx, mut cmd_rx) = mpsc::channel(32);
    let orch = SaturonOrchestrator::new(state.clone(), runner, cmd_tx);

    let (ev_tx, ev_rx) = mpsc::channel(32);
    let handle = tokio::spawn(async move { orch.run(ev_rx).await });

    for e in events {
        ev_tx.send(e).await.unwrap();
    }
    drop(ev_tx); // closes the event stream -> run() returns
    handle.await.unwrap(); // orch dropped here -> cmd_tx closed

    let mut commands = Vec::new();
    while let Some(c) = cmd_rx.recv().await {
        commands.push(c);
    }
    let final_state = Arc::try_unwrap(state).unwrap().into_inner().unwrap();
    (final_state, commands)
}

#[tokio::test]
async fn pipeline_passes_and_verifies() {
    let (state, commands) = drive(Arc::new(MockRunner::passing()), vec![discovery("HYP-001")]).await;

    assert_eq!(state.status_of("HYP-001"), Some(HypothesisStatus::Verified));
    assert_eq!(state.verified_count(), 1);
    assert_eq!(state.results.len(), 1);
    assert!(state.results[0].passed);

    assert_eq!(
        commands,
        vec![SaturonCommand::HypothesisVerified {
            hypothesis_id: "HYP-001".to_string()
        }]
    );
}

#[tokio::test]
async fn pipeline_fails_and_rejects() {
    let (state, commands) = drive(
        Arc::new(MockRunner::failing("residual=1e-3 != 0")),
        vec![discovery("HYP-002")],
    )
    .await;

    assert_eq!(state.status_of("HYP-002"), Some(HypothesisStatus::Rejected));
    assert_eq!(state.verified_count(), 0);

    assert_eq!(
        commands,
        vec![SaturonCommand::HypothesisRejected {
            hypothesis_id: "HYP-002".to_string(),
            reason: "residual=1e-3 != 0".to_string(),
        }]
    );
}

#[tokio::test]
async fn bridge_error_marks_errored_and_shows_error() {
    let (state, commands) = drive(
        Arc::new(MockRunner::erroring(BridgeError::Timeout { secs: 180 })),
        vec![discovery("HYP-003")],
    )
    .await;

    assert_eq!(state.status_of("HYP-003"), Some(HypothesisStatus::Errored));
    assert_eq!(commands.len(), 1);
    match &commands[0] {
        SaturonCommand::ShowError {
            hypothesis_id,
            message,
        } => {
            assert_eq!(hypothesis_id, "HYP-003");
            assert!(message.contains("timed out"));
        }
        other => panic!("expected ShowError, got {other:?}"),
    }
}

#[tokio::test]
async fn unparseable_solver_output_errors() {
    // Solver returns 200-OK garbage with no VERDICT line.
    let (state, commands) = drive(
        Arc::new(MockRunner::with_output(Ok("garbage, no verdict".to_string()))),
        vec![discovery("HYP-004")],
    )
    .await;

    assert_eq!(state.status_of("HYP-004"), Some(HypothesisStatus::Errored));
    assert!(matches!(commands[0], SaturonCommand::ShowError { .. }));
}

#[tokio::test]
async fn multiple_hypotheses_processed_in_order() {
    let (state, commands) = drive(
        Arc::new(MockRunner::passing()),
        vec![discovery("A"), discovery("B"), discovery("C")],
    )
    .await;

    assert_eq!(state.verified_count(), 3);
    assert_eq!(commands.len(), 3);
    // Single-consumer loop => commands arrive in submission order.
    assert_eq!(
        commands,
        vec![
            SaturonCommand::HypothesisVerified {
                hypothesis_id: "A".to_string()
            },
            SaturonCommand::HypothesisVerified {
                hypothesis_id: "B".to_string()
            },
            SaturonCommand::HypothesisVerified {
                hypothesis_id: "C".to_string()
            },
        ]
    );
}

#[tokio::test]
async fn out_of_band_result_is_folded_in() {
    use arkhe_saturon::{VerificationResult, VerificationType};

    // First discover (and verify) a hypothesis, then deliver a *separate*
    // out-of-band failing result for it via VerificationResultReceived.
    let events = vec![
        discovery("HYP-OOB"),
        SaturonEvent::VerificationResultReceived {
            result: VerificationResult {
                hypothesis_id: "HYP-OOB".to_string(),
                verifier: "did:arkhe:external".to_string(),
                check_type: VerificationType::DimensionalConsistency,
                passed: false,
                detail: "units mismatch".to_string(),
                solver_used: "manual".to_string(),
            },
        },
    ];
    let (state, commands) = drive(Arc::new(MockRunner::passing()), events).await;

    // The later out-of-band failing result wins.
    assert_eq!(
        state.status_of("HYP-OOB"),
        Some(HypothesisStatus::Rejected)
    );
    assert_eq!(state.results.len(), 2);
    assert_eq!(commands.len(), 2);
    assert!(matches!(
        commands[1],
        SaturonCommand::HypothesisRejected { .. }
    ));
}
