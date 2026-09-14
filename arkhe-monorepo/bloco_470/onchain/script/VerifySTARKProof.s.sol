// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import {STARKVerifier} from "../contracts/verifiers/STARKVerifier.sol";

/// @title Verificação on-chain de uma prova STARK (v15, S1)
contract VerifySTARKProof is Script {
    function run() external {
        // Em produção: carregar proof + public inputs do job generate-stark-proofs
        bytes memory proof = vm.envOr("PROOF_BYTES", bytes("0x"));
        bytes memory publicInputs = vm.envOr("PUBLIC_INPUTS", bytes("0x"));

        address verifier = vm.envAddress("STARK_VERIFIER_ADDRESS");
        STARKVerifier sv = STARKVerifier(verifier);

        uint256 deployerPrivateKey = vm.envUint("VERIFIER_PRIVATE_KEY");
        vm.startBroadcast(deployerPrivateKey);

        bool ok = sv.verify(proof, publicInputs);
        console.log("STARK proof verified:", ok);

        vm.stopBroadcast();
    }
}