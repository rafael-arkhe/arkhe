// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8.24;

// =============================================================================
// STARKVerifier.sol — gerado automaticamente por:
//
//   stone-cli serialize-proof \
//     --proof bloco_470/stark/proofs/governance_consensus_proof.json \
//     --output contracts/verifiers/STARKVerifier.sol \
//     --target evm
//
// Este arquivo é um placeholder de compilação. Substitua pelo Solidity gerado
// pelo stone-cli (stark-evm-adapter) no pipeline de CI (bloco_470/.github/
// workflows/stone-cli.yml, job deploy-verifier-sepolia).
// =============================================================================

contract STARKVerifier {
    function verify(bytes calldata proof, bytes calldata)
        external
        pure
        returns (bool)
    {
        // Placeholder — compilação gera o verificador real no pipeline.
        return proof.length > 0;
    }
}