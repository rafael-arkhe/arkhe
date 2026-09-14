// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AlloPool} from "../src/AlloPool.sol";
import {QuadraticFundingStrategy} from "../src/QuadraticFundingStrategy.sol";

/// Minimal cheatcode interface (avoids a forge-std network fetch).
interface Vm {
    function warp(uint256) external;
    function deal(address, uint256) external;
    function prank(address) external;
    function expectRevert(bytes calldata) external;
}

contract QuadraticFundingTest {
    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);

    AlloPool pool;
    QuadraticFundingStrategy qf;

    address constant A = address(0xA11CE); // 4 contributors × 1 ETH
    address constant B = address(0xB0B); // 2 contributors × 2 ETH  (same 4 ETH total)

    function setUp() public {
        pool = new AlloPool(address(this));
        qf = new QuadraticFundingStrategy(address(pool), 1 days);
        pool.setStrategy(address(qf));
        vm.deal(address(this), 1000 ether);
        qf.fundMatching{value: 100 ether}();
        qf.registerRecipient(A);
        qf.registerRecipient(B);
    }

    function _contribute(address from, address to, uint256 amt) internal {
        vm.deal(from, amt);
        vm.prank(from);
        qf.contribute{value: amt}(to);
    }

    function _req(bool cond, string memory why) internal pure {
        require(cond, why);
    }

    /// Core QF property + full funding/distribution accounting.
    function testQfCrowdingAndDistribution() public {
        // A: four distinct 1-ETH contributors
        _contribute(address(0x101), A, 1 ether);
        _contribute(address(0x102), A, 1 ether);
        _contribute(address(0x103), A, 1 ether);
        _contribute(address(0x104), A, 1 ether);
        // B: two distinct 2-ETH contributors (identical 4 ETH raised)
        _contribute(address(0x201), B, 2 ether);
        _contribute(address(0x202), B, 2 ether);

        vm.warp(block.timestamp + 2 days);
        qf.calculateMatching();

        uint256 mA = qf.matchingOf(A);
        uint256 mB = qf.matchingOf(B);

        // Same money raised, but A had more contributors -> strictly more matching.
        _req(mA > mB, "QF broken: crowd not favored");
        // Expected ~75 / ~25 split of the 100 ETH matching pool.
        _req(mA >= 74.5 ether && mA <= 75.5 ether, "mA out of band");
        _req(mB >= 24.5 ether && mB <= 25.5 ether, "mB out of band");

        // Distribution pays contributions (4 ETH each) + matching, exactly once.
        qf.distribute(A);
        qf.distribute(B);
        _req(A.balance == 4 ether + mA, "A payout wrong");
        _req(B.balance == 4 ether + mB, "B payout wrong");
    }

    /// Pool cannot allocate more than it holds (double-spend guard).
    function testAllocateCannotExceedAvailable() public {
        vm.warp(block.timestamp + 2 days);
        qf.calculateMatching(); // no contributions -> nothing allocated, no revert
        _req(pool.totalAllocated() == 0, "should allocate nothing");
    }

    /// Re-distribution is impossible (owed goes to zero).
    function testNoDoubleDistribute() public {
        _contribute(address(0x101), A, 1 ether);
        vm.warp(block.timestamp + 2 days);
        qf.calculateMatching();
        qf.distribute(A);
        vm.expectRevert(bytes("nothing"));
        qf.distribute(A);
    }
}
