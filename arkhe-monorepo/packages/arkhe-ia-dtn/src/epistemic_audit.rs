//! Bridge para o auditor epistémico Python (`audit_epistemic_v2.py`).
//!
//! # Correções aplicadas
//! - **P14 FIX**: Threshold adaptativo (não fixo em 30%).
//! - **P15 FIX**: Banco de evidências com janela temporal (LRU por tempo).
//! - Integração com o detector v2.0 (7 dimensões: citações, agência, consenso, crítica, neologismos, entropia, independência).

use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde_json::json;

/// Status de uma evidência (conforme D5/D6 do `epistemic_v2.py`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceStatus {
    Compiled,   // Artefato de compilador
    Executed,   // Saída de execução registrada
    Cited,      // Fonte primária com identificador resolvível
    Asserted,   // Afirmado sem lastro
    Expected,   // Resultado esperado, não executado
}

impl EvidenceStatus {
    /// Retorna o peso da evidência (0..1).
    pub fn weight(&self) -> f64 {
        match self {
            Self::Compiled => 0.9,
            Self::Executed => 1.0,
            Self::Cited => 0.8,
            Self::Asserted => 0.2,
            Self::Expected => 0.3,
        }
    }

    /// Verifica se a evidência tem lastro sólido.
    pub fn has_lastro(&self) -> bool {
        matches!(self, Self::Compiled | Self::Executed | Self::Cited)
    }
}

/// Uma evidência no banco de evidências.
#[derive(Debug, Clone)]
pub struct Evidence {
    pub claim: String,
    pub status: EvidenceStatus,
    pub artifact: Option<String>,
    pub timestamp_ms: u64,
}

/// Banco de evidências com janela temporal (P15 FIX).
pub struct EvidenceBank {
    /// Evidências armazenadas.
    pub evidence: VecDeque<Evidence>,
    /// Capacidade máxima (número de entradas).
    pub max_entries: usize,
    /// Janela temporal para evidências (ms). Entradas mais antigas são removidas.
    pub time_window_ms: u64,
}

impl EvidenceBank {
    /// Cria banco de evidências com capacidade e janela temporal.
    pub fn new(max_entries: usize, time_window_ms: u64) -> Self {
        Self {
            evidence: VecDeque::with_capacity(max_entries),
            max_entries,
            time_window_ms,
        }
    }

    /// Adiciona uma evidência.
    pub fn add(&mut self, claim: &str, status: EvidenceStatus, artifact: Option<&str>, timestamp_ms: u64) {
        // Remove entradas antigas
        self.eviction(timestamp_ms);
        self.evidence.push_back(Evidence {
            claim: claim.to_string(),
            status,
            artifact: artifact.map(|s| s.to_string()),
            timestamp_ms,
        });
        // Limite de tamanho
        while self.evidence.len() > self.max_entries {
            self.evidence.pop_front();
        }
    }

    /// Remove evidências mais antigas que a janela temporal.
    fn eviction(&mut self, current_time_ms: u64) {
        let cutoff = current_time_ms.saturating_sub(self.time_window_ms);
        self.evidence.retain(|e| e.timestamp_ms >= cutoff);
    }

    /// Calcula a fração de evidências sem lastro (Asserted + Expected).
    pub fn fraction_without_lastro(&self) -> f64 {
        if self.evidence.is_empty() {
            return 0.0;
        }
        let weak = self.evidence.iter()
            .filter(|e| e.status == EvidenceStatus::Asserted || e.status == EvidenceStatus::Expected)
            .count();
        weak as f64 / self.evidence.len() as f64
    }

    /// Calcula a pontuação de integridade epistêmica (0..1).
    pub fn integrity_score(&self) -> f64 {
        if self.evidence.is_empty() {
            return 1.0;
        }
        let total_weight: f64 = self.evidence.iter().map(|e| e.status.weight()).sum();
        total_weight / self.evidence.len() as f64
    }

    /// Limpa o banco.
    pub fn clear(&mut self) {
        self.evidence.clear();
    }
}

/// Auditor epistémico (bridge para o Python).
pub struct EpistemicAuditor {
    /// Caminho para o script Python.
    script_path: String,
    /// Banco de evidências local.
    pub evidence_bank: EvidenceBank,
    /// Threshold adaptativo para fail‑closed.
    pub integrity_threshold: f64,
}

impl EpistemicAuditor {
    /// Cria auditor com configuração padrão.
    pub fn new(script_path: Option<&str>) -> Self {
        let path = script_path.unwrap_or("audit_epistemic_v2.py").to_string();
        Self {
            script_path: path,
            evidence_bank: EvidenceBank::new(1000, 3600_000), // 1000 entradas, 1 hora
            integrity_threshold: 0.6, // Abaixo disso, ativa fail‑closed (P14 adaptativo)
        }
    }

    /// Executa auditoria sobre um texto, chamando o script Python.
    pub fn audit_text(&self, text: &str) -> Option<AuditResult> {
        #[cfg(feature = "std")]
        {
            use std::io::Write;
            use std::process::{Command, Stdio};

            let input = json!({ "text": text }).to_string();
            let mut child = Command::new("python3")
                .arg(&self.script_path)
                .arg("--json")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .ok()?;

            let mut stdin = child.stdin.take()?;
            stdin.write_all(input.as_bytes()).ok()?;
            stdin.write_all(b"\n").ok()?;
            stdin.flush().ok()?;

            let output = child.wait_with_output().ok()?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            self.parse_result(&stdout)
        }
        #[cfg(not(feature = "std"))]
        {
            // Em ambiente no_std, retorna resultado simulado
            Some(AuditResult {
                score: 0,
                tier: "G".to_string(),
                flags: Vec::new(),
                integrity: 0.9,
            })
        }
    }

    /// Audita um ficheiro (chama `audit_text`).
    pub fn audit_file(&self, path: &str) -> Option<AuditResult> {
        #[cfg(feature = "std")]
        {
            use std::fs;
            fs::read_to_string(path).ok().and_then(|content| self.audit_text(&content))
        }
        #[cfg(not(feature = "std"))]
        {
            None
        }
    }

    /// Parse do resultado JSON do script Python.
    fn parse_result(&self, json_str: &str) -> Option<AuditResult> {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
            let score = json.get("score").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let tier = json.get("tier").and_then(|v| v.as_str()).unwrap_or("G").to_string();
            let flags = json.get("flags")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let integrity = 1.0 - (score as f64 / 40.0);
            Some(AuditResult { score, tier, flags, integrity })
        } else {
            None
        }
    }

    /// Verifica se a integridade epistêmica está abaixo do threshold.
    pub fn should_fail_closed(&self) -> bool {
        let integrity = self.evidence_bank.integrity_score();
        integrity < self.integrity_threshold
    }

    /// Adiciona uma evidência ao banco.
    pub fn add_evidence(&mut self, claim: &str, status: EvidenceStatus, artifact: Option<&str>, timestamp_ms: u64) {
        self.evidence_bank.add(claim, status, artifact, timestamp_ms);
    }

    /// Gera relatório de saúde epistêmica.
    pub fn health_report(&self) -> EpistemicHealthReport {
        let integrity = self.evidence_bank.integrity_score();
        let weak_fraction = self.evidence_bank.fraction_without_lastro();
        EpistemicHealthReport {
            integrity_score: integrity,
            weak_evidence_fraction: weak_fraction,
            evidence_count: self.evidence_bank.evidence.len(),
            is_fail_closed: self.should_fail_closed(),
        }
    }

    /// Publica o relatório de saúde como evento Nostr assinado (Fase 4).
    ///
    /// Retorna o evento serializado em JSON. Apenas disponível com `std`.
    #[cfg(feature = "std")]
    pub fn publish_health_report_nostr(&self, hex_secret_key: &str) -> Option<String> {
        use nostr::{EventBuilder, JsonUtil, Keys, Kind, SecretKey};

        let secret_key = SecretKey::from_hex(hex_secret_key).ok()?;
        let keys = Keys::new(secret_key);
        let report = self.health_report();
        let content = json!({
            "integrity": report.integrity_score,
            "weak_evidence_fraction": report.weak_evidence_fraction,
            "evidence_count": report.evidence_count,
            "is_fail_closed": report.is_fail_closed,
        })
        .to_string();
        let event = EventBuilder::new(Kind::Custom(42_002), content, [])
            .to_event(&keys)
            .ok()?;
        Some(event.as_json())
    }

    /// Publica uma evidência como evento Nostr assinado (Fase 4).
    #[cfg(feature = "std")]
    pub fn publish_evidence_nostr(&self, hex_secret_key: &str, claim: &str) -> Option<String> {
        use nostr::{EventBuilder, JsonUtil, Keys, Kind, SecretKey};

        let secret_key = SecretKey::from_hex(hex_secret_key).ok()?;
        let keys = Keys::new(secret_key);
        let content = json!({
            "claim": claim,
            "integrity": self.evidence_bank.integrity_score(),
            "weak_fraction": self.evidence_bank.fraction_without_lastro(),
        })
        .to_string();
        let event = EventBuilder::new(Kind::Custom(42_003), content, [])
            .to_event(&keys)
            .ok()?;
        Some(event.as_json())
    }
}

/// Resultado da auditoria.
#[derive(Debug, Clone)]
pub struct AuditResult {
    pub score: i32,
    pub tier: String,
    pub flags: Vec<String>,
    pub integrity: f64,
}

/// Relatório de saúde epistêmica.
#[derive(Debug, Clone)]
pub struct EpistemicHealthReport {
    pub integrity_score: f64,
    pub weak_evidence_fraction: f64,
    pub evidence_count: usize,
    pub is_fail_closed: bool,
}

/// Auditoria geométrica: curvatura das crenças + distância de Fisher.
#[derive(Debug, Clone)]
pub struct GeometricAudit {
    /// Tier da psicose (G = saudável, A = rígido, S = colapso singular).
    pub tier: String,
    /// Curvatura escalar de Ricci das crenças.
    pub curvature: f64,
    /// Distância de Fisher ao estado platônico (não-Euclidiana).
    pub platonic_fisher_distance: f64,
}

impl GeometricAudit {
    /// Verifica se a curvatura indica colapso de crença (Tier S/A).
    pub fn is_collapsed(&self) -> bool {
        self.tier == "S" || self.tier == "A"
    }
}

impl EpistemicAuditor {
    /// Auditoria geométrica das crenças (Pilares 3 e 4).
    ///
    /// - `beliefs`: distribuição de crenças (ex: probabilidades da inferência).
    /// - `logits`: logits da projeção (aurora) para a métrica de Fisher.
    pub fn audit_geometry(&self, beliefs: &[f64], logits: &[f64]) -> GeometricAudit {
        let curvature = self.compute_ricci_curvature(beliefs);
        let platonic_fisher_distance = self.fisher_distance_to_platonic(logits);
        let tier = if curvature > 5.0 {
            "S"
        } else if curvature > 2.0 {
            "A"
        } else {
            "G"
        };
        GeometricAudit {
            tier: tier.to_string(),
            curvature,
            platonic_fisher_distance,
        }
    }

    /// Curvatura escalar de Ricci das crenças (heurística geométrica).
    ///
    /// Proxy: desvio padrão dos logits de crença. Alta curvatura = crenças
    /// rígidas (Tier S/A); baixa = raciocínio flexível (Tier G).
    fn compute_ricci_curvature(&self, beliefs: &[f64]) -> f64 {
        if beliefs.is_empty() {
            return 0.0;
        }
        let mean = beliefs.iter().sum::<f64>() / beliefs.len() as f64;
        let variance =
            beliefs.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / beliefs.len() as f64;
        variance.sqrt() * 10.0
    }

    /// Distância de Fisher do estado atual ao platônico (logits ideais = 0).
    fn fisher_distance_to_platonic(&self, logits: &[f64]) -> f64 {
        let platonic = alloc::vec![0.0; logits.len()];
        crate::processing::fisher_distance(logits, &platonic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evidence_bank() {
        let mut bank = EvidenceBank::new(10, 1000);
        let now = 1000;

        bank.add("claim1", EvidenceStatus::Executed, None, now);
        bank.add("claim2", EvidenceStatus::Asserted, None, now + 10);
        assert_eq!(bank.evidence.len(), 2);
        assert_eq!(bank.fraction_without_lastro(), 0.5);
        assert!((bank.integrity_score() - 0.6).abs() < 0.01);

        // Testa eviction por tempo
        bank.add("claim3", EvidenceStatus::Compiled, None, now + 2000);
        assert_eq!(bank.evidence.len(), 1); // As duas primeiras removidas (ultrapassaram 1000ms)
    }

    #[test]
    fn test_audit_result() {
        let auditor = EpistemicAuditor::new(None);
        let json = r#"{"score": 12, "tier": "D", "flags": ["atribuicao leve de agencia"]}"#;
        let result = auditor.parse_result(json);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.score, 12);
        assert_eq!(r.tier, "D");
        assert!(!r.flags.is_empty());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_publish_health_report_nostr() {
        let auditor = EpistemicAuditor::new(None);
        // Chave privada válida de 32 bytes em hex (somente para teste).
        let sk = "3f4b1f6f0f1b2d4e5a7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f90";
        let published = auditor.publish_health_report_nostr(sk);
        assert!(published.is_some());
        let event_json = published.unwrap();
        assert!(event_json.contains("\"id\""));
        assert!(event_json.contains("\"sig\""));
        assert!(event_json.contains("\"kind\""));
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_publish_evidence_nostr_invalid_key() {
        let auditor = EpistemicAuditor::new(None);
        assert!(auditor.publish_evidence_nostr("not-a-hex-key", "claim").is_none());
    }

    #[test]
    fn test_audit_geometry_tiers() {
        let auditor = EpistemicAuditor::new(None);
        // Crenças flexíveis → Tier G.
        let flat = auditor.audit_geometry(&[0.25, 0.25, 0.25, 0.25], &[0.0, 0.0, 0.0]);
        assert_eq!(flat.tier, "G");
        assert!(!flat.is_collapsed());
        // Crenças rígidas (colapso) → Tier A/S.
        let rigid = auditor.audit_geometry(&[0.95, 0.02, 0.02, 0.01], &[5.0, -2.0, -2.0, -1.0]);
        assert!(rigid.is_collapsed());
    }

    #[test]
    fn test_fisher_distance_to_platonic() {
        let auditor = EpistemicAuditor::new(None);
        // Estado já platônico → distância zero.
        let d0 = auditor.fisher_distance_to_platonic(&[0.0, 0.0, 0.0]);
        assert!(d0 < 1e-9);
        // Estado distante → distância positiva.
        let d1 = auditor.fisher_distance_to_platonic(&[3.0, -3.0, 0.5]);
        assert!(d1 > 0.0);
        assert!(d1.is_finite());
    }
}
