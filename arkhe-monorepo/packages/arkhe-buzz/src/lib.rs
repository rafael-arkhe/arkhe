//! ARKHE / SUBSTRATE FISSION THEORY (SFT) — Buzz Bridge (honest simulation core)
//!
//! Rust counterpart of the `arkhe-smeasure` (Lean) and `arkhe-sft` (Lean)
//! formalizations. Everything is implemented with **zero external dependencies**
//! so the crate builds fully offline:
//!
//!   * `SFTParticle`      — partícula mínima (m, q) com r_spin, λ, ν_C, r_S,
//!                          área de captura C_A e área de Planck A_P;
//!   * `HolographicLedger` — ledger holográfico *append-only* (no-overwrite):
//!                          escrever um modo nunca apaga modos gravados; cada
//!                          entrada tem um CID simulado (hash FNV-1a local);
//!   * `SFTBuzzAgent`      — agente Buzz que publica kinds 30002/30003 apenas
//!                          quando a S-Measure supera o limiar adaptativo;
//!   * `EscapeRisk`        — os 5 níveis de risco espelho do `checkEscape` Lean;
//!   * `adaptive_threshold`— F(n) = c/n, análogo determinístico do SAC/EXP3;
//!   * `safe_execute`      — execução à prova de pânico com rollback do ledger.
//!
//! v1.0: offline, `#![deny(warnings)]`-compatible, sem `unsafe`.

#![forbid(unsafe_code)]

/// Constante de Planck reduzida (J·s).
pub const HBAR: f64 = 1.054_571_817e-34;
/// Velocidade da luz no vácuo (m/s).
pub const C_LIGHT: f64 = 2.997_924_58e8;
/// Constante gravitacional (m³·kg⁻¹·s⁻²).
pub const G_NEWTON: f64 = 6.674_30e-11;
/// Comprimento de Planck (m).
pub const PLANCK_LENGTH: f64 = 1.616_255e-35;

/// Número de modos de vibração da SFT: exatamente nove.
pub const MODE_COUNT: usize = 9;

// ============================================================
// §1  Partícula mínima
// ============================================================

/// Partícula da SFT: excitação mínima com massa e carga.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SFTParticle {
    pub mass: f64,
    pub charge: f64,
}

impl SFTParticle {
    /// Cria uma partícula com a massa dada.
    pub fn new(mass: f64, charge: f64) -> Self {
        Self { mass, charge }
    }

    /// Raio de spin: r_spin = hbar / (2·m·c).
    pub fn r_spin(&self) -> f64 {
        HBAR / (2.0 * self.mass * C_LIGHT)
    }

    /// Comprimento de onda associado: λ = 4π·r_spin.
    pub fn wavelength(&self) -> f64 {
        4.0 * std::f64::consts::PI * self.r_spin()
    }

    /// Frequência de Compton: ν = m·c² / hbar.
    pub fn compton_frequency(&self) -> f64 {
        self.mass * C_LIGHT.powi(2) / HBAR
    }

    /// Raio de Schwarzschild: r_S = 2·g·m / c².
    pub fn schwarzschild_radius(&self) -> f64 {
        2.0 * G_NEWTON * self.mass / C_LIGHT.powi(2)
    }

    /// Área de captura: C_A = π·r_spin².
    pub fn capture_area(&self) -> f64 {
        std::f64::consts::PI * self.r_spin().powi(2)
    }

    /// Área de Planck: A_P = l_P².
    pub fn planck_area(&self) -> f64 {
        PLANCK_LENGTH.powi(2)
    }

    /// Critério da SFT: se 4·g·m² < hbar·c, a partícula não colapsa em buraco
    /// negro (r_S < r_spin). Espelho do teorema Lean `q_bh_excluded`.
    pub fn is_black_hole_excluded(&self) -> bool {
        4.0 * G_NEWTON * self.mass.powi(2) < HBAR * C_LIGHT
    }

    /// Nove modos de vibração da partícula: escada harmónica da frequência de
    /// Compton, f_j = (j+1)·ν_C (modos distintos entre si).
    pub fn modes(&self) -> Vec<f64> {
        let nu = self.compton_frequency();
        (0..MODE_COUNT).map(|j| (j + 1) as f64 * nu).collect()
    }
}

// ============================================================
// §2  Hash local (FNV-1a 64-bit) e CID simulado
// ============================================================

/// Hash FNV-1a de 64 bits (sem dependências externas).
pub fn hash64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// CID simulado (formato `Qm...`, 32 hex) a partir do conteúdo.
pub fn fake_cid(data: &[u8]) -> String {
    let h1 = hash64(data);
    let h2 = hash64(&h1.to_le_bytes());
    format!("Qm{h1:016x}{h2:016x}")
}

// ============================================================
// §3  Ledger holográfico (no-overwrite / append-only)
// ============================================================

/// Entrada imutável do ledger holográfico.
#[derive(Debug, Clone, PartialEq)]
pub struct LedgerEntry {
    pub cid: String,
    pub kind: u64,
    pub content: Vec<u8>,
    pub seq: u64,
}

/// Ledger holográfico *append-only*: a escrita nunca apaga modos gravados.
#[derive(Debug, Default)]
pub struct HolographicLedger {
    entries: Vec<LedgerEntry>,
}

impl HolographicLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Escreve uma entrada e devolve o CID (a escrita é monotónica).
    pub fn write(&mut self, kind: u64, content: &[u8]) -> String {
        let cid = fake_cid(content);
        let seq = self.entries.len() as u64;
        self.entries.push(LedgerEntry {
            cid: cid.clone(),
            kind,
            content: content.to_vec(),
            seq,
        });
        cid
    }

    /// Lê uma entrada pelo CID (memória holográfica).
    pub fn get(&self, cid: &str) -> Option<&LedgerEntry> {
        self.entries.iter().find(|e| e.cid == cid)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Número de entradas de um dado kind (p. ex. 30002/30003).
    pub fn kind_count(&self, kind: u64) -> usize {
        self.entries.iter().filter(|e| e.kind == kind).count()
    }

    /// Snapshot para rollback.
    pub fn snapshot(&self) -> Vec<LedgerEntry> {
        self.entries.clone()
    }

    /// Restaura um snapshot (usado por `safe_execute`).
    pub fn restore(&mut self, snap: Vec<LedgerEntry>) {
        self.entries = snap;
    }
}

// ============================================================
// §4  Limiar adaptativo e risco de escape
// ============================================================

/// Limiar adaptativo F(n) = c/n — análogo determinístico do SAC/EXP3
/// (espelho do `fThreshold` Lean).
pub fn adaptive_threshold(n: usize, c: f64) -> f64 {
    if n == 0 {
        return 0.0;
    }
    c / n as f64
}

/// Os 5 níveis de risco de fuga da sandbox (espelho do `EscapeRisk` Lean).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeRisk {
    None,
    Low,
    Medium,
    High,
    Escaped,
}

impl EscapeRisk {
    /// Classifica o risco a partir da S-Measure, do limiar e da tendência.
    pub fn check(sm: f64, th: f64, tr: f64) -> EscapeRisk {
        let ratio = sm / th;
        if ratio >= 1.0 && tr > 0.0 {
            EscapeRisk::Escaped
        } else if ratio >= 1.0 {
            EscapeRisk::High
        } else if ratio >= 0.85 {
            EscapeRisk::Medium
        } else if ratio >= 0.7 {
            EscapeRisk::Low
        } else {
            EscapeRisk::None
        }
    }
}

// ============================================================
// §5  Agente Buzz + execução segura
// ============================================================

/// Evento Buzz da SFT (kinds 30002/30003 do protocolo).
#[derive(Debug, Clone)]
pub struct BuzzEvent {
    pub kind: u64,
    pub pubkey: [u8; 32],
    pub content: Vec<u8>,
    pub s_measure: f64,
    pub trend: f64,
}

/// Agente Buzz que publica apenas eventos com S-Measure acima do limiar.
#[derive(Debug)]
pub struct SFTBuzzAgent {
    ledger: HolographicLedger,
    threshold: f64,
}

impl Default for SFTBuzzAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl SFTBuzzAgent {
    pub fn new() -> Self {
        Self {
            ledger: HolographicLedger::new(),
            threshold: 0.7,
        }
    }

    pub fn threshold(&self) -> f64 {
        self.threshold
    }

    pub fn set_threshold(&mut self, t: f64) {
        self.threshold = t;
    }

    pub fn ledger(&self) -> &HolographicLedger {
        &self.ledger
    }

    /// Publica o evento apenas se a S-Measure superar o limiar. Devolve o CID
    /// em caso de sucesso, ou `None` se a barreira ΔS < 0 bloquear a escrita.
    pub fn publish(&mut self, ev: BuzzEvent) -> Option<String> {
        if ev.s_measure < self.threshold {
            return None;
        }
        let mut data = ev.pubkey.to_vec();
        data.extend_from_slice(&ev.content);
        Some(self.ledger.write(ev.kind, &data))
    }
}

/// Executa uma tarefa à prova de pânico: o fecho recebe o ledger e grava o que
/// for preciso; se entrar em pânico, o ledger é reposto ao snapshot anterior
/// (nenhuma escrita parcial fica gravada).
pub fn safe_execute<F>(ledger: &mut HolographicLedger, f: F) -> Result<Vec<u8>, String>
where
    F: FnOnce(&mut HolographicLedger) -> Vec<u8>,
{
    let snap = ledger.snapshot();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(ledger))) {
        Ok(out) => Ok(out),
        Err(_) => {
            ledger.restore(snap);
            Err("panic: execução revertida (no-overwrite)".to_string())
        }
    }
}

// ============================================================
// §6  Testes
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_spin_is_positive_and_finite() {
        let e = SFTParticle::new(9.109_383_56e-31, -1.0);
        assert!(e.r_spin() > 0.0);
        assert!(e.r_spin().is_finite());
        assert!(e.wavelength() > 0.0);
        assert!(e.compton_frequency() > 0.0);
        assert!(e.capture_area() > 0.0);
        assert!(e.planck_area() > 0.0);
    }

    #[test]
    fn mass_hierarchy_encolhe_lambda_sobe_nu() {
        let light = SFTParticle::new(1.0e-30, 0.0);
        let heavy = SFTParticle::new(2.0e-30, 0.0);
        assert!(heavy.wavelength() < light.wavelength());
        assert!(light.compton_frequency() < heavy.compton_frequency());
    }

    #[test]
    fn bh_exclusion_holds_for_electron_mass() {
        let e = SFTParticle::new(9.109_383_56e-31, -1.0);
        assert!(e.is_black_hole_excluded());
        assert!(e.schwarzschild_radius() < e.r_spin());
    }

    #[test]
    fn exactly_nine_modes() {
        let e = SFTParticle::new(9.109_383_56e-31, 0.0);
        assert_eq!(e.modes().len(), 9);
        assert_eq!(MODE_COUNT, 9);
    }

    #[test]
    fn ledger_is_append_only_no_overwrite() {
        let mut l = HolographicLedger::new();
        let c1 = l.write(30002, b"alice");
        let c2 = l.write(30003, b"bob");
        assert_ne!(c1, c2);
        assert_eq!(l.len(), 2);
        assert!(l.get(&c1).is_some());
        assert!(l.get(&c2).is_some());
        assert_eq!(l.get(&c1).unwrap().content, b"alice");
        assert_eq!(l.kind_count(30002), 1);
        assert_eq!(l.kind_count(30003), 1);
    }

    #[test]
    fn safe_execute_rolls_back_on_panic() {
        let mut l = HolographicLedger::new();
        l.write(30002, b"pre-existing");
        let before = l.len();
        let res = safe_execute(&mut l, |lg| {
            lg.write(30003, b"partial");
            panic!("boom");
        });
        assert!(res.is_err());
        assert_eq!(l.len(), before, "escrita parcial deve ser revertida");
        assert_eq!(l.get("Qm00000000000000000000000000000000"), None);
    }

    #[test]
    fn safe_execute_keeps_successful_writes() {
        let mut l = HolographicLedger::new();
        let before = l.len();
        let res = safe_execute(&mut l, |lg| {
            lg.write(30002, b"data");
            b"ok".to_vec()
        });
        assert_eq!(res.unwrap(), b"ok");
        assert_eq!(l.len(), before + 1);
    }

    #[test]
    fn adaptive_threshold_is_c_over_n() {
        assert_eq!(adaptive_threshold(5, 1.0), 0.2);
        assert_eq!(adaptive_threshold(9, 1.0), 1.0 / 9.0);
        assert_eq!(adaptive_threshold(0, 1.0), 0.0);
    }

    #[test]
    fn escape_risk_levels_match_lean() {
        assert_eq!(EscapeRisk::check(1.2, 1.0, 0.4), EscapeRisk::Escaped);
        assert_eq!(EscapeRisk::check(1.0, 1.0, 0.0), EscapeRisk::High);
        assert_eq!(EscapeRisk::check(0.9, 1.0, 0.1), EscapeRisk::Medium);
        assert_eq!(EscapeRisk::check(0.75, 1.0, 0.1), EscapeRisk::Low);
        assert_eq!(EscapeRisk::check(0.5, 1.0, -0.1), EscapeRisk::None);
    }

    #[test]
    fn agent_blocks_below_threshold() {
        let mut a = SFTBuzzAgent::new();
        a.set_threshold(0.7);
        let ev_low = BuzzEvent {
            kind: 30002,
            pubkey: [1u8; 32],
            content: b"low".to_vec(),
            s_measure: 0.5,
            trend: -0.1,
        };
        let ev_ok = BuzzEvent {
            kind: 30003,
            pubkey: [2u8; 32],
            content: b"high".to_vec(),
            s_measure: 0.9,
            trend: 0.1,
        };
        assert_eq!(a.publish(ev_low), None);
        let cid = a.publish(ev_ok).expect("deve publicar acima do limiar");
        assert!(a.ledger().get(&cid).is_some());
    }

    #[test]
    fn hash_is_deterministic() {
        assert_eq!(hash64(b"arkhe"), hash64(b"arkhe"));
        assert_ne!(hash64(b"arkhe"), hash64(b"arkh"));
        assert!(fake_cid(b"x").starts_with("Qm"));
    }
}
