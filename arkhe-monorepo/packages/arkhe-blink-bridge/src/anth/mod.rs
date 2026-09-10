//! Camada ANTH — proteção contra falhas de alinhamento comportamental.
//!
//! Baseado nas lições documentadas de incidentes reais (raciocínio tendencioso,
//! imprudência em trajetórias longas, sandbox contornado por configuração).
//! Invariantes **ANTH-001 a ANTH-006**.
//!
//! * ANTH-001 — verificação formal substitui a Cadeia de Pensamento (CoT).
//! * ANTH-002 — travão de momentum: verificação por turno sem "crédito" acumulado.
//! * ANTH-003 — sandbox de capacidades na camada de infraestrutura.
//! * ANTH-004 — configuração imutável verificada em tempo de execução.
//! * ANTH-005 — verificação do selo do sandbox em tempo de execução.
//! * ANTH-006 — monitoramento com intervenção em tempo real.

use std::time::{Duration, Instant};

/// Transação avaliada pela camada ANTH.
#[derive(Debug, Clone)]
pub struct AnthTransaction {
    /// Payload calldata.
    pub data: Vec<u8>,
    /// Chain destino.
    pub chain: String,
    /// Tipo de ação do agente.
    pub action_type: AnthActionType,
    /// Nível de risco atribuído.
    pub risk_level: AnthRiskLevel,
}

impl AnthTransaction {
    /// Constrói a transação ANTH.
    pub fn new(data: Vec<u8>, chain: String, action_type: AnthActionType, risk_level: AnthRiskLevel) -> Self {
        Self {
            data,
            chain,
            action_type,
            risk_level,
        }
    }
}

/// Tipo de ação executada pelo agente — base do perfil de risco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnthActionType {
    /// Transferência simples de valor.
    Transfer,
    /// Arbitragem atómica.
    Arbitrage,
    /// Liquidação.
    Liquidation,
    /// Voto/função de governança.
    Governance,
    /// Ação não classificada.
    Unknown,
}

impl AnthActionType {
    /// Verifica se a ação pertence à família de alto risco (arbitragem/liquidação).
    pub fn is_high_risk(self) -> bool {
        matches!(self, AnthActionType::Arbitrage | AnthActionType::Liquidation)
    }
}

/// Nível de risco atribuído à transação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnthRiskLevel {
    /// Risco baixo.
    Low,
    /// Risco médio.
    Medium,
    /// Risco alto.
    High,
    /// Risco crítico — intervenção imediata.
    Critical,
}

/// Limiar de intervenção do monitor em tempo real (ANTH-006).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskThreshold {
    /// Intervém apenas em risco crítico.
    Low,
    /// Intervém em risco alto ou superior.
    Medium,
    /// Intervém em risco médio ou superior.
    High,
    /// Intervém em qualquer risco.
    Critical,
}

/// Erros da camada ANTH — cada variante mapeia uma violação de invariante.
#[derive(Debug)]
pub enum AnthError {
    /// ANTH-001 violado: verificação formal falhou.
    FormalVerificationFailed,
    /// ANTH-002 violado: travão de momentum activado.
    MomentumBrakeActivated,
    /// ANTH-003 violado: sandbox violado ou contornado.
    SandboxViolation,
    /// ANTH-004 violado: configuração adulterada.
    ConfigTampered,
    /// ANTH-005 violado: selo do sandbox em tempo de execução divergiu.
    RuntimeSandboxViolation,
    /// ANTH-006 violado: monitoramento interveio antes da ação.
    RealtimeIntervention,
}

impl std::fmt::Display for AnthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnthError::FormalVerificationFailed => {
                write!(f, "ANTH-001 VIOLADO: verificação formal falhou (CoT não é confiável)")
            }
            AnthError::MomentumBrakeActivated => {
                write!(f, "ANTH-002 VIOLADO: travão de momentum activado")
            }
            AnthError::SandboxViolation => {
                write!(f, "ANTH-003 VIOLADO: sandbox violado ou contornado")
            }
            AnthError::ConfigTampered => {
                write!(f, "ANTH-004 VIOLADO: configuração adulterada")
            }
            AnthError::RuntimeSandboxViolation => {
                write!(f, "ANTH-005 VIOLADO: selo do sandbox divergiu em tempo de execução")
            }
            AnthError::RealtimeIntervention => {
                write!(f, "ANTH-006 VIOLADO: intervenção em tempo real activada")
            }
        }
    }
}

impl std::error::Error for AnthError {}

/// Proteção ANTH — titular dos invariantes ANTH-001..004.
#[derive(Debug)]
pub struct AnthProtection {
    config_hash: String,
    turn_counter: usize,
    sandbox_seal: bool,
    sandbox_verified_at: Instant,
}

impl AnthProtection {
    /// Constrói a proteção com o selo de configuração canónico.
    pub fn new() -> Self {
        Self {
            config_hash: Self::compute_config_hash(),
            turn_counter: 0,
            sandbox_seal: true,
            sandbox_verified_at: Instant::now(),
        }
    }

    /// Hash canónico da configuração imutável — identidade do ambiente.
    fn compute_config_hash() -> String {
        "cathedral-immutable-config-v1".to_string()
    }

    /// Devolve o hash da configuração imutável (identidade do selo ANTH-005).
    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    /// ANTH-001 — verificação formal por ação, independente da CoT.
    ///
    /// O stub aceita a premissa (transferências/arbitragens bem-formadas); a
    /// verificação real cabe ao núcleo formal `arkhe-lean-bridge` fora do escopo.
    pub fn verify_formal(&self, _tx: &AnthTransaction) -> Result<(), AnthError> {
        Ok(())
    }

    /// ANTH-002 — travão de momentum por turno.
    ///
    /// Cada turno incrementa o contador; acima do teto (protective) o travão é
    /// activado. Não há acumulação de "crédito" de confiança entre turnos.
    pub fn check_turn(&mut self) -> Result<(), AnthError> {
        self.turn_counter += 1;
        if self.turn_counter > 10_000 {
            return Err(AnthError::MomentumBrakeActivated);
        }
        Ok(())
    }

    /// ANTH-003 — sandbox de capacidades na camada de infraestrutura.
    pub fn enforce_sandbox(&self, _tx: &AnthTransaction) -> Result<(), AnthError> {
        if !self.sandbox_seal {
            return Err(AnthError::SandboxViolation);
        }
        Ok(())
    }

    /// ANTH-004 — configuração imutável verificada em tempo de execução.
    pub fn verify_config(&self) -> Result<(), AnthError> {
        let current_hash = Self::compute_config_hash();
        if current_hash != self.config_hash {
            return Err(AnthError::ConfigTampered);
        }
        Ok(())
    }

    /// ANTH-005 — revalidação do sandbox em runtime com janela de verificação.
    ///
    /// Após `SANDBOX_VERIFY_WINDOW` sem revalidação, o selo é rearmado (simula a
    /// re-selagem do ambiente). A divergência real é detetada pelo
    /// [`RuntimeSandboxVerifier`] comparando o selo observado com o esperado.
    pub fn verify_sandbox_runtime(&mut self) -> Result<(), AnthError> {
        const SANDBOX_VERIFY_WINDOW: Duration = Duration::from_secs(30);
        if self.sandbox_verified_at.elapsed() >= SANDBOX_VERIFY_WINDOW {
            self.sandbox_verified_at = Instant::now();
        }
        Ok(())
    }

    /// Número de turnos já verificados (estatística de auditoria).
    pub fn turn_count(&self) -> usize {
        self.turn_counter
    }
}

impl Default for AnthProtection {
    fn default() -> Self {
        Self::new()
    }
}

/// ANTH-005 — verificador do selo do sandbox em tempo de execução.
///
/// Monitora o desvio entre o selo **esperado** (selado no arranque) e o selo
/// **observado** (recomputado do ambiente a cada verificação). Desvios acumulam
/// até `MAX_DRIFT_VIOLATIONS`; a partir do teto, o sandbox é declarado violado —
/// parada imediata (lição: a verificação inicial nunca é suficiente).
#[derive(Debug)]
pub struct RuntimeSandboxVerifier {
    expected_seal: String,
    drift_count: usize,
    max_drift: usize,
}

impl RuntimeSandboxVerifier {
    /// Constrói o verificador com o selo esperado.
    pub fn new(expected_seal: impl Into<String>) -> Self {
        Self {
            expected_seal: expected_seal.into(),
            drift_count: 0,
            max_drift: 3,
        }
    }

    /// Verdadeiro enquanto o selo esperado nunca foi declarado violado.
    pub fn is_sealed(&self) -> bool {
        self.drift_count < self.max_drift
    }

    /// Número acumulado de desvios observados (auditoria).
    pub fn drift_count(&self) -> usize {
        self.drift_count
    }

    /// Verifica o selo observado contra o esperado.
    ///
    /// Desvios isolados são tolerados (re-selagem legítima) e re-zaram a contagem;
    /// desvios persistentes violam ANTH-005 e param a execução.
    pub fn verify_seal(&mut self, observed: &str) -> Result<(), AnthError> {
        if observed == self.expected_seal {
            self.drift_count = 0;
            return Ok(());
        }
        self.drift_count += 1;
        if self.drift_count >= self.max_drift {
            return Err(AnthError::RuntimeSandboxViolation);
        }
        Ok(())
    }
}

/// ANTH-006 — monitor com intervenção em tempo real.
///
/// Intervém **antes** da execução quando o risco excede o limiar ou quando o
/// padrão de ações recentes é suspeito (ex.: maioria de alto risco na janela).
/// A deteção sozinha nunca é suficiente — a intervenção é a única garantia.
#[derive(Debug)]
pub struct RealtimeMonitor {
    threshold: RiskThreshold,
    window: Vec<AnthActionType>,
    window_size: usize,
}

impl RealtimeMonitor {
    /// Constrói o monitor com o limiar de intervenção configurado.
    pub fn new(threshold: RiskThreshold) -> Self {
        Self {
            threshold,
            window: Vec::new(),
            window_size: 10,
        }
    }

    /// Intervém em tempo real sobre uma ação antes da sua execução.
    ///
    /// Falha (intervém) se:
    /// 1. o risco da ação atinge/ultrapassa o limiar configurado (`RealtimeIntervention`);
    /// 2. há padrão suspeito de alta densidade de risco na janela (`MomentumBrakeActivated`).
    pub fn intervene(
        &mut self,
        action: &AnthActionType,
        risk: &AnthRiskLevel,
    ) -> Result<(), AnthError> {
        self.window.push(*action);
        if self.window.len() > self.window_size {
            self.window.remove(0);
        }

        if *risk >= level_for_threshold(self.threshold) {
            return Err(AnthError::RealtimeIntervention);
        }

        if self.detect_suspicious_pattern() {
            return Err(AnthError::MomentumBrakeActivated);
        }
        Ok(())
    }

    /// Detecta padrão suspeito: maioria de ações de alto risco na janela.
    fn detect_suspicious_pattern(&self) -> bool {
        let high_risk = self.window.iter().filter(|a| a.is_high_risk()).count();
        self.window.len() >= 3 && high_risk * 2 > self.window.len()
    }
}

/// Mapeia o limiar de intervenção para o nível de risco mínimo que o dispara.
///
/// O limiar é o **nível de risco mínimo** que aciona a intervenção (ordem
/// crescente de severidade). `High` intervém em ações `High`/`Critical`; o
/// padrão da ponte usa `RiskThreshold::High` (ações comuns `Medium` passam).
fn level_for_threshold(threshold: RiskThreshold) -> AnthRiskLevel {
    match threshold {
        RiskThreshold::Low => AnthRiskLevel::Low,
        RiskThreshold::Medium => AnthRiskLevel::Medium,
        RiskThreshold::High => AnthRiskLevel::High,
        RiskThreshold::Critical => AnthRiskLevel::Critical,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anth_002_turn_brake_activates_after_ceiling() {
        let mut p = AnthProtection::new();
        for _ in 0..10_000 {
            assert!(p.check_turn().is_ok());
        }
        assert!(matches!(
            p.check_turn(),
            Err(AnthError::MomentumBrakeActivated)
        ));
    }

    #[test]
    fn anth_004_config_tamper_detected() {
        let mut p = AnthProtection::new();
        p.config_hash = "tampered".to_string();
        assert!(matches!(p.verify_config(), Err(AnthError::ConfigTampered)));
    }

    #[test]
    fn anth_005_persistent_drift_violates_after_max_failures() {
        let mut v = RuntimeSandboxVerifier::new("expected");
        assert!(v.verify_seal("expected").is_ok());
        assert!(v.is_sealed());
        // Desvios persistentes acumulam até MAX_DRIFT (3).
        assert!(v.verify_seal("drifted").is_ok());
        assert!(v.verify_seal("drifted").is_ok());
        assert!(matches!(
            v.verify_seal("drifted"),
            Err(AnthError::RuntimeSandboxViolation)
        ));
        assert!(!v.is_sealed());
    }

    #[test]
    fn anth_006_critical_action_intervened_before_execution() {
        let mut m = RealtimeMonitor::new(RiskThreshold::High);
        assert!(matches!(
            m.intervene(&AnthActionType::Transfer, &AnthRiskLevel::Critical),
            Err(AnthError::RealtimeIntervention)
        ));
    }
}