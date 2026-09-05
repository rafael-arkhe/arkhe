//! Catedral OS v152.0 — materialização física (I157–I201).
//!
//! Camadas implementadas neste crate:
//!
//! - **Estrutura Espectral** (I157–I160): coerência irreduzível, limiar de
//!   Gödel, anti‑flatness e cascas diádicas.
//! - **Entropia Construtiva** (I167): balanço entre espectro, anti‑flatness,
//!   limiar de Gödel e custo de handovers.
//! - **Undulator** (I194–I198): decaimento adaptativo com Zeno Veto e ranging.
//! - **Bridge Z1T** (G1/G8/G11): UART/USB com framing CRC16 e fallback para
//!   simulação.
//! - **Sincronização IPFS** (I201): ledger distribuído com validação cruzada
//!   do Zeno Veto entre nós.
//! - **Verificação Lean embarcada** (I200): checagem de invariantes em tempo
//!   real.

pub mod core;
pub mod hardware;
pub mod network;
pub mod verification;

pub use core::entropy::constructive_entropy;
pub use core::spectral::{
    compute_anti_flatness, diadic_shell, godel_threshold, handover_level, irreducible_coherence,
    is_godel_threshold_reached, organize_in_shells,
};
pub use hardware::{
    framer::{crc16_xmodem, Z1TFramer},
    undulator::{DecayRate, RangingDelta, UndulatorHandover, UndulatorNode, UndulatorStats},
};
pub use verification::lean_runtime::{InvariantStatus, LeanVerifier, VerificationReport};