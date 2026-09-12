#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
//! # ARKHE PoTT — Proof-of-Transit Timestamping (substrate `924-POTT-INTERPLANETARY-TRANSPORT`)
//!
//! Rust implementation of the receipt layer formalised in
//! *"Bitcoin as an Interplanetary Monetary Standard with
//! Proof-of-Transit Timestamping"* (Puente & Puente, arXiv:2508.20591).
//!
//! PoTT produces a **tamper-evident, hop-timed chain of custody** bound to the
//! hash of a Bitcoin payload transported over a Delay/Disruption-Tolerant
//! Networking (DTN) path. Every relay appends a signed receipt; the receipts
//! form a splice-resistant hash chain (each hop hashes the previous receipt
//! sans signature) that survives audits, Lightning/watchtower disputes and
//! sidechain peg evidence packages.
//!
//! PoTT is strictly *out-of-band*: it does not modify Bitcoin consensus, BOLT
//! wire formats or the DTN bundle core. Like the reference paper, it supplies
//! accountability evidence only.
//!
//! ## Modules
//!
//! * [`time`] — 64-bit TAI seconds (CCSDS CUC, epoch 1958-01-01) and the
//!   TAI↔UTC/leap-second conversions needed to anchor "arrived-before-expiry"
//!   to Bitcoin Median-Time-Past (BIP-113).
//! * [`wire`] — minimal canonical CBOR codec for the PoTT receipt map
//!   (Appendix A: keys `0..=6`), including the normative test vector.
//! * [`receipt`] — [`Receipt`], chain construction, BIP-340 signing and the
//!   Bitcoin-native payload digests (double-SHA-256; BIP157/158 filters).
//! * [`verify`] — chain verification, the [`PoTT-M2`](verify::M2Policy)
//!   assurance profile, transcript commit/reveal and MTP anchoring.
//! * [`lightning`] — latency-aware CLTV/CSV margins
//!   (`∆extra_CLTV = ⌈(RTT+J)/btarget⌉`) and header/filter link budgets.
//!
//! ## Conventions
//!
//! Values crossing the wire are *untrusted* until [`verify::verify_chain`]
//! accepts them under an authorized relay allowlist (529-RUST-VALIDATE-KERNEL-API
//! pattern). Relay identities are BIP-340 x-only secp256k1 public keys.
//! All invariants are checked, never assumed.

extern crate alloc;

pub mod lightning;
pub mod receipt;
pub mod time;
pub mod verify;
pub mod wire;

// Core evidence types.
pub use receipt::{Chain, ChainError, PayloadKind, Receipt};
pub use wire::{H32, NodeId, Nonce16, TAI};

// Verification entry points.
pub use verify::{verify_chain, VerifyOptions, Domain, Profile, AuthorizedRelay};

/// Substrate identifier registered in the ARKHE monorepo.
pub const SUBSTRATE: &str = "924-POTT-INTERPLANETARY-TRANSPORT";
/// Version stamp.
pub const VERSION: &str = "0.1.0-ARKHE-POTT-2026-09-06";
/// Reference paper (arXiv).
pub const PAPER_ARXIV: &str = "https://arxiv.org/abs/2508.20591";