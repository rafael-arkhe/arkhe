//! Recurrency engine — the cohesive facade of the `491-AGI-CORTEX` substrate.
//!
//! Wires the perception loop (local recurrency), the arousal daemon (cellular),
//! the lateral field (character) and the global workspace (access) into one
//! stateful component that consumes a stream of stimulus frames and emits
//! [`RecurrencyTicket`]s. It also closes the *brain-body* loop (Box 3): after
//! an action is broadcast, [`RecurrencyEngine::act_and_listen`] waits on an
//! [`EnvironmentListener`] for a perceivable consequence and reflects it back
//! as a clamp on the perceptual stream.

use std::collections::VecDeque;

use nalgebra::DVector;

use crate::closure::closure_depth;
use crate::cycle::PerceptionLoop;
use crate::environment::{EnvFeedback, EnvironmentListener};
use crate::state::StateDaemon;
use crate::workspace::GlobalWorkspace;

/// Identifier of a stimulus frame entering the engine.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StimulusId(pub String);

/// Result of processing one stimulus frame.
#[derive(Clone, Debug, serde::Serialize)]
pub struct RecurrencyTicket {
    /// The stimulus this ticket was computed for.
    pub stimulus_id: StimulusId,
    /// Whether the local two-pass loop closed
    /// (`error_reduction > closure_threshold`).
    pub local_loop_closed: bool,
    /// Pass-2 relative error reduction of the perception cycle.
    pub error_reduction: f64,
    /// Causal-closure depth of the local recurrence (the `Φ` proxy).
    pub local_depth: f64,
    /// Whether the global workspace broadcast the content.
    pub access_granted: bool,
    /// Effective tick of the state daemon when the frame was processed.
    pub tick: u64,
}

/// Stateful end-to-end recurrency pipeline.
#[derive(Clone, Debug)]
pub struct RecurrencyEngine {
    /// Local perceptual substrate.
    pub perception: PerceptionLoop,
    /// Cellular-state (arousal) daemon.
    pub daemon: StateDaemon,
    /// Global access gate.
    pub workspace: GlobalWorkspace,
    /// Minimum number of frames required to measure causal closure.
    pub min_window: usize,
    window: VecDeque<DVector<f64>>,
    window_capacity: usize,
}

impl RecurrencyEngine {
    /// Create an engine over the given components.
    pub fn new(
        perception: PerceptionLoop,
        daemon: StateDaemon,
        workspace: GlobalWorkspace,
    ) -> Self {
        Self {
            perception,
            daemon,
            workspace,
            min_window: 4,
            window: VecDeque::with_capacity(32),
            window_capacity: 32,
        }
    }

    /// Process one stimulus frame and produce a ticket.
    ///
    /// The frame is gated by the daemon's drive gain, passed through the
    /// (regime-modulated) two-pass loop, and its causal-closure depth is
    /// measured over a sliding window of recent frames. Access is decided by
    /// [`GlobalWorkspace::offer_in_state`].
    pub fn process_stimulus(
        &mut self,
        stimulus_id: StimulusId,
        frame: &DVector<f64>,
    ) -> RecurrencyTicket {
        let tick = self.daemon.tick();
        let gated = frame * self.daemon.drive_gain();

        let mut loop_cfg = self.perception.clone();
        loop_cfg.feedback_gain = self.perception.feedback_gain * self.daemon.recurrence();
        let percept = loop_cfg.perceive(&gated);

        self.window.push_back(gated);
        while self.window.len() > self.window_capacity {
            self.window.pop_front();
        }

        // Causal closure over a sliding window of the (gated) input stream.
        let mut win: Vec<DVector<f64>> = self.window.iter().cloned().collect();
        while win.len() < self.min_window {
            win.push(win[win.len() - 1].clone());
        }
        let (states, drv) = loop_cfg.closed_loop_trajectory(&win);
        let local_depth = closure_depth(&states, &drv).depth;

        let access_granted = matches!(
            self.workspace.offer_in_state(local_depth, &self.daemon),
            crate::workspace::AccessDecision::Accessed
        );

        RecurrencyTicket {
            stimulus_id,
            local_loop_closed: percept.local_recurrence,
            error_reduction: percept.error_reduction,
            local_depth,
            access_granted,
            tick,
        }
    }

    /// Close the brain-body loop: wait for a consequence of `action_id`.
    pub fn act_and_listen(
        &self,
        action_id: &str,
        environment: &dyn EnvironmentListener,
        budget: u64,
    ) -> Option<EnvFeedback> {
        environment.wait_for_consequence(action_id, budget)
    }

    /// Reflect an environment echo back into the perceptual stream.
    ///
    /// The `perception_delta` is appended as the next frame (if its dimension
    /// matches the substrate), so the consequence of an action clamps the
    /// next perception cycle. Returns `true` if a frame was clamped.
    pub fn clamp_from_feedback(&mut self, feedback: &EnvFeedback) -> bool {
        let dim = self.perception.n;
        if feedback.perception_delta.len() != dim {
            return false;
        }
        let frame = DVector::from_iterator(dim, feedback.perception_delta.iter().copied());
        let norm = frame.norm();
        let frame = if norm > 1e-12 { frame / norm } else { frame };
        self.window.push_back(frame);
        while self.window.len() > self.window_capacity {
            self.window.pop_front();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::SimulatedEnvironment;
    use crate::state::ArousalRegime;

    fn engine(regime: ArousalRegime, threshold: f64) -> RecurrencyEngine {
        RecurrencyEngine::new(
            PerceptionLoop::default(),
            StateDaemon::new(regime),
            GlobalWorkspace::new(threshold),
        )
    }

    fn stream(n: usize, len: usize) -> Vec<DVector<f64>> {
        (0..len)
            .map(|t| {
                DVector::from_fn(n, |i, _| {
                    (0.5 * (i + 1) as f64 * t as f64 * 0.02).sin() + 0.1 * (i as f64)
                })
            })
            .collect()
    }

    #[test]
    fn alert_processes_frames_and_grants_access() {
        // Strong internal transport, weak input clamp: deep recurrence so the
        // sliding-window closure measurement clears the 0.3 access threshold.
        let loop_cfg = PerceptionLoop {
            feedback_gain: 0.5,
            coupling_scale: 3.0,
            ..Default::default()
        };
        let mut eng = RecurrencyEngine::new(
            loop_cfg,
            StateDaemon::new(ArousalRegime::Alert),
            GlobalWorkspace::new(0.3),
        );
        let frames = stream(8, 48);
        let mut last = None;
        for (i, f) in frames.iter().enumerate() {
            last = Some(eng.process_stimulus(StimulusId(format!("s{i}")), f));
        }
        let t = last.expect("at least one ticket");
        assert!(t.local_loop_closed, "default loop must close");
        assert!(
            t.local_depth > 0.3,
            "Alert must sustain deep closure, got {}",
            t.local_depth
        );
        assert!(t.access_granted, "Alert + depth 0.3+ must broadcast");
        assert!(t.tick >= 48);
    }

    #[test]
    fn deepsleep_suppresses_access_but_content_remains() {
        let loop_cfg = PerceptionLoop {
            feedback_gain: 0.5,
            coupling_scale: 3.0,
            ..Default::default()
        };
        let mut eng = RecurrencyEngine::new(
            loop_cfg,
            StateDaemon::new(ArousalRegime::DeepSleep),
            GlobalWorkspace::new(0.3),
        );
        let frames = stream(8, 48);
        let mut last = None;
        for (i, f) in frames.iter().enumerate() {
            last = Some(eng.process_stimulus(StimulusId(format!("s{i}")), f));
        }
        let t = last.expect("at least one ticket");
        assert!(t.local_loop_closed, "content must still be processed");
        assert!(
            !t.access_granted,
            "DeepSleep must suppress broadcast even with depth {}",
            t.local_depth
        );
    }

    #[test]
    fn brain_body_loop_closes_when_environment_echoes() {
        let mut eng = engine(ArousalRegime::Alert, 0.3);
        let env = SimulatedEnvironment::with_echo("osint_tweet_1", vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let fb = eng
            .act_and_listen("osint_tweet_1", &env, 100)
            .expect("environment must echo");
        assert!(eng.clamp_from_feedback(&fb), "echo must clamp the stream");
    }

    #[test]
    fn brain_body_loop_stays_open_without_echo() {
        let eng = engine(ArousalRegime::Alert, 0.3);
        let env = SimulatedEnvironment::silent();
        assert!(eng.act_and_listen("osint_tweet_1", &env, 100).is_none());
    }

    #[test]
    fn mismatched_echo_dimension_does_not_clamp() {
        let mut eng = engine(ArousalRegime::Alert, 0.3);
        assert!(
            !eng.clamp_from_feedback(&EnvFeedback {
                action_id: "x".into(),
                perception_delta: vec![1.0, 0.0], // n = 8, mismatch
            }),
            "wrong-dimension echo must be rejected"
        );
    }
}
