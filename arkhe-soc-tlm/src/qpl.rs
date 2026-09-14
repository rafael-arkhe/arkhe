//! QPL accelerator model: a ring convolution `out[i] = (left+center+right)/3`
//! over the 8-node domain, plus performance counters. This is the arithmetic
//! the RTL `qpl_accel.sv` must reproduce bit-for-bit (in fixed point).

use crate::payload::DOMAIN_NODES;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QplResult {
    pub node: usize,
    pub input: f64,
    pub output: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize)]
pub struct PerfCounters {
    pub qpl_ops: u64,
    pub frames_emitted: u64,
    pub frames_verified: u64,
    pub frames_rejected: u64,
}

/// One QPL hop over the ring domain.
pub fn qpl_forward(domain: &[f64; DOMAIN_NODES]) -> [QplResult; DOMAIN_NODES] {
    let mut out = [QplResult { node: 0, input: 0.0, output: 0.0 }; DOMAIN_NODES];
    for i in 0..DOMAIN_NODES {
        let left = domain[(i + DOMAIN_NODES - 1) % DOMAIN_NODES];
        let right = domain[(i + 1) % DOMAIN_NODES];
        out[i] = QplResult {
            node: i,
            input: domain[i],
            output: (left + domain[i] + right) / 3.0,
        };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_field_is_a_fixed_point() {
        // average of three equal values is that value
        let d = [3.0; DOMAIN_NODES];
        for r in qpl_forward(&d) {
            assert!((r.output - 3.0).abs() < 1e-12);
        }
    }

    #[test]
    fn deterministic() {
        let d = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        assert_eq!(qpl_forward(&d), qpl_forward(&d));
    }

    #[test]
    fn ring_wraps_at_edges() {
        let d = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let out = qpl_forward(&d);
        // node 0: (d[7] + d[0] + d[1]) / 3 = (8+1+2)/3
        assert!((out[0].output - (8.0 + 1.0 + 2.0) / 3.0).abs() < 1e-12);
    }
}
