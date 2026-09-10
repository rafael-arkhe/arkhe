//! Cenarios de integracao AID (bloco 1074) — fluxos de ponta a ponta.
//!
//! HONESTIDADE: os testes exercitam as realizacoes de sanidade sobre o
//! substrato real; nenhum comportamento e fabricado. O nucleo de provas
//! formais vive em `src/lean/AgentIdentityNucleus.lean` (sem FFI).

use arkhe_aid::{AgentIdentity, Delegation, Scope, SignedAuditEvent};

#[test]
fn delegation_chain_never_widens() {
    // A -> B -> C: a cadeia so aciona se cada salto for atenuado NA HIERARQUIA,
    // e o resultado e: escopos de C ⊆ escopos de A (propriedade transitiva).
    let (a, a_key) = AgentIdentity::new();
    let (b, b_key) = AgentIdentity::new();
    let (c, _c_key) = AgentIdentity::new();

    let a_scopes = vec![Scope::Read, Scope::Write, Scope::Execute];
    let ab = Delegation::new(&a, &b, vec![Scope::Read, Scope::Write], &a_key);
    let bc = Delegation::new(&b, &c, vec![Scope::Read], &b_key);

    // Cada salto e atenuado em relacao ao escopo do delegador RESPECTIVO.
    assert!(ab.is_attenuated(&a_scopes));
    // B so detem o que A lhe concedeu:
    let b_effective = ab.scopes.clone();
    assert!(bc.is_attenuated(&b_effective));

    // Transitividade: o que C pode fazer ⊆ o que o AUTHORITY original definiu.
    assert!(bc.is_attenuated(&a_scopes));
}

#[test]
fn signed_audit_full_chain_survives_resequence_detection() {
    let (identity, key) = AgentIdentity::new();
    let mut events: Vec<SignedAuditEvent> = Vec::new();
    let genesis = SignedAuditEvent::new(
        &identity,
        &key,
        "init",
        "sys",
        "success",
        "ctx-a",
        None,
    );
    events.push(genesis);
    for i in 0..5 {
        let prev = events.last().unwrap().compute_hash();
        let e = SignedAuditEvent::new(
            &identity,
            &key,
            "op",
            &format!("r{i}"),
            "success",
            "ctx-a",
            Some(prev),
        );
        events.push(e);
    }
    // Assinatura valida em todos os elos e encadeamento exato.
    assert!(events.iter().all(|e| e.verify(&identity)));
    assert!(events.windows(2).all(|w| w[1].follows(&w[0])));

    // Reordenacao detectada: trocar dois elos adjacentes quebra `follows`.
    events.swap(2, 3);
    assert!(!events.windows(2).all(|w| w[1].follows(&w[0])));
}

#[test]
fn sod_guards_payment_flow() {
    let mut op = arkhe_aid::SensitiveOperation {
        id: "payment-1".to_string(),
        steps: vec![
            arkhe_aid::OperationStep {
                name: "create".to_string(),
                authorized_agent: "treasurer".to_string(),
            },
            arkhe_aid::OperationStep {
                name: "approve".to_string(),
                authorized_agent: "director".to_string(),
            },
        ],
        executed_by: Vec::new(),
    };
    assert!(op.try_record("treasurer", "create").is_ok());
    assert!(op.try_record("director", "approve").is_ok());
    assert!(op.is_complete());
}