// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {Math} from "@openzeppelin/contracts/utils/math/Math.sol";

interface IAlloPool {
    function deposit() external payable;
    function allocate(address recipient, uint256 amount) external;
    function distribute(address recipient, uint256 amount) external;
    function availableBalance() external view returns (uint256);
}

/// @title QuadraticFundingStrategy — REAL QF: matchRaw_i = (Σ√c_ij)² − Σc_ij
/// @notice Per-contribution square roots are accumulated as they arrive, so the
///         crowd-favoring QF signal is preserved. Contributions AND the matching
///         pool are held in the AlloPool; both are allocated then distributed, so
///         nothing is stranded.
contract QuadraticFundingStrategy is AccessControl, ReentrancyGuard {
    IAlloPool public immutable pool;
    uint256 public matchingPool; // matching ETH, held inside the pool
    uint256 public roundEnd;
    bool public isFinalized;

    struct RecipientData {
        uint256 totalContributions; // Σ c_ij (wei)
        uint256 sumSqrt; // Σ √c_ij (integer sqrt of each contribution)
        uint256 matchingAllocated;
        uint256 distributed;
        bool registered;
    }

    mapping(address => RecipientData) public data;
    address[] public recipients;

    event Contributed(address indexed recipient, address indexed from, uint256 amount);
    event MatchingCalculated(address indexed recipient, uint256 matchRaw, uint256 matchFinal);
    event Finalized(uint256 matchingPool, uint256 totalMatchRaw);
    event Distributed(address indexed recipient, uint256 amount);

    constructor(address _pool, uint256 _duration) {
        pool = IAlloPool(_pool);
        roundEnd = block.timestamp + _duration;
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }

    function registerRecipient(address r) external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(!data[r].registered, "exists");
        data[r].registered = true;
        recipients.push(r);
    }

    /// @notice Fund matching; ETH is forwarded into the pool so allocations are backed.
    function fundMatching() external payable onlyRole(DEFAULT_ADMIN_ROLE) {
        require(msg.value > 0, "zero");
        matchingPool += msg.value;
        pool.deposit{value: msg.value}();
    }

    /// @notice Contribute to a recipient; ETH forwarded to the pool; Σ√c updated.
    function contribute(address recipient) external payable nonReentrant {
        require(block.timestamp <= roundEnd, "round ended");
        require(msg.value > 0, "zero");
        require(data[recipient].registered, "not registered");
        RecipientData storage d = data[recipient];
        d.totalContributions += msg.value;
        d.sumSqrt += Math.sqrt(msg.value); // the QF signal: per-contribution sqrt
        pool.deposit{value: msg.value}();
        emit Contributed(recipient, msg.sender, msg.value);
    }

    /// @notice True QF matching, scaled proportionally to the matching pool, then
    ///         each recipient's own contributions are allocated too.
    function calculateMatching() external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(block.timestamp > roundEnd, "not ended");
        require(!isFinalized, "finalized");

        uint256 n = recipients.length;
        uint256[] memory raw = new uint256[](n);
        uint256 totalRaw;
        for (uint256 i = 0; i < n; i++) {
            RecipientData storage d = data[recipients[i]];
            uint256 sq = d.sumSqrt * d.sumSqrt; // (Σ√c)²
            uint256 r = sq > d.totalContributions ? sq - d.totalContributions : 0;
            raw[i] = r;
            totalRaw += r;
        }

        for (uint256 i = 0; i < n; i++) {
            RecipientData storage d = data[recipients[i]];
            if (totalRaw > 0 && raw[i] > 0) {
                uint256 m = (matchingPool * raw[i]) / totalRaw; // scale to real pool
                d.matchingAllocated = m;
                pool.allocate(recipients[i], m);
                emit MatchingCalculated(recipients[i], raw[i], m);
            }
            if (d.totalContributions > 0) {
                pool.allocate(recipients[i], d.totalContributions); // back their own funds
            }
        }

        isFinalized = true;
        emit Finalized(matchingPool, totalRaw);
    }

    /// @notice Pull payment: contributions + matching, once.
    function distribute(address recipient) external nonReentrant {
        require(isFinalized, "not finalized");
        RecipientData storage d = data[recipient];
        uint256 owed = d.totalContributions + d.matchingAllocated - d.distributed;
        require(owed > 0, "nothing");
        d.distributed += owed;
        pool.distribute(recipient, owed);
        emit Distributed(recipient, owed);
    }

    function matchingOf(address r) external view returns (uint256) {
        return data[r].matchingAllocated;
    }
}
