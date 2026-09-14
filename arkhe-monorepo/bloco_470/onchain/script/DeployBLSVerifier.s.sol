// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import {BLS12_381GovernanceVerifier} from "../contracts/verifiers/BLS12_381Verifier.sol";

/// @title Deploy do verificador BLS12-381 (v15, B1) em Sepolia.
contract DeployBLSVerifier is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");

        vm.startBroadcast(deployerPrivateKey);

        BLS12_381GovernanceVerifier verifier = new BLS12_381GovernanceVerifier();
        console.log("BLS12-381 Verifier deployed at:", address(verifier));

        vm.stopBroadcast();
    }
}