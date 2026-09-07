//! Ledger de coerência encadeado e append-only (Fase 4, Opção A).
//!
//! O [`CoherenceLedger`] materializa a **cadeia de dados** do field-stability:
//! uma sequência imutável de [`CoherenceEntry`] — uma por janela de medição —
//! encadeadas criptograficamente por hash SHA3-256 da entrada anterior
//! (Loopseal-2) e com rejeição nativa de timestamps não-monotônicos
//! (Gravity-1).
//!
//! ## Relação com o Journal
//!
//! Os blocos `bloco_987..993` são o **Journal** (registro de decisões
//! arquiteturais). Este ledger é a **cadeia de dados** — as próprias métricas
//! (`Φ`, componentes, janelas). O Journal descreve a reforma; o ledger a
//! materializa.
//!
//! ## Invariantes
//!
//! * **Loopseal-2 — append-only:** uma entrada inserida nunca é alterada;
//!   violações são impossíveis porque não há método de escrita.
//! * **Gravity-1 — monotonia de tempo:** `push` rejeita timestamps
//!   `<= last_timestamp` com erro explícito, sem corromper a cadeia.
//! * **Loopseal-3 — audit trail:** `verify_integrity()` re-encadeia do início
//!   e aponta qualquer quebra de hash.
//!
//! ## Reconciliamento com o esboço da decisão
//!
//! A decisão arquitetural (`CATEDRAL-OS-DECISAO-LEDGER-2026-09-06`) nomeava o
//! terceiro componente `structural_similarity` (α). O crate real mede
//! `latency_score` (Λ) como componente com peso `w_Λ = 0.2` (ver
//! [`crate::coherence`]). Para que o encadeamento capture a métrica efetiva
//! que determina `Φ`, este ledger usa **`latency_score`**, o componente real
//! no funcional de coerência — mesmo precedente da errata Cauchy–Schwarz
//! (bloco 991): a decisão nomeia o mecanismo; a implementação vincula ao dado
//! constitucional.

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};

/// Testemunho da invariante Gravity-1 (monotonia de timestamps).
pub const GRAVITY_1: &str = "GRAVITY-1: timestamps devem ser monotonicos crescentes.";

/// Hash da gênese — primeira âncora da cadeia (nenhum entry aponta para ela).
pub const GENESIS: &str = "GENESIS";

/// Horizonte formal coberto pela modelagem TLA+ (`MaxWindows = 4` em
/// `ArkheCoherenceLedger.tla`, bloco 1000) e pelo núcleo Lean I517–I523
/// (D3). Cadeias com `len > MAX_HORIZON_WINDOWS` retornam
/// [`IntegrityStatus::BeyondHorizon`] — os dados ainda são verificados pela
/// varredura integral (Loopseal-3); a garantia formal (TLC exhaustivo/Lean)
/// cobre apenas até o horizonte.
pub const MAX_HORIZON_WINDOWS: usize = 4;

/// Resultado semântico de [`CoherenceLedger::verify_integrity`] (D3).
///
/// A hierarquia é estrita: `Broken` (dado realmente adulterado) prevalece
/// sobre `BeyondHorizon` (fora do horizonte formal, mas dados íntegros) que
/// prevalece sobre `Ok`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrityStatus {
    /// Cadeia íntegra e dentro do horizonte formal (`len <= MAX_HORIZON_WINDOWS`).
    Ok,
    /// Entradas cujo hash interno não corresponde à forma canônica re-derivada,
    /// ou cujo `previous_hash` não encadeia — conjugado com a varredura de
    /// conteúdo da **última** entrada (A1/D1).
    Broken {
        /// `window_id` das entradas com quebra de integridade.
        window_ids: Vec<u64>,
    },
    /// Cadeia com mais janelas que o horizonte formal (`len > MAX_HORIZON_WINDOWS`).
    /// **Não é erro:** os dados foram verificados e estão íntegros; apenas a
    /// garantia formal (TLC/Lean) não cobre todo o comprimento.
    BeyondHorizon {
        /// Comprimento real da cadeia.
        len: usize,
        /// Horizonte formal (`MAX_HORIZON_WINDOWS`).
        max_windows: usize,
    },
}

impl IntegrityStatus {
    /// `true` quando a integridade de dados está confirmada (`Ok` ou
    /// `BeyondHorizon` — neste último caso os dados são verificados, apenas o
    /// horizonte formal foi ultrapassado).
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok | Self::BeyondHorizon { .. })
    }
}

/// Entrada imutável da cadeia de dados — uma medição por janela.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoherenceEntry {
    /// Índice sequencial da janela de medição.
    pub window_id: u64,
    /// Marca de tempo (segundos Unix ou tick lógico); deve ser estritamente
    /// crescente — rejeitado se `<=` à anterior (Gravity-1).
    pub timestamp: u64,
    /// Coerência total do funcional `Φ(Ω,Σ,Λ;W)` (Gap-1).
    pub phi: f64,
    /// Componente `Ω` — estabilidade de campo (peso `w_Ω = 0.4`).
    pub stability: f64,
    /// Componente `Σ` — taxa de sucesso (peso `w_Σ = 0.4`).
    pub success_rate: f64,
    /// Componente `Λ` — score de latência (peso `w_Λ = 0.2`).
    pub latency_score: f64,
    /// Hash SHA3-256 (hex) da entrada anterior — elo do encadeamento.
    pub previous_hash: String,
    /// Hash SHA3-256 (hex) desta própria entrada — fingerprint da sua forma
    /// canônica no momento da construção. `verify_integrity()` re-deriva e
    /// compara (A1/D1): sem este campo, a última entrada não tem ninguém
    /// depois dela para re-derivar o hash, e a tautologia
    /// `compute_hash() != compute_hash()` jamais detectava adulteração.
    pub hash: String,
}

impl CoherenceEntry {
    /// Constrói uma entrada a partir dos campos e calcula o hash SHA3-256 da
    /// sua forma canônica (Loopseal-2/Ghost-1).
    pub fn from_parts(
        window_id: u64,
        timestamp: u64,
        phi: f64,
        stability: f64,
        success_rate: f64,
        latency_score: f64,
        previous_hash: impl Into<String>,
    ) -> Self {
        let mut entry = Self {
            window_id,
            timestamp,
            phi,
            stability,
            success_rate,
            latency_score,
            previous_hash: previous_hash.into(),
            hash: String::new(),
        };
        entry.hash = entry.compute_hash();
        entry
    }
    /// Serializa a entrada em forma canônica (comprimentos fixos, sem
    /// semântica ambígua) para o cálculo de hash.
    pub fn canonical(&self) -> String {
        format!(
            "{}-{}-{:.6}-{:.6}-{:.6}-{:.6}-{}",
            self.window_id,
            self.timestamp,
            self.phi,
            self.stability,
            self.success_rate,
            self.latency_score,
            self.previous_hash
        )
    }

    /// Hash SHA3-256 (hex) desta entrada a partir da forma canônica.
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha3_256::new();
        hasher.update(self.canonical().as_bytes());
        hex_encode(hasher.finalize())
    }

    /// Re-verifica que esta entrada encadeia corretamente com `previous`,
    /// isto é, que o hash interno aponta para a forma canônica e (se
    /// `previous` fornecido) que os campos de encadeamento batem.
    pub fn links_from(&self, previous: &Self) -> bool {
        self.previous_hash == previous.compute_hash()
    }
}

/// Cadeia de dados do field-stability — append-only, encadeada por hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceLedger {
    /// Entradas na ordem de ingesta (append-only, nunca reescritas).
    entries: Vec<CoherenceEntry>,
}

impl Default for CoherenceLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl CoherenceLedger {
    /// Ledger vazio ancorado em `GENESIS`.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Último hash efetivamente gravado na cadeia (`GENESIS` se vazio).
    pub fn last_hash(&self) -> String {
        self.entries
            .last()
            .map(CoherenceEntry::compute_hash)
            .unwrap_or_else(|| GENESIS.to_string())
    }

    /// Timestamp da última entrada gravada (0 se vazia).
    pub fn last_timestamp(&self) -> u64 {
        self.entries.last().map(|e| e.timestamp).unwrap_or(0)
    }

    /// Insere uma entrada, aplicando Gravity-1 (monotonia estrita de
    /// timestamp) **e** validando o encadeamento (`previous_hash` deve apontar
    /// para o último hash efetivo).
    ///
    /// Falha com erro explícito se violar qualquer invariante — a cadeia
    /// permanece intacta (Loopseal-2).
    pub fn push(&mut self, mut entry: CoherenceEntry) -> Result<(), String> {
        let last_ts = self.last_timestamp();
        if entry.timestamp <= last_ts {
            return Err(format!(
                "{} entrada (window_id={}) timestamp {} <= {}", 
                GRAVITY_1,
                entry.window_id,
                entry.timestamp,
                last_ts
            ));
        }
        if entry.previous_hash != self.last_hash() {
            return Err(format!(
                "Loopseal-2: previous_hash esparso '{}' != hash efetivo '{}' (window_id={})",
                entry.previous_hash,
                self.last_hash(),
                entry.window_id
            ));
        }
        entry.previous_hash = self.last_hash();
        entry.hash = entry.compute_hash();
        self.entries.push(entry);
        Ok(())
    }

    /// Apenas leitura — não expõe escrita; suporta iteração e estatísticas.
    pub fn entries(&self) -> &[CoherenceEntry] {
        &self.entries
    }

    /// Re-encadeia a cadeia desde o início e verifica cada hash — Loopseal-3.
    ///
    /// Para cada entrada: (1) o **hash interno** `entry.hash` é re-derivado pela
    /// forma canônica e comparado (isto detecta adulteração de conteúdo da
    /// **última** entrada — A1/D1, que a tautologia `compute_hash != compute_hash`
    /// ocultava); (2) o encadeamento `previous_hash` aponta para a entrada
    /// anterior / `GENESIS`.
    ///
    /// Retorna um [`IntegrityStatus`] semântico (D3):
    /// `Ok`, `Broken { window_ids }` ou `BeyondHorizon { len, max_windows }`
    /// (dados íntegros além do horizonte formal da modelagem TLA+/Lean).
    pub fn verify_integrity(&self) -> IntegrityStatus {
        let mut broken = Vec::new();
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.compute_hash() != entry.hash {
                broken.push(entry.window_id);
            }
            let prev = if i == 0 {
                entry.previous_hash == GENESIS
            } else {
                entry.links_from(&self.entries[i - 1])
            };
            if !prev {
                broken.push(entry.window_id);
            }
        }
        if !broken.is_empty() {
            IntegrityStatus::Broken { window_ids: broken }
        } else if self.entries.len() > MAX_HORIZON_WINDOWS {
            IntegrityStatus::BeyondHorizon {
                len: self.entries.len(),
                max_windows: MAX_HORIZON_WINDOWS,
            }
        } else {
            IntegrityStatus::Ok
        }
    }

    /// Média de um campo numérico das entradas.
    fn mean_of(&self, f: impl Fn(&CoherenceEntry) -> f64) -> f64 {
        if self.entries.is_empty() {
            return 0.0;
        }
        self.entries.iter().map(f).sum::<f64>() / self.entries.len() as f64
    }

    /// Coerência média `Φ` do ledger inteiro.
    pub fn mean_phi(&self) -> f64 {
        self.mean_of(|e| e.phi)
    }

    /// Relatório final autogerado em Markdown (Fase 4): tabela da cadeia
    /// completa + estatística agregada (`Φ` médio).
    pub fn generate_report(&self) -> String {
        let mut out = String::new();
        let integrity = match self.verify_integrity() {
            IntegrityStatus::Ok => "OK".to_string(),
            IntegrityStatus::Broken { ref window_ids } => {
                let ids = window_ids.iter().map(u64::to_string).collect::<Vec<_>>().join(", ");
                format!("QUEBRADA (window_ids: {ids})")
            }
            IntegrityStatus::BeyondHorizon { len, max_windows } => format!(
                "OK (dados) — além do horizonte formal TLC/Lean (len={len} > MaxWindows={max_windows})"
            ),
        };
        out.push_str("# Relatório de coerência — cadeia de dados\n\n");
        out.push_str(&format!(
            "- **Entradas:** {}\n- **Âncora gênese:** {}\n- **Integridade (SHA3-256):** {}\n\n",
            self.entries.len(),
            GENESIS,
            integrity
        ));
        out.push_str("| window | timestamp | Φ | stability (Ω) | success_rate (Σ) | latency (Λ) | prev |\n");
        out.push_str("|---|---|---|---|---|---|---|\n");
        for e in &self.entries {
            let prev = &e.previous_hash[..e.previous_hash.len().min(12)];
            out.push_str(&format!(
                "| {} | {} | {:.4} | {:.4} | {:.4} | {:.4} | `{}…` |\n",
                e.window_id,
                e.timestamp,
                e.phi,
                e.stability,
                e.success_rate,
                e.latency_score,
                prev
            ));
        }
        out.push_str(&format!("\n**Φ médio:** {:.4}\n", self.mean_phi()));
        out
    }

    /// Serializa a cadeia inteira como JSON pretty.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(&self.entries).expect("legder serializable")
    }
}

/// Codifica um digest em hex minúsculo.
fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    let mut s = String::with_capacity(bytes.as_ref().len() * 2);
    for b in bytes.as_ref() {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(window_id: u64, timestamp: u64, phi: f64, prev: String) -> CoherenceEntry {
        CoherenceEntry::from_parts(window_id, timestamp, phi, 0.9, 0.95, 0.8, prev)
    }

    #[test]
    fn genesis_append_and_hash() {
        let mut ledger = CoherenceLedger::new();
        ledger.push(entry(0, 1, 0.96, GENESIS.into())).unwrap();
        assert_eq!(ledger.entries().len(), 1);
        assert_eq!(ledger.last_hash(), ledger.entries()[0].compute_hash());
        assert_eq!(ledger.verify_integrity(), IntegrityStatus::Ok);
        assert_eq!(ledger.entries()[0].hash, ledger.entries()[0].compute_hash());
    }

    #[test]
    fn chain_linking_two_entries() {
        let mut ledger = CoherenceLedger::new();
        ledger.push(entry(0, 1, 0.96, GENESIS.into())).unwrap();
        let h1 = ledger.last_hash();
        ledger.push(entry(1, 2, 0.97, h1)).unwrap();
        assert!(ledger.entries()[1].links_from(&ledger.entries()[0]));
        assert_eq!(ledger.entries().len(), 2);
        assert_eq!(ledger.verify_integrity(), IntegrityStatus::Ok);
    }

    #[test]
    fn gravity1_rejects_non_monotonic_timestamp() {
        let mut ledger = CoherenceLedger::new();
        ledger.push(entry(0, 5, 0.96, GENESIS.into())).unwrap();
        let err = ledger.push(entry(1, 5, 0.97, ledger.last_hash())).unwrap_err();
        assert!(err.contains(GRAVITY_1), "err: {err}");
        assert!(ledger.push(entry(1, 4, 0.97, ledger.last_hash())).is_err());
        // Cadeia intacta apesar do erro (Loopseal-2).
        assert_eq!(ledger.entries().len(), 1);
    }

    #[test]
    fn loopseal2_rejects_forged_previous_hash() {
        let mut ledger = CoherenceLedger::new();
        ledger.push(entry(0, 1, 0.96, GENESIS.into())).unwrap();
        let err = ledger.push(entry(1, 2, 0.97, "FAKE".into())).unwrap_err();
        assert!(err.contains("Loopseal-2"), "err: {err}");
        assert_eq!(ledger.entries().len(), 1);
    }

    #[test]
    fn verify_integrity_detects_tamper() {
        let mut ledger = CoherenceLedger::new();
        ledger.push(entry(0, 1, 0.96, GENESIS.into())).unwrap();
        let h1 = ledger.last_hash();
        ledger.push(entry(1, 2, 0.97, h1)).unwrap();
        // Corrompe a phi da primeira entrada.
        let mut tampered = ledger.entries()[0].clone();
        tampered.phi = 0.5;
        ledger.entries[0] = tampered;
        assert!(
            matches!(ledger.verify_integrity(), IntegrityStatus::Broken { .. }),
            "a adulteracao da primeira entrada deve apontar Broken"
        );
    }

    #[test]
    fn verify_integrity_detects_last_entry_content_tamper() {
        let mut ledger = CoherenceLedger::new();
        ledger.push(entry(0, 1, 0.96, GENESIS.into())).unwrap();
        let h1 = ledger.last_hash();
        ledger.push(entry(1, 2, 0.97, h1)).unwrap();
        // A1 (D1): mutação da ÚLTIMA entrada fora do `push`. Nenhuma entrada
        // seguinte re-deriva o hash dela, então a varredura que só compara o
        // encadeamento `previous_hash` não detecta. O hash interno (campo
        // re-derivado) precisa ser verificado explicitamente.
        let mut tampered = ledger.entries()[1].clone();
        tampered.phi = 0.10;
        ledger.entries[1] = tampered;
        assert_eq!(
            ledger.verify_integrity(),
            IntegrityStatus::Broken {
                window_ids: vec![1]
            },
            "A1: adulteracao de conteudo da ultima entrada deve ser detectada pelo hash interno"
        );
    }

    #[test]
    fn beyond_horizon_reported_above_max_windows() {
        let mut ledger = CoherenceLedger::new();
        let mut prev = GENESIS.to_string();
        for w in 0..MAX_HORIZON_WINDOWS as u64 + 1 {
            ledger.push(entry(w, 1 + w, 0.95, prev)).unwrap();
            prev = ledger.last_hash();
        }
        assert_eq!(ledger.entries().len(), 5);
        assert_eq!(
            ledger.verify_integrity(),
            IntegrityStatus::BeyondHorizon {
                len: 5,
                max_windows: MAX_HORIZON_WINDOWS
            }
        );
        // D3: BeyondHorizon nao e erro — integridade de dados confirmada.
        assert!(ledger.verify_integrity().is_ok());
    }

    #[test]
    fn within_horizon_clean_chain_is_ok() {
        let mut ledger = CoherenceLedger::new();
        let mut prev = GENESIS.to_string();
        for w in 0..MAX_HORIZON_WINDOWS as u64 {
            ledger.push(entry(w, 1 + w, 0.95, prev)).unwrap();
            prev = ledger.last_hash();
        }
        assert_eq!(ledger.verify_integrity(), IntegrityStatus::Ok);
        // Borda exata: len == MAX_HORIZON_WINDOWS ainda esta no horizonte
        // formal (guard TLA+ Len(chain) < MaxWindows atinge no maximo MaxWindows).
        assert_eq!(ledger.entries().len(), MAX_HORIZON_WINDOWS);
    }

    #[test]
    fn mean_phi_empty_and_populated() {
        let ledger = CoherenceLedger::new();
        assert_eq!(ledger.mean_phi(), 0.0);
        let mut l = CoherenceLedger::new();
        l.push(entry(0, 1, 0.9, GENESIS.into())).unwrap();
        l.push(entry(1, 2, 0.7, l.last_hash())).unwrap();
        assert!((l.mean_phi() - 0.8).abs() < 1e-12);
    }

    #[test]
    fn report_empty_is_safe() {
        let ledger = CoherenceLedger::new();
        let r = ledger.generate_report();
        assert!(r.contains("**Entradas:** 0"));
        assert!(!r.contains("NaN"));
    }

    #[test]
    fn report_populated_has_table_and_mean() {
        let mut l = CoherenceLedger::new();
        l.push(entry(0, 1, 0.9, GENESIS.into())).unwrap();
        l.push(entry(1, 2, 0.7, l.last_hash())).unwrap();
        let r = l.generate_report();
        assert!(r.contains("| 0 |"));
        assert!(r.contains("| 1 |"));
        assert!(r.contains("**Φ médio:** 0.8000"));
    }
}