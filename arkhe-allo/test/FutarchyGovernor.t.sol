// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AlloPool} from "../src/AlloPool.sol";
import {FutarchyStrategy} from "../src/FutarchyStrategy.sol";
import {FutarchyGovernor} from "../src/FutarchyGovernor.sol";

interface Vm {
    function deal(address, uint256) external;
    function prank(address) external;
    function warp(uint256) external;
    function expectRevert(bytes calldata) external;
}

contract FutarchyGovernorTest {
    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);

    AlloPool pool;
    FutarchyStrategy fut;
    FutarchyGovernor gov;

    bytes32 constant M = keccak256("fund-audit");
    address constant BEN = address(0xBE);

    function setUp() public {
        fut = new FutarchyStrategy();
        pool = new AlloPool(address(this));
        gov = new FutarchyGovernor(address(fut), address(pool));
        pool.setStrategy(address(gov)); // governor is the pool's strategy
        vm.deal(address(this), 10_000 ether);
        pool.deposit{value: 50 ether}(); // proposal funding budget (separate pot)
        fut.createMarket{value: 100 ether}(M, 1 days); // market liquidity (separate pot)
        gov.propose(M, BEN, 50 ether);
    }

    function _req(bool c, string memory why) internal pure {
        require(c, why);
    }

    /// YES verdict funds the beneficiary from the pool.
    function testYesVerdictFundsBeneficiary() public {
        vm.warp(block.timestamp + 2 days);
        fut.resolve(M, true);
        gov.execute(M);
        _req(BEN.balance == 50 ether, "beneficiary not funded on YES");
        _req(pool.availableBalance() == 0, "pool not drained by funding");
    }

    /// NO verdict funds nothing; the pool is untouched.
    function testNoVerdictFundsNothing() public {
        vm.warp(block.timestamp + 2 days);
        fut.resolve(M, false);
        gov.execute(M);
        _req(BEN.balance == 0, "beneficiary funded on NO");
        _req(pool.availableBalance() == 50 ether, "pool touched on NO");
    }

    /// Cannot execute before the market resolves.
    function testCannotExecuteBeforeResolution() public {
        vm.expectRevert(bytes("not resolved"));
        gov.execute(M);
    }

    /// Execution is one-shot.
    function testNoDoubleExecute() public {
        vm.warp(block.timestamp + 2 days);
        fut.resolve(M, true);
        gov.execute(M);
        vm.expectRevert(bytes("executed"));
        gov.execute(M);
    }

    /// END-TO-END: a trader redeems from market collateral AND the beneficiary is
    /// funded from the pool — the two pots are independent, both stay solvent.
    function testTraderRedemptionAndFundingAreIndependent() public {
        address trader = address(0x77);
        vm.deal(trader, 10 ether);
        vm.prank(trader);
        fut.buy{value: 10 ether}(M, true, 10 ether);
        (uint256 yShares,) = fut.balances(M, trader);

        vm.warp(block.timestamp + 2 days);
        fut.resolve(M, true);

        // proposal funding (from pool)
        gov.execute(M);
        _req(BEN.balance == 50 ether, "beneficiary not funded");

        // trader redemption (from market collateral) — unaffected by the funding
        vm.prank(trader);
        fut.redeem(M);
        _req(trader.balance == yShares, "trader redemption wrong");

        // both pots settled independently
        _req(pool.availableBalance() == 0, "pool funding not settled");
        _req(address(fut).balance == fut.collateralOf(M), "market collateral drift");
    }
}
