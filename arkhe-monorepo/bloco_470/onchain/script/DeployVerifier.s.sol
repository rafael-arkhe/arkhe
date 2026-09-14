// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import {GovernanceVerifier} from "../contracts/verifiers/Groth16Verifier.sol";

/// @title Deploy do GovernanceVerifier na rede de teste via Foundry (v13, O1)
contract DeployVerifier is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");

        vm.startBroadcast(deployerPrivateKey);

        GovernanceVerifier verifier = new GovernanceVerifier();
        console.log("Verifier deployed at:", address(verifier));

        vm.stopBroadcast();
    }
}