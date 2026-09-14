//! Integrator: wires SRAM D/X, QPL, the Smith AFE, and AOTB into one cycle:
//! sample I/Q → coupling → domain D → QPL hop → expand/swap → emit → verify.

use crate::aotb::AotbEncoder;
use crate::payload::{AotbPayload, DOMAIN_NODES};
use crate::qpl::{qpl_forward, PerfCounters, QplResult};
use crate::smith::smith_coupling_f64;
use crate::sram::DoubleBuffer;

pub struct ArkheSoc {
    pub memory: DoubleBuffer,
    pub weights: [u8; DOMAIN_NODES],
    pub counters: PerfCounters,
    session_id: [u8; 16],
    proof_hash: [u8; 32],
}

impl ArkheSoc {
    pub fn new(domain: [f64; DOMAIN_NODES], session_id: [u8; 16]) -> Self {
        let memory = DoubleBuffer::new(domain);
        let proof_hash = AotbPayload {
            session_id,
            sequence: 0,
            nonce: 0,
            proof_hash: [0; 32],
            domain_values: domain,
            weights: [100; DOMAIN_NODES],
        }
        .hash();
        Self { memory, weights: [100; DOMAIN_NODES], counters: PerfCounters::default(), session_id, proof_hash }
    }

    pub fn session_id(&self) -> [u8; 16] {
        self.session_id
    }
    pub fn proof_hash(&self) -> [u8; 32] {
        self.proof_hash
    }

    /// Load the fundamental domain from 8 AFE (I,Q) channels via the Smith map.
    pub fn load_from_afe(&mut self, iq: &[(i64, i64); DOMAIN_NODES]) {
        for (i, &(qi, qq)) in iq.iter().enumerate() {
            self.memory.write_domain(i, smith_coupling_f64(qi, qq));
        }
    }

    pub fn qpl_hop(&mut self) -> [QplResult; DOMAIN_NODES] {
        let r = qpl_forward(self.memory.domain());
        self.counters.qpl_ops += DOMAIN_NODES as u64;
        r
    }

    pub fn expand(&mut self, sequence: u64) {
        self.memory.expand(sequence, self.weights);
        self.memory.swap();
    }

    pub fn emit(&mut self, enc: &mut AotbEncoder) -> crate::aotb::AotbFrame {
        let f = enc.next_frame(*self.memory.domain(), self.weights);
        self.counters.frames_emitted += 1;
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aotb::AotbVerifier;
    use crate::smith::ONE_IN;
    use ed25519_dalek::SigningKey;

    #[test]
    fn end_to_end_afe_to_verified_frame() {
        let sk = SigningKey::from_bytes(&[9u8; 32]);
        let vk = sk.verifying_key();
        let mut soc = ArkheSoc::new([0.0; DOMAIN_NODES], [3; 16]);
        let mut enc = AotbEncoder::new(sk, soc.session_id(), soc.proof_hash());
        let mut ver = AotbVerifier::new(vk, soc.session_id());

        // matched loads on all channels -> coupling ~1.0
        let iq = [(0i64, 0i64); DOMAIN_NODES];
        soc.load_from_afe(&iq);
        for &c in soc.memory.domain() {
            assert!((c - 1.0).abs() < 1e-3);
        }

        for seq in 0..8u64 {
            let _ = soc.qpl_hop();
            soc.expand(seq);
            let f = soc.emit(&mut enc);
            assert_eq!(ver.verify(&f), Ok(()));
            soc.counters.frames_verified += 1;
        }
        assert_eq!(soc.counters.frames_emitted, 8);
        assert_eq!(soc.counters.frames_verified, 8);
    }

    #[test]
    fn afe_half_reflection_gives_three_quarter_coupling() {
        let mut soc = ArkheSoc::new([0.0; DOMAIN_NODES], [3; 16]);
        let iq = [(ONE_IN / 2, 0i64); DOMAIN_NODES]; // |Γ|=0.5
        soc.load_from_afe(&iq);
        for &c in soc.memory.domain() {
            assert!((c - 0.75).abs() < 0.01);
        }
    }
}
