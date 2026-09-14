//! Offsets canônicos do wire format ML-DSA-65 (FIPS 204).
//!
//! Valores verificados:
//!   c̃  = λ/4 = 48 bytes
//!   z  = l × POLYZ_PACKEDBYTES = 5 × 640 = 3200 bytes
//!   h  = ω + k = 55 + 6 = 61 bytes
//!   Total = 3309 bytes
//!   Offset do hint = 48 + 3200 = 3248 bytes

pub mod ml_dsa_65_offsets {
    pub const CTILDE_BYTES: usize = 48;
    pub const Z_BYTES: usize = 5 * 640;
    pub const H_BYTES: usize = 55 + 6;
    pub const Z_OFFSET: usize = CTILDE_BYTES;
    pub const H_OFFSET: usize = CTILDE_BYTES + Z_BYTES;
    pub const SIGNATURE_BYTES: usize = CTILDE_BYTES + Z_BYTES + H_BYTES;
}