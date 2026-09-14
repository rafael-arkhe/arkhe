// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {FutarchyStrategy} from "../src/FutarchyStrategy.sol";

interface Vm {
    function warp(uint256) external;
    function deal(address, uint256) external;
    function prank(address) external;
    function expectRevert(bytes calldata) external;
}

contract FutarchyTest {
    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);

    FutarchyStrategy fut;
    bytes32 constant M = keccak256("proposal-1");

    function setUp() public {
        fut = new FutarchyStrategy();
        vm.deal(address(this), 10_000 ether);
        fut.createMarket{value: 100 ether}(M, 1 days);
    }

    function _buy(address who, bool yes, uint256 amt) internal returns (uint256 sharesOut) {
        vm.deal(who, amt);
        (uint256 y0, uint256 n0) = fut.balances(M, who);
        vm.prank(who);
        fut.buy{value: amt}(M, yes, amt);
        (uint256 y1, uint256 n1) = fut.balances(M, who);
        sharesOut = yes ? (y1 - y0) : (n1 - n0);
    }

    function _req(bool c, string memory why) internal pure {
        require(c, why);
    }

    /// CONVEXITY: two identical YES buys — the second yields strictly fewer shares.
    /// Under a 1:1 exchange both would return exactly `amt`, so this assertion fails.
    function testConvexPricingBreaks1to1() public {
        uint256 s1 = _buy(address(0x1), true, 10 ether);
        uint256 s2 = _buy(address(0x2), true, 10 ether);
        _req(s1 > 10 ether, "at 50/50, 10 ETH should buy >10 YES"); // 1:1 gives ==10
        _req(s2 < s1, "price impact missing (1:1 would be equal)"); // the key check
    }

    /// PRICE DISCOVERY: buying YES pushes the YES price above 0.5 (started at 0.5).
    /// Under 1:1 reserves never skew, so price is undefined/constant — assertion fails.
    function testYesBuysMovePriceUp() public {
        uint256 p0 = fut.yesPrice(M);
        _req(p0 == 0.5e18, "should start at 0.5");
        _buy(address(0x3), true, 40 ether);
        uint256 p1 = fut.yesPrice(M);
        _req(p1 > 0.55e18, "big YES buy must raise YES price");
        (uint256 ry, uint256 rn) = fut.reserves(M);
        _req(ry < rn, "YES reserve must shrink relative to NO");
    }

    /// quoteBuy must match the realized buy exactly.
    function testQuoteMatchesBuy() public {
        uint256 q = fut.quoteBuy(M, true, 7 ether);
        uint256 got = _buy(address(0x4), true, 7 ether);
        _req(q == got, "quote != realized");
    }

    /// REDEMPTION: winners get 1:1, losers get 0, contract stays solvent.
    function testRedeemPaysWinnersAndIsSolvent() public {
        uint256 yShares = _buy(address(0xBEEF), true, 10 ether);
        _buy(address(0xCAFE), false, 10 ether);

        vm.warp(block.timestamp + 2 days);
        fut.resolve(M, true); // YES wins

        // YES holder redeems 1:1
        vm.prank(address(0xBEEF));
        fut.redeem(M);
        _req(address(0xBEEF).balance == yShares, "winner payout != shares (1:1)");

        // NO holder gets nothing
        vm.prank(address(0xCAFE));
        vm.expectRevert(bytes("nothing"));
        fut.redeem(M);

        // Solvency: contract still holds at least the unredeemed collateral.
        _req(address(fut).balance == fut.collateralOf(M), "collateral accounting drift");
        _req(address(fut).balance >= 0, "insolvent");
    }

    /// Last-second 1-wei buy cannot meaningfully move the market (the 1:1 exploit).
    function testDustBuyBarelyMovesPrice() public {
        uint256 p0 = fut.yesPrice(M);
        _buy(address(0x9), true, 1);
        uint256 p1 = fut.yesPrice(M);
        // moved by less than 1e-6 of full scale
        _req(p1 - p0 < 1e12, "dust buy moved price too much");
    }
}
