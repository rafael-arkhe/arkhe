// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

interface IFutarchyOutcome {
    function outcome(bytes32 id) external view returns (bool resolved, bool outcomeYes);
}

interface IAlloPool {
    function allocate(address, uint256) external;
    function distribute(address, uint256) external;
    function availableBalance() external view returns (uint256);
}

/// @title FutarchyGovernor — turns a market verdict into real funding.
/// @notice Closes the futarchy loop: a proposal is bound to a FutarchyStrategy
///         market; once that market resolves, `execute` funds the beneficiary
///         from an AlloPool iff the market said YES, and funds nothing on NO.
/// @dev Funding comes from the pool, which is a SEPARATE pot from the market's
///      trader collateral. Trader redemptions (paid out of market collateral) and
///      proposal funding (paid out of the pool) never touch each other, so wiring
///      the decision cannot make the market insolvent.
contract FutarchyGovernor is AccessControl, ReentrancyGuard {
    IFutarchyOutcome public immutable market;
    IAlloPool public immutable pool;

    struct Proposal {
        address beneficiary;
        uint256 fundingAmount;
        bool exists;
        bool executed;
        bool funded;
    }

    mapping(bytes32 => Proposal) public proposals;

    event Proposed(bytes32 indexed id, address beneficiary, uint256 fundingAmount);
    event Executed(bytes32 indexed id, bool funded, uint256 amount);

    constructor(address _market, address _pool) {
        market = IFutarchyOutcome(_market);
        pool = IAlloPool(_pool);
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }

    function propose(bytes32 id, address beneficiary, uint256 fundingAmount) external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(!proposals[id].exists, "exists");
        require(beneficiary != address(0), "beneficiary");
        require(fundingAmount > 0, "amount");
        proposals[id] = Proposal(beneficiary, fundingAmount, true, false, false);
        emit Proposed(id, beneficiary, fundingAmount);
    }

    /// @notice Enact the market's decision. YES → fund the beneficiary; NO → nothing.
    ///         Callable by anyone once the market is resolved (the verdict is the gate).
    function execute(bytes32 id) external nonReentrant {
        Proposal storage p = proposals[id];
        require(p.exists, "no proposal");
        require(!p.executed, "executed");
        (bool resolved, bool outcomeYes) = market.outcome(id);
        require(resolved, "not resolved");

        p.executed = true;
        if (outcomeYes) {
            require(pool.availableBalance() >= p.fundingAmount, "pool underfunded");
            p.funded = true;
            pool.allocate(p.beneficiary, p.fundingAmount);
            pool.distribute(p.beneficiary, p.fundingAmount);
        }
        emit Executed(id, p.funded, p.funded ? p.fundingAmount : 0);
    }
}
