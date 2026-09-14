// =============================================================================
// BLOCO 470 v13 — CIRCUITO CAIRO (O4)
// policy_evaluation.cairo
//
// Circuito STARK para avaliação de política:
// policy_satisfied = (phi_scaled >= threshold) && !critical_violations
// =============================================================================
use core::array::ArrayTrait;

#[derive(Copy, Drop, Serde)]
struct PolicyInputs {
    phi_scaled: felt252,        // Coerência escalada (0-100)
    threshold: felt252,         // Limiar da política (ex.: 85)
    critical_violations: felt252, // ≥1 => política insatisfeita
}

#[derive(Copy, Drop, Serde)]
struct PolicyOutputs {
    policy_satisfied: felt252,
    requires_manual_review: felt252,
}

fn evaluate_policy(inputs: PolicyInputs) -> PolicyOutputs {
    let policy_satisfied =
        if inputs.phi_scaled >= inputs.threshold && inputs.critical_violations == 0 {
            1
        } else {
            0
        };

    let requires_manual_review =
        if inputs.critical_violations > 0 { 1 } else { 0 };

    PolicyOutputs {
        policy_satisfied: policy_satisfied,
        requires_manual_review: requires_manual_review,
    }
}

#[test]
fn test_policy() {
    let inputs = PolicyInputs {
        phi_scaled: 92,
        threshold: 85,
        critical_violations: 0,
    };
    let outputs = evaluate_policy(inputs);
    assert(outputs.policy_satisfied == 1, 'policy should be satisfied');
    assert(outputs.requires_manual_review == 0, 'no manual review needed');
}