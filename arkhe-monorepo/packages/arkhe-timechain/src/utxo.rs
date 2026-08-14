//! A Bitcoin-style UTXO ledger mapped onto field topology.
//!
//! Each unspent output is a **flux tube**: it carries an amount and a flux
//! `Φ` (a `ℕ` from the phase field). Spending a UTXO *is* the reconnection of
//! the tubes it references — the inputs merge and split into new tubes. The
//! transaction is only valid when **flux is conserved** across the reconnection
//! (matching helicity/magnetic flux conservation in the MHD layer).

use serde::{Deserialize, Serialize};

/// Failure modes of a flux-tube spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UtxoError {
    /// An input index does not exist.
    UnknownInput,
    /// An input was already spent (double-spend via reconnection).
    AlreadySpent,
    /// Sum of input flux differs from the sum of output flux.
    FluxMismatch,
}

impl std::fmt::Display for UtxoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A single unspent flux tube.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Utxo {
    /// Transaction id of the producing spend.
    pub txid: [u8; 32],
    /// Output position within that transaction.
    pub out_index: u32,
    /// Block height it was created.
    pub height: u64,
    /// The ledger amount.
    pub amount: f64,
    /// The topological flux Φ carried by the tube.
    pub flux: f64,
    /// Whether the tube has been reconnected (spent).
    pub spent: bool,
}

impl Utxo {
    fn is_spendable(&self) -> bool {
        !self.spent && self.amount.is_finite() && self.flux.is_finite()
    }
}

/// The canonical flux-tube UTXO ledger.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FluxLedger {
    /// All known outputs, oldest first.
    pub utxos: Vec<Utxo>,
}

impl FluxLedger {
    /// Create it empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of currently-unspent outputs.
    pub fn unspent_count(&self) -> usize {
        self.utxos.iter().filter(|u| !u.spent).count()
    }

    /// Create a brand-new output (e.g. a genesis/coinbase flux tube).
    pub fn mint(&mut self, txid: [u8; 32], out_index: u32, amount: f64, flux: f64, height: u64) {
        self.utxos.push(Utxo {
            txid,
            out_index,
            height,
            amount,
            flux,
            spent: false,
        });
    }

    /// Spend a set of input tubes into new outputs, verifying flux conservation.
    ///
    /// `inputs` are indices into [`Self::utxos`]. `outputs` is `(amount, flux)`
    /// per new tube. The sum of *input flux* must agree with the sum of *output
    /// flux* within `tolerance` — the reconnection conserves `Φ`.
    ///
    /// Returns the new transaction id (`txid`).
    pub fn spend(
        &mut self,
        inputs: &[usize],
        outputs: &[(f64, f64)],
        height: u64,
        tolerance: f64,
    ) -> Result<[u8; 32], UtxoError> {
        let mut in_flux = 0.0;
        let mut in_total = 0.0;
        for &idx in inputs {
            let u = self.utxos.get(idx).ok_or(UtxoError::UnknownInput)?;
            if !u.is_spendable() {
                return Err(UtxoError::AlreadySpent);
            }
            in_flux += u.flux;
            in_total += u.amount;
        }

        let out_flux: f64 = outputs.iter().map(|(_, f)| f).sum();
        let out_total: f64 = outputs.iter().map(|(a, _)| a).sum();

        if outputs.is_empty() || (in_flux - out_flux).abs() > tolerance.max(1e-12) {
            return Err(UtxoError::FluxMismatch);
        }
        if (in_total - out_total).abs() > tolerance.max(1e-12) {
            return Err(UtxoError::FluxMismatch);
        }

        // Clone the inputs' txids to form a deterministic new txid.
        let metasci = bincode::serialize(&in_flux).unwrap_or_default();
        let mut bs = metasci;
        for &idx in inputs {
            let u = &self.utxos[idx];
            bs.extend_from_slice(&u.txid);
            bs.push(u.out_index as u8);
        }
        bs.push(height as u8);
        let txid = crypto_hash(&bs);

        // Mark inputs spent.
        for &idx in inputs {
            self.utxos[idx].spent = true;
        }
        // Create outputs.
        for (i, &(amount, flux)) in outputs.iter().enumerate() {
            self.utxos.push(Utxo {
                txid,
                out_index: i as u32,
                height,
                amount,
                flux,
                spent: false,
            });
        }
        Ok(txid)
    }
}

fn crypto_hash(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spend_reconnects_conserving_flux() {
        let mut ledger = FluxLedger::new();
        let coinbase = [1u8; 32];
        ledger.mint(coinbase, 0, 100.0, 0.33, 0);
        ledger.mint(coinbase, 1, 50.0, 0.15, 0);

        let txid = ledger.spend(&[0, 1], &[(90.0, 0.30), (60.0, 0.18)], 2, 1e-9).unwrap();
        assert_eq!(txid.len(), 32);
        // Two inputs (idx 0,1) spent; two new outputs added.
        assert!(ledger.utxos[0].spent && ledger.utxos[1].spent);
        assert_eq!(ledger.unspent_count(), 2);
    }

    #[test]
    fn double_spend_rejected() {
        let mut ledger = FluxLedger::new();
        ledger.mint([2u8; 32], 0, 10.0, 0.1, 0);
        ledger.spend(&[0], &[(10.0, 0.1)], 1, 1e-9).unwrap();
        let err = ledger.spend(&[0], &[(10.0, 0.1)], 2, 1e-9);
        assert!(matches!(err, Err(UtxoError::AlreadySpent)));
    }

    #[test]
    fn flux_mismatch_rejected() {
        let mut ledger = FluxLedger::new();
        ledger.mint([3u8; 32], 0, 10.0, 0.1, 0);
        let err = ledger.spend(&[0], &[(10.0, 0.4)], 1, 1e-9);
        // 0.1 in -> 0.4 out => mismatch.
        assert!(matches!(err, Err(UtxoError::FluxMismatch)));
    }
}