// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import {STARKVerifier} from "../contracts/verifiers/STARKVerifier.sol";

/// @title Deploy do STARKVerifier (v15, S1) — contrato gerado pelo stone-cli.
contract DeploySTARKVerifier is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");

        vm.startBroadcast(deployerPrivateKey);

        STARKVerifier verifier = new STARKVerifier();
        console.log("STARK Verifier deployed at:", address(verifier));

        vm.stopBroadcast();
    }
}