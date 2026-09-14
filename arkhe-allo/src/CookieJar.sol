// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

interface IAlloPool {
    function allocate(address, uint256) external;
    function distribute(address, uint256) external;
    function availableBalance() external view returns (uint256);
}

/// @title CookieJar — grant requests decoupled from pool accounting.
/// @notice Fixes the original bug: `claimGrant` used to immediately call
///         `pool.allocate`, so any never-approved grant permanently locked
///         `totalAllocated` and shrank `availableBalance` forever. Here a claim
///         only RECORDS a request; the pool is touched exclusively on approval,
///         so pending/rejected grants strand nothing.
contract CookieJar is AccessControl, ReentrancyGuard {
    bytes32 public constant TRUSTED_ROLE = keccak256("TRUSTED_ROLE");

    IAlloPool public immutable pool;
    uint256 public maxGrant;
    uint256 public cooldown;

    struct Grant {
        address contributor;
        string description;
        uint256 amount;
        uint256 timestamp;
        bool approved;
        bool distributed;
    }

    mapping(address => Grant[]) public grants;
    mapping(address => uint256) public lastClaim;

    event GrantClaimed(address indexed contributor, uint256 indexed index, uint256 amount);
    event GrantApproved(address indexed contributor, uint256 indexed index, uint256 amount);
    event GrantRejected(address indexed contributor, uint256 indexed index);

    constructor(address _pool, uint256 _maxGrant, uint256 _cooldown) {
        pool = IAlloPool(_pool);
        maxGrant = _maxGrant;
        cooldown = _cooldown;
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(TRUSTED_ROLE, msg.sender);
    }

    /// @notice Records a grant request ONLY. Deliberately touches nothing in the pool.
    function claimGrant(string calldata description, uint256 amount) external returns (uint256 index) {
        require(amount > 0 && amount <= maxGrant, "amount");
        require(block.timestamp >= lastClaim[msg.sender] + cooldown, "cooldown");
        lastClaim[msg.sender] = block.timestamp;
        grants[msg.sender].push(Grant(msg.sender, description, amount, block.timestamp, false, false));
        index = grants[msg.sender].length - 1;
        emit GrantClaimed(msg.sender, index, amount);
    }

    /// @notice Allocation AND distribution happen here, only on approval.
    function approveAndDistribute(address contributor, uint256 index) external onlyRole(TRUSTED_ROLE) nonReentrant {
        Grant storage g = grants[contributor][index];
        require(!g.approved, "approved");
        require(g.amount > 0, "empty");
        require(g.amount <= pool.availableBalance(), "pool underfunded");
        g.approved = true;
        g.distributed = true;
        pool.allocate(contributor, g.amount);
        pool.distribute(contributor, g.amount);
        emit GrantApproved(contributor, index, g.amount);
    }

    /// @notice Rejecting a pending grant strands nothing — the pool was never touched.
    function reject(address contributor, uint256 index) external onlyRole(TRUSTED_ROLE) {
        Grant storage g = grants[contributor][index];
        require(!g.approved, "approved");
        g.amount = 0;
        emit GrantRejected(contributor, index);
    }

    function grantCount(address c) external view returns (uint256) {
        return grants[c].length;
    }

    function grantView(address c, uint256 i) external view returns (uint256 amount, bool approved, bool distributed) {
        Grant storage g = grants[c][i];
        return (g.amount, g.approved, g.distributed);
    }
}
