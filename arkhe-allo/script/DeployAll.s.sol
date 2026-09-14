// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AlloPool} from "../src/AlloPool.sol";
import {QuadraticFundingStrategy} from "../src/QuadraticFundingStrategy.sol";
import {DedicatedDomainAllocation} from "../src/DedicatedDomainAllocation.sol";
import {RetroFundingStrategy} from "../src/RetroFundingStrategy.sol";
import {FutarchyStrategy} from "../src/FutarchyStrategy.sol";
import {FutarchyGovernor} from "../src/FutarchyGovernor.sol";
import {DirectToContractIncentives} from "../src/DirectToContractIncentives.sol";
import {CookieJar} from "../src/CookieJar.sol";

interface Vm {
    function startBroadcast() external;
    function stopBroadcast() external;
}

/// @title DeployAll — deploys and wires the whole Allo.Capital suite.
/// @notice Each strategy gets its OWN AlloPool, because a pool grants STRATEGY_ROLE
///         to a single strategy at a time. Addresses are stored in public fields so
///         they show up in `forge script` output / traces.
///
/// Run (simulation):   forge script script/DeployAll.s.sol
/// Run (broadcast):    forge script script/DeployAll.s.sol --rpc-url <URL> --broadcast
contract DeployAll {
    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);

    AlloPool public qfPool;
    QuadraticFundingStrategy public qf;
    AlloPool public ddaPool;
    DedicatedDomainAllocation public dda;
    AlloPool public retroPool;
    RetroFundingStrategy public retro;
    AlloPool public incPool;
    DirectToContractIncentives public incentives;
    AlloPool public jarPool;
    CookieJar public cookieJar;
    FutarchyStrategy public futarchy;
    AlloPool public govPool;
    FutarchyGovernor public governor;

    function run() external {
        address deployer = msg.sender;
        address oracle = msg.sender; // replace with the real oracle key in production

        vm.startBroadcast();

        // Quadratic Funding
        qfPool = new AlloPool(deployer);
        qf = new QuadraticFundingStrategy(address(qfPool), 7 days);
        qfPool.setStrategy(address(qf));

        // Dedicated Domain Allocation
        ddaPool = new AlloPool(deployer);
        dda = new DedicatedDomainAllocation(address(ddaPool));
        ddaPool.setStrategy(address(dda));

        // Retro Funding
        retroPool = new AlloPool(deployer);
        retro = new RetroFundingStrategy(address(retroPool));
        retroPool.setStrategy(address(retro));

        // Incentives (oracle-signature gated)
        incPool = new AlloPool(deployer);
        incentives = new DirectToContractIncentives(address(incPool), oracle);
        incPool.setStrategy(address(incentives));

        // Cookie Jar
        jarPool = new AlloPool(deployer);
        cookieJar = new CookieJar(address(jarPool), 10 ether, 1 days);
        jarPool.setStrategy(address(cookieJar));

        // Futarchy market + governor (governor is the pool's strategy)
        futarchy = new FutarchyStrategy();
        govPool = new AlloPool(deployer);
        governor = new FutarchyGovernor(address(futarchy), address(govPool));
        govPool.setStrategy(address(governor));

        vm.stopBroadcast();
    }
}
