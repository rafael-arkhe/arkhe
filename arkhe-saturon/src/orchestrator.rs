//! The event-driven orchestrator — the heart of ARKHE-SATURON.
//!
//! It is a **single-consumer async loop**: one event is handled at a time, so
//! at most one solver run is ever in flight. Rust does no math — it registers
//! hypotheses, delegates the symbolic check to a [`ScriptRunner`], parses the
//! verdict, and folds the outcome into [`SaturonState`] via the reducer.

use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::mpsc;

use crate::bridge::ScriptRunner;
use crate::event::{SaturonCommand, SaturonEvent};
use crate::state::{Action, SaturonState};
use crate::types::{Hypothesis, HypothesisStatus, VerificationResult, VerificationType};
use crate::verify::{build_check_script, parse_verdict};

/// Orchestrates the hypothesis-verification pipeline.
pub struct SaturonOrchestrator {
    state: Arc<Mutex<SaturonState>>,
    runner: Arc<dyn ScriptRunner>,
    commands: mpsc::Sender<SaturonCommand>,
}

impl SaturonOrchestrator {
    pub fn new(
        state: Arc<Mutex<SaturonState>>,
        runner: Arc<dyn ScriptRunner>,
        commands: mpsc::Sender<SaturonCommand>,
    ) -> Self {
        Self {
            state,
            runner,
            commands,
        }
    }

    /// Consume events until the channel closes.
    pub async fn run(&self, mut events: mpsc::Receiver<SaturonEvent>) {
        while let Some(event) = events.recv().await {
            self.handle(event).await;
        }
    }

    /// Handle a single event. Public so it can be driven directly in tests.
    pub async fn handle(&self, event: SaturonEvent) {
        match event {
            SaturonEvent::ArxivDiscovery {
                hypothesis_id,
                arxiv_toon_id,
                hypothesis_text,
                math_syntax,
                variables,
            } => {
                let hypothesis = Hypothesis {
                    id: hypothesis_id.clone(),
                    arxiv_toon_id,
                    text: hypothesis_text,
                    math_syntax,
                    variables,
                    status: HypothesisStatus::Proposed,
                };
                let script = build_check_script(&hypothesis);

                // Short, await-free critical section: register + mark verifying.
                {
                    let mut state = self.lock();
                    state.reduce(Action::RegisterHypothesis(hypothesis));
                    state.reduce(Action::BeginVerification {
                        id: hypothesis_id.clone(),
                    });
                }

                match self.runner.run(&script).await {
                    Ok(output) => match parse_verdict(&output) {
                        Ok((passed, detail)) => {
                            let result = VerificationResult {
                                hypothesis_id: hypothesis_id.clone(),
                                verifier: "did:arkhe:saturon-verifier".to_string(),
                                check_type: VerificationType::AlgebraicIdentity,
                                passed,
                                detail,
                                solver_used: "external-solver".to_string(),
                            };
                            self.apply_result(result).await;
                        }
                        Err(e) => {
                            self.fail(&hypothesis_id, format!("unparseable solver output: {e}"))
                                .await;
                        }
                    },
                    Err(e) => {
                        self.fail(&hypothesis_id, format!("bridge error: {e}"))
                            .await;
                    }
                }
            }
            SaturonEvent::VerificationResultReceived { result } => {
                self.apply_result(result).await;
            }
            SaturonEvent::ScriptGenerationError {
                hypothesis_id,
                error_log,
            } => {
                self.fail(&hypothesis_id, error_log).await;
            }
        }
    }

    async fn apply_result(&self, result: VerificationResult) {
        let passed = result.passed;
        let id = result.hypothesis_id.clone();
        let detail = result.detail.clone();

        // Reduce inside a temporary; the guard drops before the await below.
        self.lock().reduce(Action::ApplyResult { result });

        let command = if passed {
            SaturonCommand::HypothesisVerified { hypothesis_id: id }
        } else {
            SaturonCommand::HypothesisRejected {
                hypothesis_id: id,
                reason: detail,
            }
        };
        let _ = self.commands.send(command).await;
    }

    async fn fail(&self, id: &str, log: String) {
        self.lock().reduce(Action::MarkErrored {
            id: id.to_string(),
            log: log.clone(),
        });
        let _ = self
            .commands
            .send(SaturonCommand::ShowError {
                hypothesis_id: id.to_string(),
                message: log,
            })
            .await;
    }

    fn lock(&self) -> MutexGuard<'_, SaturonState> {
        self.state.lock().expect("saturon state mutex poisoned")
    }
}
