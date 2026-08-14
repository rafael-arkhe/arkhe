//! LT fountain encoding/decoding (AFT — ARKHE Fountain Transport).
//!
//! Real LT codes: robust-soliton degree distribution, XOR-combined blocks,
//! deterministic seed-based PRNG so the decoder can reproduce block selection,
//! and SHA3-256 integrity checksums (NOT CRC32).

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sha3::{Digest, Sha3_256};
use std::collections::HashSet;
use thiserror::Error;

/// Frame magic bytes.
pub const AFT_MAGIC: [u8; 4] = *b"ARTH";

/// Fixed header length: magic(4) + session(4) + seq(4) + k(2) + block_size(2)
/// + data_len(4) + degree(2) = 22 bytes.
pub const HEADER_LEN: usize = 22;
/// SHA3-256 digest length.
pub const CHECKSUM_LEN: usize = 32;

#[derive(Debug, Error)]
pub enum FountainError {
    #[error("data length {0} exceeds capacity {1}")]
    DataTooLarge(usize, usize),
    #[error("invalid frame: bad magic")]
    BadMagic,
    #[error("invalid frame: truncated")]
    Truncated,
    #[error("invalid frame: checksum mismatch")]
    ChecksumMismatch,
    #[error("invalid frame: k or block_size mismatch (expected k={} bs={})", .expected_k, .expected_bs)]
    ParameterMismatch { expected_k: usize, expected_bs: usize },
    #[error("invalid frame: degree {0} out of range")]
    InvalidDegree(usize),
}

/// LT encoder producing self-describing frames.
///
/// Frame layout:
/// ```text
/// [magic 4][session 4][seq 4][k 2][block_size 2][data_len 4][degree 2][payload block_size][checksum 32]
/// ```
///
/// Block selection is derived deterministically from `(session_id, seq_num)`,
/// so a decoder can reconstruct which blocks were XOR-combined into each frame.
pub struct FountainEncoder {
    k: usize,
    block_size: usize,
    session_id: u32,
    seq_num: u32,
    dist: Vec<f64>,
}

impl FountainEncoder {
    pub fn new(k: usize, block_size: usize, session_id: u32) -> Self {
        let dist = robust_soliton_cdf(k, 0.1, 0.05);
        Self { k, block_size, session_id, seq_num: 0, dist }
    }

    pub fn next_frame(&mut self, data: &[u8]) -> Result<Vec<u8>, FountainError> {
        if data.len() > self.k * self.block_size {
            return Err(FountainError::DataTooLarge(data.len(), self.k * self.block_size));
        }

        let seed: u64 = (u64::from(self.session_id) << 32) | u64::from(self.seq_num);
        let mut rng = StdRng::seed_from_u64(seed);
        let degree = sample_degree(&mut rng, &self.dist, self.k);
        let indices = frame_indices(seed, degree, self.k);

        let mut payload = vec![0u8; self.block_size];
        for &idx in &indices {
            let start = idx * self.block_size;
            for (off, p) in payload.iter_mut().enumerate() {
                let src = start + off;
                if src < data.len() {
                    *p ^= data[src];
                }
            }
        }

        let mut frame = Vec::with_capacity(HEADER_LEN + self.block_size + CHECKSUM_LEN);
        frame.extend_from_slice(&AFT_MAGIC);
        frame.extend_from_slice(&self.session_id.to_le_bytes());
        frame.extend_from_slice(&self.seq_num.to_le_bytes());
        frame.extend_from_slice(&(self.k as u16).to_le_bytes());
        frame.extend_from_slice(&(self.block_size as u16).to_le_bytes());
        frame.extend_from_slice(&(data.len() as u32).to_le_bytes());
        frame.extend_from_slice(&(degree as u16).to_le_bytes());
        frame.extend_from_slice(&payload);
        frame.extend_from_slice(&Sha3_256::digest(&payload));

        self.seq_num = self.seq_num.wrapping_add(1);
        Ok(frame)
    }
}

struct FrameRecord {
    indices: Vec<usize>,
    payload: Vec<u8>,
}

/// LT decoder using the standard peeling (belief-propagation) algorithm.
pub struct FountainDecoder {
    k: usize,
    block_size: usize,
    data_len: Option<usize>,
    session_id: u32,
    blocks: Vec<Option<Vec<u8>>>,
    graph: Vec<FrameRecord>,
    complete: bool,
}

impl FountainDecoder {
    pub fn new(k: usize, block_size: usize) -> Self {
        Self {
            k,
            block_size,
            data_len: None,
            session_id: 0,
            blocks: vec![None; k],
            graph: Vec::new(),
            complete: false,
        }
    }

    /// Ingest one frame. Returns `true` when all blocks are decoded.
    pub fn ingest_frame(&mut self, frame: &[u8]) -> Result<bool, FountainError> {
        if frame.len() < HEADER_LEN + CHECKSUM_LEN {
            return Err(FountainError::Truncated);
        }
        if frame[..4] != AFT_MAGIC {
            return Err(FountainError::BadMagic);
        }
        let session_id = u32::from_le_bytes(frame[4..8].try_into().unwrap());
        let seq_num = u32::from_le_bytes(frame[8..12].try_into().unwrap());
        let k = u16::from_le_bytes(frame[12..14].try_into().unwrap()) as usize;
        let block_size = u16::from_le_bytes(frame[14..16].try_into().unwrap()) as usize;
        let data_len = u32::from_le_bytes(frame[16..20].try_into().unwrap()) as usize;
        let degree = u16::from_le_bytes(frame[20..22].try_into().unwrap()) as usize;

        if k != self.k || block_size != self.block_size {
            return Err(FountainError::ParameterMismatch {
                expected_k: self.k,
                expected_bs: self.block_size,
            });
        }
        if degree == 0 || degree > self.k {
            return Err(FountainError::InvalidDegree(degree));
        }

        let payload = &frame[HEADER_LEN..HEADER_LEN + self.block_size];
        let checksum = &frame[HEADER_LEN + self.block_size..HEADER_LEN + self.block_size + CHECKSUM_LEN];
        if Sha3_256::digest(payload).as_slice() != checksum {
            return Err(FountainError::ChecksumMismatch);
        }

        self.session_id = session_id;
        self.data_len = Some(data_len);

        let seed: u64 = (u64::from(session_id) << 32) | u64::from(seq_num);
        let indices = frame_indices(seed, degree, self.k);

        self.graph.push(FrameRecord { indices, payload: payload.to_vec() });
        self.run_peeling();

        self.complete = self.blocks.iter().all(Option::is_some);
        Ok(self.complete)
    }

    fn run_peeling(&mut self) {
        loop {
            let mut resolved: Option<(usize, usize)> = None;
            for (i, rec) in self.graph.iter().enumerate() {
                let unknown: Vec<usize> = rec
                    .indices
                    .iter()
                    .copied()
                    .filter(|&idx| self.blocks[idx].is_none())
                    .collect();
                if unknown.len() == 1 {
                    resolved = Some((i, unknown[0]));
                    break;
                }
            }

            match resolved {
                Some((fi, block_idx)) => {
                    let mut value = self.graph[fi].payload.clone();
                    for &idx in &self.graph[fi].indices {
                        if idx == block_idx {
                            continue;
                        }
                        if let Some(bv) = &self.blocks[idx] {
                            for (v, b) in value.iter_mut().zip(bv.iter()) {
                                *v ^= b;
                            }
                        }
                    }
                    self.blocks[block_idx] = Some(value);
                    self.graph.swap_remove(fi);
                }
                None => break,
            }
        }
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn get_decoded(&self) -> Option<Vec<u8>> {
        if !self.complete {
            return None;
        }
        let mut out = Vec::with_capacity(self.k * self.block_size);
        for b in &self.blocks {
            out.extend_from_slice(b.as_ref()?);
        }
        out.truncate(self.data_len.unwrap_or(out.len()));
        Some(out)
    }
}

/// Build the cumulative robust-soliton distribution over degrees `1..=k`.
fn robust_soliton_cdf(k: usize, c: f64, delta: f64) -> Vec<f64> {
    let mut p = vec![0.0; k + 1];
    if k == 1 {
        p[1] = 1.0;
        return p;
    }
    let kf = k as f64;
    let r = c * (kf / delta).ln() * kf.sqrt();
    let mut z = 0.0;
    for (d, slot) in p.iter_mut().enumerate().skip(1).take(k) {
        let d = d + 1;
        let mut prob = if d == 1 { 1.0 / kf } else { 1.0 / (d as f64 * (d as f64 - 1.0)) };
        if (d as f64) <= kf / r {
            prob += if (d as f64) < kf / r {
                r / (d as f64 * kf)
            } else {
                (r / kf) * (r / delta).ln()
            };
        }
        *slot = prob;
        z += prob;
    }
    let mut acc = 0.0;
    for slot in p.iter_mut().skip(1).take(k) {
        acc += *slot / z;
        *slot = acc;
    }
    p
}

fn sample_degree(rng: &mut StdRng, cdf: &[f64], k: usize) -> usize {
    let u: f64 = rng.gen();
    for (d, slot) in cdf.iter().enumerate().skip(1).take(k) {
        if u <= *slot {
            return d;
        }
    }
    k
}

fn distinct_indices(rng: &mut StdRng, k: usize, degree: usize) -> Vec<usize> {
    let mut set = HashSet::with_capacity(degree);
    while set.len() < degree {
        set.insert(rng.gen_range(0..k));
    }
    set.into_iter().collect()
}

/// Deterministically select the `degree` blocks XOR-combined into a frame.
///
/// The index RNG is seeded from `(seed, degree)` so the encoder and decoder
/// reproduce identical block selections even though the encoder additionally
/// consumes RNG entropy when drawing the degree. `distinct_indices` uses
/// `gen_range`, which relies on the RNG's internal state, so giving it a
/// dedicated seed (independent of the degree-draw RNG) is what keeps the two
/// sides in sync.
fn frame_indices(seed: u64, degree: usize, k: usize) -> Vec<usize> {
    let index_seed = seed ^ (degree as u64).rotate_left(17);
    let mut rng = StdRng::seed_from_u64(index_seed);
    let mut indices = distinct_indices(&mut rng, k, degree);
    indices.sort_unstable();
    indices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_no_loss() {
        let data = b"ARKHE fountain transport test payload 0123456789".to_vec();
        let mut enc = FountainEncoder::new(16, 8, 0xCAFE_BABE);
        let mut dec = FountainDecoder::new(16, 8);
        for _ in 0..64 {
            let frame = enc.next_frame(&data).unwrap();
            dec.ingest_frame(&frame).unwrap();
            if dec.is_complete() {
                break;
            }
        }
        assert!(dec.is_complete());
        assert_eq!(dec.get_decoded().unwrap(), data);
    }

    #[test]
    fn roundtrip_with_30pct_loss() {
        let data: Vec<u8> = (0..256u32).map(|i| (i * 31) as u8).collect();
        let mut enc = FountainEncoder::new(32, 8, 0x0BAD_F00D);
        let mut dec = FountainDecoder::new(32, 8);
        let mut rng = StdRng::seed_from_u64(42);
        let mut dropped = 0usize;
        let mut sent = 0usize;
        while !dec.is_complete() && sent < 512 {
            let frame = enc.next_frame(&data).unwrap();
            sent += 1;
            if rng.gen::<f64>() < 0.30 {
                dropped += 1;
                continue;
            }
            dec.ingest_frame(&frame).unwrap();
        }
        assert!(dec.is_complete(), "failed after {sent} sent / {dropped} dropped");
        assert_eq!(dec.get_decoded().unwrap(), data);
    }

    #[test]
    fn rejects_bad_checksum() {
        let data = b"integrity".to_vec();
        let mut enc = FountainEncoder::new(8, 4, 0x1111_2222);
        let mut frame = enc.next_frame(&data).unwrap();
        let last = frame.len() - 1;
        frame[last] ^= 0xFF;
        let mut dec = FountainDecoder::new(8, 4);
        assert!(dec.ingest_frame(&frame).is_err());
    }
}
