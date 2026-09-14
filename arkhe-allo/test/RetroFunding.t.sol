// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AlloPool} from "../src/AlloPool.sol";
import {RetroFundingStrategy} from "../src/RetroFundingStrategy.sol";

interface Vm {
    function deal(address, uint256) external;
    function prank(address) external;
    function expectRevert() external;
    function expectRevert(bytes calldata) external;
}

contract RetroFundingTest {
    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);

    AlloPool pool;
    RetroFundingStrategy retro;

    address constant P1 = address(0xF1);
    address constant P2 = address(0xF2);

    function setUp() public {
        pool = new AlloPool(address(this));
        retro = new RetroFundingStrategy(address(pool));
        pool.setStrategy(address(retro));
        vm.deal(address(this), 1000 ether);
        retro.fund{value: 100 ether}();
        retro.registerProject(P1);
        retro.registerProject(P2);
    }

    function _req(bool c, string memory why) internal pure {
        require(c, why);
    }

    function _evaluator(uint160 i) internal pure returns (address) {
        return address(uint160(0xE000) + i);
    }

    /// THE FIX: FIVE evaluators — more than the original hard cap of 3 — all
    /// participate. Under the buggy design the 4th evaluator was locked out.
    function testMoreThanThreeEvaluatorsAllScore() public {
        for (uint160 i = 0; i < 5; i++) {
            retro.addEvaluator(_evaluator(i));
        }
        _req(retro.evaluatorCount() == 5, "should have 5 evaluators");

        // Every one of the 5 can score both projects (no lock-out for #4, #5).
        for (uint160 i = 0; i < 5; i++) {
            vm.prank(_evaluator(i));
            retro.submitScore(P1, 80);
            vm.prank(_evaluator(i));
            retro.submitScore(P2, 20);
        }
        _req(retro.totalScore(P1) == 400, "P1 score (5*80)");
        _req(retro.totalScore(P2) == 100, "P2 score (5*20)");
    }

    /// Payout is proportional to total score, then claimable.
    function testProportionalPayoutAndClaim() public {
        for (uint160 i = 0; i < 5; i++) {
            retro.addEvaluator(_evaluator(i));
            vm.prank(_evaluator(i));
            retro.submitScore(P1, 80);
            vm.prank(_evaluator(i));
            retro.submitScore(P2, 20);
        }
        retro.finalize();
        // 400:100 of 100 ETH -> 80 / 20
        _req(retro.allocated(P1) == 80 ether, "P1 alloc");
        _req(retro.allocated(P2) == 20 ether, "P2 alloc");

        retro.claim(P1);
        retro.claim(P2);
        _req(P1.balance == 80 ether, "P1 payout");
        _req(P2.balance == 20 ether, "P2 payout");
    }

    /// An evaluator cannot score the same project twice.
    function testDoubleScoreReverts() public {
        retro.addEvaluator(_evaluator(0));
        vm.prank(_evaluator(0));
        retro.submitScore(P1, 50);
        vm.prank(_evaluator(0));
        vm.expectRevert(bytes("scored"));
        retro.submitScore(P1, 50);
    }

    /// A non-evaluator cannot submit scores (OZ AccessControl custom-error revert).
    function testNonEvaluatorCannotScore() public {
        vm.prank(address(0xDEAD));
        vm.expectRevert();
        retro.submitScore(P1, 50);
    }

    /// No double claim.
    function testNoDoubleClaim() public {
        retro.addEvaluator(_evaluator(0));
        vm.prank(_evaluator(0));
        retro.submitScore(P1, 50);
        retro.finalize();
        retro.claim(P1);
        vm.expectRevert(bytes("nothing"));
        retro.claim(P1);
    }
}
