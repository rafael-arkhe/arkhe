// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AlloPool} from "../src/AlloPool.sol";
import {CookieJar} from "../src/CookieJar.sol";

interface Vm {
    function deal(address, uint256) external;
    function prank(address) external;
    function warp(uint256) external;
    function expectRevert(bytes calldata) external;
}

contract CookieJarTest {
    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);

    AlloPool pool;
    CookieJar jar;
    address constant ALICE = address(0xA11CE);

    function setUp() public {
        vm.warp(1_700_000_000); // realistic epoch so cooldown math isn't near t=0
        pool = new AlloPool(address(this));
        jar = new CookieJar(address(pool), 50 ether, 1 days);
        pool.setStrategy(address(jar));
        vm.deal(address(this), 1000 ether);
        pool.deposit{value: 100 ether}();
    }

    function _req(bool c, string memory why) internal pure {
        require(c, why);
    }

    function _claim(address who, uint256 amount) internal returns (uint256) {
        vm.prank(who);
        return jar.claimGrant("work", amount);
    }

    /// THE FIX: a pending (unapproved) grant does NOT lock pool accounting.
    function testPendingGrantDoesNotLockPool() public {
        _claim(ALICE, 40 ether);
        _req(pool.availableBalance() == 100 ether, "pending grant locked the pool");
        _req(pool.totalAllocated() == 0, "nothing should be allocated yet");
    }

    /// Approval is what allocates + distributes; funds only move then.
    function testApproveFreesAndPays() public {
        uint256 idx = _claim(ALICE, 40 ether);
        jar.approveAndDistribute(ALICE, idx);
        _req(ALICE.balance == 40 ether, "contributor not paid on approval");
        _req(pool.availableBalance() == 60 ether, "available not reduced by paid grant");
        (, bool approved, bool distributed) = jar.grantView(ALICE, idx);
        _req(approved && distributed, "grant flags wrong");
    }

    /// A rejected grant strands nothing and pays nothing.
    function testRejectStrandsNothing() public {
        uint256 idx = _claim(ALICE, 40 ether);
        jar.reject(ALICE, idx);
        _req(pool.availableBalance() == 100 ether, "reject should not touch pool");
        vm.expectRevert(bytes("empty"));
        jar.approveAndDistribute(ALICE, idx);
    }

    function testNoDoubleApprove() public {
        uint256 idx = _claim(ALICE, 40 ether);
        jar.approveAndDistribute(ALICE, idx);
        vm.expectRevert(bytes("approved"));
        jar.approveAndDistribute(ALICE, idx);
    }

    function testCooldownEnforced() public {
        _claim(ALICE, 10 ether);
        vm.prank(ALICE);
        vm.expectRevert(bytes("cooldown"));
        jar.claimGrant("again", 10 ether);
        // after cooldown, a second claim works
        vm.warp(block.timestamp + 2 days);
        uint256 idx2 = _claim(ALICE, 10 ether);
        _req(idx2 == 1, "second grant index wrong");
    }

    function testMaxGrantEnforced() public {
        vm.prank(ALICE);
        vm.expectRevert(bytes("amount"));
        jar.claimGrant("greedy", 51 ether); // > 50 maxGrant
    }
}
