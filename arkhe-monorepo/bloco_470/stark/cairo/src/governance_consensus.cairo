// =============================================================================
// BLOCO 470 v13 — CIRCUITO CAIRO (O4)
// governance_consensus.cairo
//
// Circuito STARK para consenso ponderado de governança.
// Prova que a média ponderada (phi * weight / sum_weight) atingiu o threshold
// sem revelar os scores individuais de cada agente de IA.
//
// Compilação: scarb build   -> target/release/governance_stark.casm.json
// Provas:     stone-cli prove --program <casm> --program_input <json>
// =============================================================================
use core::array::ArrayTrait;
use core::array::SpanTrait;

#[derive(Copy, Drop, Serde)]
struct GovernanceInputs {
    phi_values: Array<felt252>,  // Scores de coerência (escalados por 100)
    weights: Array<felt252>,     // Pesos dos agentes
    threshold: felt252,          // Threshold (ex.: 85)
}

#[derive(Copy, Drop, Serde)]
struct GovernanceOutputs {
    weighted_phi: felt252,       // Média ponderada * 100
    consensus_reached: felt252,  // 1 se >= threshold
}

fn compute_weighted_phi(inputs: GovernanceInputs) -> GovernanceOutputs {
    let mut sum_phi: felt252 = 0;
    let mut sum_w: felt252 = 0;
    let mut i: felt252 = 0;

    loop {
        if i >= inputs.phi_values.len() {
            break;
        }
        let phi = *inputs.phi_values.at(i as usize);
        let w = *inputs.weights.at(i as usize);
        sum_phi += phi * w;
        sum_w += w;
        i += 1;
    };

    // Evita divisão por zero
    if sum_w == 0 {
        return GovernanceOutputs {
            weighted_phi: 0,
            consensus_reached: 0,
        };
    }

    // Média ponderada: (sum_phi / sum_w) * 100
    let weighted_phi = (sum_phi * 100) / sum_w;
    let consensus_reached = if weighted_phi >= inputs.threshold { 1 } else { 0 };

    GovernanceOutputs {
        weighted_phi: weighted_phi,
        consensus_reached: consensus_reached,
    }
}

#[test]
fn test_governance() {
    let mut phi_values: Array<felt252> = ArrayTrait::new();
    phi_values.append(90);
    phi_values.append(85);
    phi_values.append(78);

    let mut weights: Array<felt252> = ArrayTrait::new();
    weights.append(3);
    weights.append(2);
    weights.append(1);

    let inputs = GovernanceInputs {
        phi_values: phi_values,
        weights: weights,
        threshold: 85,
    };

    let outputs = compute_weighted_phi(inputs);
    assert(outputs.weighted_phi == 86, 'weighted_phi should be 86');
    assert(outputs.consensus_reached == 1, 'consensus should be reached');
}