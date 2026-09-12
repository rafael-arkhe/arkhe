//! ARKHE-VSEPR Núcleo 3 — Canal Tzinor (Quantum Switch ICO, Camada de comunicação).
//!
//! Dois dos Sete Selos de agosto de 2026 definem a física do canal:
//!
//! - **Selo #1 — arXiv:2608.13997** "Limits of independent and identical
//!   measurements for quantum illumination with an unknown return phase"
//!   (K. Shiraiwa, S. Kukita, 14 ago 2026): a iluminação quântica melhora o
//!   expoente de erro por ~6 dB (fator 4) **apenas com fase conhecida**. Com fase
//!   de retorno desconhecida, luz coerente incoerente com **detecção heteródina**
//!   já satura o limite do pior caso para todo receptor i.i.d. e todo estado de
//!   entrada — o emaranhamento **não confere vantagem** nesse cenário. Estratégia:
//!   heteródina em fase desconhecida; emaranhado onde a fase é estimável a priori.
//!
//! - **Selo #7 — arXiv:2608.14110** "Entanglement certification via causal-order
//!   interferometry in a quantum switch" (H. Wang, S. Liu, Q. He, 14 ago 2026):
//!   o **quantum switch** controla coerentemente as ordens de dois canais (ICO);
//!   sob ruído Pauli estocástico, a saída pós-selecionada exibe negatividade de
//!   emaranhamento **maior** que qualquer mistura clássica das duas ordens — há
//!   regimes onde a negatividade ICO é não nula e a de toda mistura clássica se
//!   anula — ampliados por uma unitária Pauli local.
//!
//! Invariantes: Runtime-2, Provenance-1, Gap-2 (entropia/ruído).

pub mod illumination;
pub mod switch;

pub use illumination::{
    ENTANGLEMENT_GAIN_DB_KNOWN_PHASE, HETERODYNE_WORSECASE, ILLUM_REF_2026, DetectionStrategy,
    IlluminationAssessment, choose_strategy, error_exponent_gain_db, heterodyne_saturates_bound,
    known_phase_factor,
};
pub use switch::{
    PPT_TILES_REF, SWITCH_REF_2026, ICORegion, NegativityReport, QuantumSwitch,
    ico_only_region_width, ico_region, local_pauli_amplification, postselected_negativity,
};