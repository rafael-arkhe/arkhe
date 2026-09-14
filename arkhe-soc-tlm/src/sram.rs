//! SRAM D/X double buffer with the IFS expansion (bandIso `toFun`).
//!
//! D is the 8-node fundamental domain; X is the 16-node expanded space. Expand
//! writes X = [D ; IFS(D)] where the upper half is the contractive map
//! `w*base + (1-w)*neighbor`, then the active buffer swaps atomically.

use crate::payload::DOMAIN_NODES;

pub const FULL_NODES: usize = 2 * DOMAIN_NODES; // 16

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferId {
    Domain,
    Full,
}

#[derive(Debug, Clone)]
pub struct DoubleBuffer {
    domain: [f64; DOMAIN_NODES],
    full: [f64; FULL_NODES],
    active: BufferId,
    ready_sequence: Option<u64>,
}

impl DoubleBuffer {
    pub fn new(domain: [f64; DOMAIN_NODES]) -> Self {
        let mut b = Self {
            domain,
            full: [0.0; FULL_NODES],
            active: BufferId::Domain,
            ready_sequence: None,
        };
        b.expand(0, [100; DOMAIN_NODES]);
        b
    }

    pub fn domain(&self) -> &[f64; DOMAIN_NODES] {
        &self.domain
    }
    pub fn full(&self) -> &[f64; FULL_NODES] {
        &self.full
    }
    pub fn active(&self) -> BufferId {
        self.active
    }
    pub fn ready_sequence(&self) -> Option<u64> {
        self.ready_sequence
    }

    pub fn write_domain(&mut self, idx: usize, value: f64) {
        assert!(idx < DOMAIN_NODES, "domain index out of range");
        self.domain[idx] = value;
    }

    /// IFS expansion of D into X; `weights[i]` in 0..=100 is the contraction w=weights/100.
    pub fn expand(&mut self, sequence: u64, weights: [u8; DOMAIN_NODES]) {
        self.ready_sequence = None; // busy
        // index-based: each node reads its ring neighbor (i+1)%N and writes two X slots
        #[allow(clippy::needless_range_loop)]
        for i in 0..DOMAIN_NODES {
            let w = f64::from(weights[i]) / 100.0;
            let neighbor = self.domain[(i + 1) % DOMAIN_NODES];
            self.full[i] = self.domain[i]; // direct inclusion
            self.full[i + DOMAIN_NODES] = w * self.domain[i] + (1.0 - w) * neighbor;
        }
        self.ready_sequence = Some(sequence);
    }

    pub fn swap(&mut self) {
        self.active = match self.active {
            BufferId::Domain => BufferId::Full,
            BufferId::Full => BufferId::Domain,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_includes_domain_in_lower_half() {
        let b = DoubleBuffer::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        assert_eq!(&b.full()[..DOMAIN_NODES], b.domain());
        assert_eq!(b.ready_sequence(), Some(0));
    }

    #[test]
    fn expand_weight_100_is_identity_on_upper_half() {
        // w=1.0 -> upper[i] = base[i]
        let b = DoubleBuffer::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        for i in 0..DOMAIN_NODES {
            assert_eq!(b.full()[i + DOMAIN_NODES], b.domain()[i]);
        }
    }

    #[test]
    fn expand_weight_zero_is_neighbor() {
        let mut b = DoubleBuffer::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        b.expand(1, [0; DOMAIN_NODES]); // w=0 -> upper[i] = neighbor
        for i in 0..DOMAIN_NODES {
            assert_eq!(b.full()[i + DOMAIN_NODES], b.domain()[(i + 1) % DOMAIN_NODES]);
        }
    }

    #[test]
    fn swap_toggles_active() {
        let mut b = DoubleBuffer::new([0.0; DOMAIN_NODES]);
        assert_eq!(b.active(), BufferId::Domain);
        b.swap();
        assert_eq!(b.active(), BufferId::Full);
    }
}
