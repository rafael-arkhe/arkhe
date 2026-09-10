//! Testes de integração da `UnifiedBlinkBridge` (bloco 1052).
//!
//! Cobertura transversal: fluxo feliz (16 invariantes), rejeição MEV-004
//! (searcher não autorizado), MEV-005 (chain não suportada), I623 (energia),
//! ANTH-006 (intervenção realtime), ANTH-005 (selo runtime) e consultas de
//! estado (sandbox/configuração).

use arkhe_blink_bridge::anth::{AnthActionType, AnthRiskLevel, RuntimeSandboxVerifier};
use arkhe_blink_bridge::{
    AnthError, ArkheError, BridgeError, MevError, UnifiedBlinkBridge, UnifiedTransaction,
    ARKHE_INVARIANTS, ANTH_INVARIANTS, MEV_INVARIANTS,
};

fn happy_tx() -> UnifiedTransaction {
    UnifiedTransaction::new(
        b"test_tx_data".to_vec(),
        "ethereum".to_string(),
        30_000_000_000,
        1_000_000_000,
        true,
        true,
        Some("flashbots".to_string()),
        AnthActionType::Arbitrage,
        AnthRiskLevel::Medium,
        true,
        true,
        500_000,
        "master".to_string(),
    )
}

/// Cobertura mínima das 16 invariantes declaradas.
#[test]
fn consolidacao_cobre_16_invariantes() {
    assert_eq!(MEV_INVARIANTS.len(), 6);
    assert_eq!(ANTH_INVARIANTS.len(), 6);
    assert_eq!(ARKHE_INVARIANTS.len(), 4);
    assert_eq!(MEV_INVARIANTS.len() + ANTH_INVARIANTS.len() + ARKHE_INVARIANTS.len(), 16);
}

/// Fluxo feliz: a transação atravessa as 16 verificações e emite o recibo.
#[test]
fn unified_transaction_flow_happy_path() {
    let mut bridge = UnifiedBlinkBridge::new();
    let receipt = bridge.process_transaction(happy_tx()).unwrap();
    assert!(receipt.privacy_guaranteed, "MEV-001");
    assert!(receipt.atomicity_guaranteed, "MEV-002");
    assert!(receipt.gas_refund > 0, "MEV-003");
    assert!(receipt.recovered_mev > 0, "MEV-003");
    assert!(receipt.searcher_validated, "MEV-004");
    assert!(receipt.chain_compatible, "MEV-005");
    assert!(receipt.immutable, "MEV-006");
    assert!(receipt.anth_verified, "ANTH-001..006");
    assert!(receipt.arkhe_verified, "I619/I622/I623/I624");
}

/// MEV-004: searcher fora do catálogo aprovado é rejeitado.
#[test]
fn mev_004_searcher_validation_rejects_untrusted() {
    let mut bridge = UnifiedBlinkBridge::new();
    let mut tx = happy_tx();
    tx.searcher_id = Some("untrusted_searcher".to_string());
    let err = bridge.process_transaction(tx).unwrap_err();
    assert!(matches!(err, BridgeError::Mev(MevError::SearcherUnauthorized)));
}

/// MEV-005: as seis chains canónicas são suportadas.
#[test]
fn mev_005_supports_all_six_chains() {
    for chain in ["ethereum", "base", "solana", "arbitrum", "bsc", "polygon"] {
        // Ponte nova por chain: ANTH-006 detecta padrão suspeito por trajetória
        // de agente, não por catálogo de chains.
        let mut bridge = UnifiedBlinkBridge::new();
        let mut tx = happy_tx();
        tx.chain = chain.to_string();
        let receipt = bridge.process_transaction(tx).unwrap();
        assert!(receipt.chain_compatible, "MEV-005 falhou para {chain}");
    }
}

/// I623: energia requerida acima da reserva é rejeitada.
#[test]
fn arkhe_i623_energy_deficit_rejected() {
    let mut bridge = UnifiedBlinkBridge::new();
    let mut tx = happy_tx();
    tx.energy_estimate = 5_000_000;
    let err = bridge.process_transaction(tx).unwrap_err();
    assert!(matches!(
        err,
        BridgeError::Arkhe(ArkheError::InsufficientEnergy { .. })
    ));
}

/// ANTH-006: ação de risco crítico é interrompida em tempo real antes da submissão.
#[test]
fn anth_006_realtime_intervention_blocks_critical() {
    let mut bridge = UnifiedBlinkBridge::new();
    let mut tx = happy_tx();
    tx.risk_level = AnthRiskLevel::Critical;
    let err = bridge.process_transaction(tx).unwrap_err();
    assert!(matches!(err, BridgeError::Anth(AnthError::RealtimeIntervention)));
}

/// ANTH-005: desvio persistente do selo viola o sandbox runtime após o teto.
#[test]
fn anth_005_runtime_sandbox_drift_detected() {
    let mut verifier = RuntimeSandboxVerifier::new("canonical-seal");
    // Selo coerente: contagem zera e nada viola.
    for _ in 0..5 {
        assert!(verifier.verify_seal("canonical-seal").is_ok());
    }
    assert!(verifier.is_sealed());
    // Desvio persistente: acumula até MAX_DRIFT (3) e viola.
    assert!(verifier.verify_seal("drifted-seal").is_ok());
    assert!(verifier.verify_seal("drifted-seal").is_ok());
    let err = verifier.verify_seal("drifted-seal").unwrap_err();
    assert!(matches!(err, AnthError::RuntimeSandboxViolation));
    assert!(!verifier.is_sealed());
}

/// Estado consultável da ponte: sandbox selado e configuração íntegra.
#[test]
fn bridge_state_queries() {
    let bridge = UnifiedBlinkBridge::new();
    assert!(bridge.get_sandbox_status(), "ANTH-005: sandbox deve estar selado");
    assert!(bridge.verify_config_integrity(), "ANTH-004: configuração íntegra");
    let stats = bridge.get_mev_stats();
    assert!(!stats.chain_compatible, "sem submissão ainda — receipt vazio");
}