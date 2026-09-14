//! Post-quantum security levels for hash functions.

/// The two security notions of a hash function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashSecurity {
    /// Preimage resistance. Grover gives a quadratic speedup, so a
    /// classical n-bit output provides n/2 bits of PQ security.
    /// See draft-vauban §3.2.2 (256-bit hash → 128-bit PQ preimage).
    Preimage,
    /// Collision resistance. Brassard–Høyer–Tapp (1997) gives a
    /// cubic-root speedup, so n bits provide n/3 bits of PQ security.
    ///
    /// Note: BHT requires exponential qRAM, which is not currently
    /// available. The n/3 figure is a theoretical upper bound on the
    /// adversary's advantage; it is not an operational parameter.
    Collision,
}

impl HashSecurity {
    /// PQ security bits for a classical output size of `n` bits.
    pub fn pq_bits(self, n: u32) -> f64 {
        match self {
            HashSecurity::Preimage => n as f64 / 2.0,
            HashSecurity::Collision => n as f64 / 3.0,
        }
    }
}