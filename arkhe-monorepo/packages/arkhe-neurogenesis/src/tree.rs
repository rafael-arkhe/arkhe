use crate::hamiltonian::Lcg;

/// A dendrite growth event: site `parent` sprouted a child node at `pos`
/// during simulation step `step`.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct GrowEvent {
    pub step: usize,
    pub parent: usize,
    pub child: usize,
    pub pos: (f64, f64),
}

/// A growing neuron: a set of 2-D nodes plus the dendrite branches and the
/// growth events that created them.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Neuron {
    pub nodes: Vec<(f64, f64)>,
    pub branches: Vec<(usize, usize)>,
    pub events: Vec<GrowEvent>,
    #[serde(skip)]
    clock: usize,
    #[serde(skip)]
    rng: Lcg,
}

impl Neuron {
    /// Create a neuron rooted at `origin`. Growth directions are drawn
    /// deterministically from `seed`.
    pub fn seed(origin: (f64, f64), seed: u64) -> Self {
        Self {
            nodes: vec![origin],
            branches: Vec::new(),
            events: Vec::new(),
            clock: 0,
            rng: Lcg::new(seed ^ 0xA7A7),
        }
    }

    /// Append a lattice node and return its index.
    pub fn add_node(&mut self, pos: (f64, f64)) -> usize {
        self.nodes.push(pos);
        self.nodes.len() - 1
    }

    /// Advance the internal step clock (called once per simulation step).
    pub fn tick(&mut self) {
        self.clock += 1;
    }

    /// Grow a dendrite branch off the given node.
    ///
    /// A child node is placed at a deterministic angle/length, the branch
    /// `(parent, child)` is recorded, and a [`GrowEvent`] is emitted. Returns
    /// the new node index, or `None` if `node` is out of range.
    pub fn grow(&mut self, node: usize) -> Option<usize> {
        if node >= self.nodes.len() {
            return None;
        }
        let (px, py) = self.nodes[node];
        let theta = self.rng.unit() * std::f64::consts::TAU;
        let length = 0.5 + 0.5 * self.rng.unit();
        let pos = (px + length * theta.cos(), py + length * theta.sin());
        let child = self.add_node(pos);
        self.branches.push((node, child));
        self.events.push(GrowEvent {
            step: self.clock,
            parent: node,
            child,
            pos,
        });
        Some(child)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grow_appends_node_branch_and_event() {
        let mut n = Neuron::seed((0.0, 0.0), 3);
        n.add_node((1.0, 0.0));
        n.add_node((2.0, 0.0));
        n.tick();
        let child = n.grow(1).expect("node 1 must exist");
        assert_eq!(n.nodes.len(), 4);
        assert_eq!(n.branches, vec![(1, child)]);
        assert_eq!(n.events.len(), 1);
        assert_eq!(n.events[0].step, 1);
        assert_eq!(n.events[0].parent, 1);
        assert_eq!(n.events[0].child, child);
    }

    #[test]
    fn grow_rejects_out_of_range_parent() {
        let mut n = Neuron::seed((0.0, 0.0), 3);
        assert!(n.grow(5).is_none());
        assert!(n.branches.is_empty());
    }

    #[test]
    fn growth_is_deterministic() {
        let mut a = Neuron::seed((0.0, 0.0), 9);
        let mut b = Neuron::seed((0.0, 0.0), 9);
        a.add_node((1.0, 0.0));
        b.add_node((1.0, 0.0));
        let ca = a.grow(0).unwrap();
        let cb = b.grow(0).unwrap();
        assert_eq!(a.nodes, b.nodes);
        assert_eq!(ca, cb);
    }
}
