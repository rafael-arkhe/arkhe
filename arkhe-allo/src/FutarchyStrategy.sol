// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @title FutarchyStrategy — binary decision market with a constant-product AMM.
/// @notice Omen/Gnosis-style fixed-product market maker (FPMM):
///   * A buy mints a "complete set" (equal YES+NO into the pool) from the ETH in,
///     then swaps within the pool to keep reserveYes*reserveNo == k invariant.
///   * Marginal price therefore RISES as you buy one side (convex cost) — a 1 wei
///     last-second buy can no longer flip the outcome (fixes the 1:1 exploit).
///   * On resolution, winning shares redeem 1:1 for collateral; losing shares are
///     worth 0. The market is fully collateralized because the initial liquidity
///     is seeded with real ETH at creation.
///
/// Instantaneous YES price = reserveNo / (reserveYes + reserveNo).
contract FutarchyStrategy is AccessControl, ReentrancyGuard {
    bytes32 public constant MARKET_MAKER_ROLE = keccak256("MARKET_MAKER_ROLE");

    struct Market {
        uint256 reserveYes; // AMM reserve (also = YES tokens held by the pool)
        uint256 reserveNo;
        uint256 k; // constant product invariant
        uint256 collateral; // total ETH backing all outstanding shares
        uint256 resolutionTime;
        bool exists;
        bool resolved;
        bool outcomeYes;
        mapping(address => uint256) yesBal;
        mapping(address => uint256) noBal;
    }

    mapping(bytes32 => Market) private markets;
    bytes32[] public marketIds;

    event MarketCreated(bytes32 indexed id, uint256 liquidity, uint256 resolutionTime);
    event Bought(bytes32 indexed id, address indexed trader, bool yes, uint256 ethIn, uint256 sharesOut);
    event Resolved(bytes32 indexed id, bool outcomeYes);
    event Redeemed(bytes32 indexed id, address indexed trader, uint256 payout);

    constructor() {
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(MARKET_MAKER_ROLE, msg.sender);
    }

    /// @notice Create a market seeded with real ETH liquidity (keeps it solvent).
    function createMarket(bytes32 id, uint256 duration) external payable onlyRole(MARKET_MAKER_ROLE) {
        require(!markets[id].exists, "exists");
        require(msg.value > 0, "need liquidity");
        Market storage m = markets[id];
        m.exists = true;
        m.reserveYes = msg.value;
        m.reserveNo = msg.value;
        m.k = msg.value * msg.value;
        m.collateral = msg.value;
        m.resolutionTime = block.timestamp + duration;
        marketIds.push(id);
        emit MarketCreated(id, msg.value, m.resolutionTime);
    }

    /// @notice Pure quote of shares out for a given buy (for tests / UIs).
    function quoteBuy(bytes32 id, bool yes, uint256 ethIn) public view returns (uint256) {
        Market storage m = markets[id];
        uint256 ry = m.reserveYes + ethIn; // complete-set mint
        uint256 rn = m.reserveNo + ethIn;
        if (yes) {
            uint256 newYes = m.k / rn; // restore invariant on the YES side
            return ry - newYes;
        } else {
            uint256 newNo = m.k / ry;
            return rn - newNo;
        }
    }

    function buy(bytes32 id, bool yes, uint256 ethIn) external payable nonReentrant {
        Market storage m = markets[id];
        require(m.exists, "no market");
        require(!m.resolved, "resolved");
        require(block.timestamp < m.resolutionTime, "closed");
        require(ethIn > 0 && msg.value == ethIn, "bad value");

        m.reserveYes += ethIn; // mint complete set into the pool
        m.reserveNo += ethIn;
        m.collateral += ethIn;

        uint256 sharesOut;
        if (yes) {
            uint256 newYes = m.k / m.reserveNo; // keep reserveYes*reserveNo == k
            sharesOut = m.reserveYes - newYes;
            m.reserveYes = newYes;
            m.yesBal[msg.sender] += sharesOut;
        } else {
            uint256 newNo = m.k / m.reserveYes;
            sharesOut = m.reserveNo - newNo;
            m.reserveNo = newNo;
            m.noBal[msg.sender] += sharesOut;
        }
        require(sharesOut > 0, "dust");
        emit Bought(id, msg.sender, yes, ethIn, sharesOut);
    }

    function resolve(bytes32 id, bool outcomeYes) external onlyRole(MARKET_MAKER_ROLE) {
        Market storage m = markets[id];
        require(m.exists && !m.resolved, "bad state");
        require(block.timestamp >= m.resolutionTime, "too early");
        m.resolved = true;
        m.outcomeYes = outcomeYes;
        emit Resolved(id, outcomeYes);
    }

    /// @notice Winning shares pay 1:1 in ETH; losing shares pay 0. One-shot.
    function redeem(bytes32 id) external nonReentrant {
        Market storage m = markets[id];
        require(m.resolved, "not resolved");
        uint256 payout;
        if (m.outcomeYes) {
            payout = m.yesBal[msg.sender];
            m.yesBal[msg.sender] = 0;
            m.noBal[msg.sender] = 0;
        } else {
            payout = m.noBal[msg.sender];
            m.noBal[msg.sender] = 0;
            m.yesBal[msg.sender] = 0;
        }
        require(payout > 0, "nothing");
        require(payout <= m.collateral, "insolvent"); // invariant guard
        m.collateral -= payout;
        (bool ok,) = payable(msg.sender).call{value: payout}("");
        require(ok, "xfer");
        emit Redeemed(id, msg.sender, payout);
    }

    // --- views ---
    function reserves(bytes32 id) external view returns (uint256 yes, uint256 no) {
        return (markets[id].reserveYes, markets[id].reserveNo);
    }

    /// @notice YES price in 1e18 fixed point = reserveNo/(reserveYes+reserveNo).
    function yesPrice(bytes32 id) external view returns (uint256) {
        Market storage m = markets[id];
        return (m.reserveNo * 1e18) / (m.reserveYes + m.reserveNo);
    }

    function balances(bytes32 id, address who) external view returns (uint256 yes, uint256 no) {
        return (markets[id].yesBal[who], markets[id].noBal[who]);
    }

    function collateralOf(bytes32 id) external view returns (uint256) {
        return markets[id].collateral;
    }

    /// @notice Resolved state + verdict, for a governor to act on.
    function outcome(bytes32 id) external view returns (bool resolved, bool outcomeYes) {
        Market storage m = markets[id];
        return (m.resolved, m.outcomeYes);
    }
}
