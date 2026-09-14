// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AlloPool} from "../src/AlloPool.sol";

interface Vm {
    function deal(address, uint256) external;
}

/// Fuzz the pool accounting identity:  totalDistributed <= totalAllocated <= deposits.
/// (Foundry-native fuzzing; no forge-std dependency, so the project stays standalone.)
contract PoolInvariantTest {
    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);

    AlloPool pool;
    uint256 deposits; // ground-truth sum of everything deposited
    address constant RCPT = address(0xBEEF);

    function setUp() public {
        pool = new AlloPool(address(this));
        pool.setStrategy(address(this)); // test acts as the strategy
    }

    function _bound(uint256 x, uint256 lo, uint256 hi) internal pure returns (uint256) {
        return lo + (x % (hi - lo + 1));
    }

    function _check() internal view {
        require(pool.totalDistributed() <= pool.totalAllocated(), "invariant: D <= A");
        require(pool.totalAllocated() <= deposits, "invariant: A <= deposits");
        // availableBalance never underflows (would revert on the subtraction otherwise)
        pool.availableBalance();
    }

    /// One deposit/allocate/distribute cycle over fuzzed amounts.
    function testFuzz_singleCycle(uint96 d, uint96 a, uint96 dist) public {
        uint256 dep = _bound(d, 1, 1e27);
        vm.deal(address(this), dep);
        pool.deposit{value: dep}();
        deposits += dep;
        _check();

        uint256 alloc = _bound(a, 0, dep); // at most available
        if (alloc > 0) pool.allocate(RCPT, alloc);
        _check();

        uint256 payout = _bound(dist, 0, alloc); // at most allocated
        if (payout > 0) pool.distribute(RCPT, payout);
        _check();
    }

    /// Many interleaved rounds; the invariant must hold after every operation.
    function testFuzz_manyRounds(uint96[16] calldata xs) public {
        for (uint256 i = 0; i + 2 < xs.length; i += 3) {
            uint256 dep = _bound(xs[i], 1, 1e24);
            vm.deal(address(this), dep);
            pool.deposit{value: dep}();
            deposits += dep;

            uint256 avail = pool.availableBalance();
            uint256 alloc = _bound(xs[i + 1], 0, avail);
            if (alloc > 0) pool.allocate(RCPT, alloc);

            uint256 headroom = pool.totalAllocated() - pool.totalDistributed();
            uint256 payout = _bound(xs[i + 2], 0, headroom);
            if (payout > 0) pool.distribute(RCPT, payout);

            _check();
        }
    }
}
