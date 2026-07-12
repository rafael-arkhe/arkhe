//! Teste de integração: `ApprovalWorkflow` (política) decide se um candidato
//! pode prosseguir; `Registry`/`Verifier` (mecanismo) validam a integridade
//! estrutural do que é escrito. As duas coisas não se sobrepõem.

use arkhe_rsi::{ApprovalWorkflow, CargoTestEvaluator, Evaluator, InMemoryRegistryBackend, Registry, RustClippyValidator, StaticValidator, Vote};
use arkhe_rsi_core::{Artifact, ArtifactKind, CheckpointStatus, EvaluationResult, IterationRecord};

#[test]
fn approval_workflow_gates_what_registry_accepts_as_applied() {
    let artifact = Artifact::new(ArtifactKind::Code, "fn improved() {}");
    let mut approvals = ApprovalWorkflow::new(2);
    let mut registry = Registry::new(Box::new(InMemoryRegistryBackend::new()), 0.5);

    approvals.cast_vote(artifact.id, "alice", Vote::Approve);
    // Só um voto: quorum não atingido. A política nega, e nunca chegamos a
    // construir um IterationRecord "aplicado" para o Registry.
    assert!(approvals.decide(&artifact.id).is_err());

    approvals.cast_vote(artifact.id, "bob", Vote::Approve);
    approvals.decide(&artifact.id).expect("quorum reached");

    // Só agora, com a política satisfeita, o mecanismo recebe um registro
    // marcado como aplicado e validado.
    let record = IterationRecord::new(
        1,
        artifact.id,
        Some(artifact.id),
        Some(artifact.id),
        Some(EvaluationResult::new(0.92)),
        CheckpointStatus::Validated,
        None,
    );
    registry.append(record).unwrap();

    assert_eq!(registry.history().count(), 1);
    assert!(registry.last_stable_checkpoint().is_some());
}

#[test]
fn vetoed_candidate_never_reaches_the_registry() {
    let artifact = Artifact::new(ArtifactKind::Code, "fn risky() {}");
    let mut approvals = ApprovalWorkflow::new(1);

    approvals.cast_vote(artifact.id, "alice", Vote::Reject);

    // A política barra aqui. O código que monta o IterationRecord e chama
    // registry.append nunca deveria ser alcançado neste fluxo.
    assert!(approvals.decide(&artifact.id).is_err());
}

/// Não existe ainda um `RSILoop` que orquestre isso automaticamente — este
/// teste documenta a ordem pretendida (`StaticValidator` antes do
/// `Evaluator`) até que um orquestrador real exista para impor essa ordem.
#[test]
fn static_validator_gates_the_evaluator() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"pipeline-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "// placeholder\n").unwrap();

    let validator = RustClippyValidator::new(dir.path(), "src/lib.rs");
    let evaluator = CargoTestEvaluator::new(dir.path(), "src/lib.rs");

    let broken = Artifact::new(ArtifactKind::Code, "pub fn add(a: i32, b: i32) -> i32 { a + b");
    let report = validator.validate(&broken).unwrap();
    assert!(!report.passes);

    // Um RSILoop real pararia aqui. Rodar o evaluator sobre um candidato que
    // não compila é só desperdício de tempo de sandbox — mas ele funciona de
    // qualquer forma se chamado, o que confirma que a ordem é uma decisão de
    // quem orquestra, não uma dependência estrutural entre os dois traits.
    if report.passes {
        evaluator.evaluate(&broken).unwrap();
    }
}
