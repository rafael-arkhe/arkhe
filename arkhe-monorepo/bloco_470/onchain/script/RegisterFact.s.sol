// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8.24;

import "forge-std/Script.sol";

/// @title Registro de fact (hash) on-chain após verificação bem-sucedida (v15, S1)
/// @dev Fact registry similar ao modelo StarkEx/StarkNet: prova verificada
///      publicamente => fact registrado cost narrativo para o TemporalChain.
contract FactRegistry {
    mapping(bytes32 => bool) public facts;
    event FactRegistered(bytes32 indexed fact, address indexed registrar);

    function registerFact(bytes32 fact) external {
        require(!facts[fact], "Fact already registered");
        facts[fact] = true;
        emit FactRegistered(fact, msg.sender);
    }

    function isFact(bytes32 fact) external view returns (bool) {
        return facts[fact];
    }
}

contract RegisterFact is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("VERIFIER_PRIVATE_KEY");

        // fact = keccak(proof_public_inputs); em produção derivado da prova STARK
        bytes32 fact = vm.envOr("FACT", bytes32(0));

        vm.startBroadcast(deployerPrivateKey);

        FactRegistry registry = FactRegistry(vm.envAddress("FACT_REGISTRY_ADDRESS"));
        if (fact == bytes32(0)) {
            console.log("FACT not provided; simulating registration of zero fact");
        } else {
            registry.registerFact(fact);
            console.logBytes32(fact);
            console.log("Fact registered by:", msg.sender);
        }

        vm.stopBroadcast();
    }
}