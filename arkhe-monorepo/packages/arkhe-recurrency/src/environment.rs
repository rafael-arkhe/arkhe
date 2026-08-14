//! Environment listener — the *brain-body* loop (Box 3).
//!
//! In the `491-AGI-CORTEX` context the "environment" is not an Atari game; it
//! is the execution infrastructure (APIs, blockchain, honeypots). The loop is
//! closed when an action the agent broadcast through the global workspace
//! produces a *perceivable consequence* — a reply to an OSINT tweet, a CCDI
//! state change on-chain — that is reflected back as a `perception_delta`.
//!
//! This module defines the pure contract (a deterministic, non-async trait,
//! so it stays testable inside the library) plus a fully deterministic
//! [`SimulatedEnvironment`] for unit tests. Async adapters over real
//! infrastructure live in the daemon/service layer, not here.

/// A perceivable consequence of a broadcast action, reflected back into the
/// perceptual loop as a clamp (the "echo" that closes the brain-body loop).
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct EnvFeedback {
    /// Identifier of the action that produced this consequence.
    pub action_id: String,
    /// Numeric perception delta, e.g. `[reach, sentiment_shift]` or
    /// `[1.0]` for a successful on-chain CCDI update.
    pub perception_delta: Vec<f64>,
}

/// Contract for waiting on a consequence of a broadcast action.
///
/// Returns `Some(feedback)` if the environment echoed within `budget`
/// (abstract work units; concrete timeouts are the transport layer's concern),
/// or `None` if the loop stays open — the state where, per the paper, access
/// fails and no global broadcast can close the cycle.
pub trait EnvironmentListener {
    /// Wait (in abstract `budget` units) for a perceivable consequence of
    /// `action_id`.
    fn wait_for_consequence(&self, action_id: &str, budget: u64) -> Option<EnvFeedback>;
}

/// Deterministic environment for unit tests: a lookup table of known echoes.
///
/// A query matches if `action_id` equals one of the stored action prefixes
/// (exact string match); the stored `perception_delta` is returned verbatim.
#[derive(Clone, Debug, serde::Serialize)]
pub struct SimulatedEnvironment {
    /// `(action_id, perception_delta)` pairs that echo back.
    pub echoes: Vec<(String, Vec<f64>)>,
}

impl SimulatedEnvironment {
    /// Build an environment that echoes nothing (loop always open).
    pub fn silent() -> Self {
        Self { echoes: Vec::new() }
    }

    /// Build an environment with one known echo.
    pub fn with_echo(action_id: &str, perception_delta: Vec<f64>) -> Self {
        Self {
            echoes: vec![(action_id.to_string(), perception_delta)],
        }
    }
}

impl EnvironmentListener for SimulatedEnvironment {
    fn wait_for_consequence(&self, action_id: &str, _budget: u64) -> Option<EnvFeedback> {
        self.echoes
            .iter()
            .find(|(id, _)| id == action_id)
            .map(|(id, delta)| EnvFeedback {
                action_id: id.clone(),
                perception_delta: delta.clone(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_action_echoes() {
        let env = SimulatedEnvironment::with_echo("osint_tweet_1", vec![4200.0, 0.3]);
        let fb = env.wait_for_consequence("osint_tweet_1", 100).expect("must echo");
        assert_eq!(fb.action_id, "osint_tweet_1");
        assert_eq!(fb.perception_delta, vec![4200.0, 0.3]);
    }

    #[test]
    fn unknown_action_leaves_loop_open() {
        let env = SimulatedEnvironment::with_echo("osint_tweet_1", vec![1.0]);
        assert!(env.wait_for_consequence("osint_tweet_2", 100).is_none());
    }

    #[test]
    fn silent_environment_never_echoes() {
        let env = SimulatedEnvironment::silent();
        assert!(env.wait_for_consequence("anything", 1_000).is_none());
    }
}
