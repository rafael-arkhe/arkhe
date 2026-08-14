//! Consensus finality through **loop closure**.
//!
//! A block is not final on production alone. It becomes final when, across the
//! P2P overlay, a **majority of the returned echoes interfere constructively**
//! with the block's predicted topology — i.e. the average cosine of the phase
//! difference between each node's echo and the candidate's Chern–Simons phase
//! is positive (the loop is closed rather than left open).

use crate::block::TimeBlock;
use crate::retro::EchoSignal;

/// The phase-interference cosine between a peer echo and the candidate phase.
///
/// `> 0` ⇒ pointing the same way (constructive). `≤ 0` ⇒ destructive (the
/// peer disagrees, the loop is open there).
pub fn interference_cos(echo: &EchoSignal, candidate_phase: f64) -> f64 {
    (echo.phase - candidate_phase).cos()
}

/// The loop-closure finality verdict over a set of echoes.
#[derive(Debug, Clone, Copy)]
pub struct Finality {
    /// Whether the block is considered final (loop closed).
    pub block_final: bool,
    /// Number of echoes that interfered constructively.
    pub constructive_votes: usize,
    /// Total number of echoes considered.
    pub total_echoes: usize,
    /// Mean `cos` of the phase difference (loop coherence).
    pub mean_interference: f64,
}

/// Decide finality by constructive-interference majority.
///
/// `constructive_tol` bounds what counts as "constructive" (a phase cosine).
/// `majority` is the fraction of echoes that must confirm.
pub fn loop_closure(
    candidate: &TimeBlock,
    echoes: &[EchoSignal],
    constructive_tol: f64,
    majority: f64,
) -> Finality {
    let total = echoes.len();
    if total == 0 {
        return Finality {
            block_final: false,
            constructive_votes: 0,
            total_echoes: 0,
            mean_interference: 0.0,
        };
    }
    let cos: Vec<f64> = echoes
        .iter()
        .map(|e| interference_cos(e, candidate.phase))
        .collect();
    let constructive = cos.iter().filter(|&&c| c > constructive_tol).count();
    let mean: f64 = cos.iter().sum::<f64>() / total as f64;
    let block_final = (constructive as f64) >= majority * (total as f64) && mean > 0.0;
    Finality {
        block_final,
        constructive_votes: constructive,
        total_echoes: total,
        mean_interference: mean,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::TimeBlock;

    fn block(phase: f64) -> TimeBlock {
        TimeBlock::new(
            1,
            crate::block::BlockHash([0u8; 32]),
            0,
            0.5,
            0.9,
            phase,
            1.0,
            vec![0.1, 0.04, 0.01],
        )
    }

    fn echo(id: u64, phase: f64) -> EchoSignal {
        EchoSignal::new(id, 1, 1, 0.0, 4.0, phase, 0.4, 0.5, vec![])
    }

    #[test]
    fn majority_constructive_closes_the_loop() {
        // 3 of 4 echoes share the block phase -> majority constructive.
        let candidate = block(0.3);
        let echoes = vec![
            echo(1, 0.31),
            echo(2, 0.29),
            echo(3, 0.32),
            echo(4, std::f64::consts::PI), // adversarial / out of phase
        ];
        let f = loop_closure(&candidate, &echoes, 0.5, 0.5);
        assert!(f.block_final, "block should finalise: {f:?}");
        assert_eq!(f.constructive_votes, 3);
        assert_eq!(f.total_echoes, 4);
        assert!(f.mean_interference > 0.0);
    }

    #[test]
    fn minority_constructive_leaves_loop_open() {
        let candidate = block(0.3);
        let echoes = vec![
            echo(1, 0.3),
            echo(2, std::f64::consts::PI),
            echo(3, std::f64::consts::PI),
        ];
        let f = loop_closure(&candidate, &echoes, 0.5, 0.5);
        assert!(!f.block_final, "block must stay open: {f:?}");
    }

    #[test]
    fn no_echoes_never_final() {
        let candidate = block(0.3);
        let f = loop_closure(&candidate, &[], 0.5, 0.5);
        assert!(!f.block_final);
    }
}