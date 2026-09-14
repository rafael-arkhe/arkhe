// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

interface IAlloPool {
    function deposit() external payable;
    function allocate(address, uint256) external;
    function distribute(address, uint256) external;
    function availableBalance() external view returns (uint256);
}

/// @title DedicatedDomainAllocation — per-domain budgets managed by stewards.
/// @notice Fixes the original bug where `fundDomain` accepted ETH but never
///         forwarded it to the pool (so allocations reverted). Here funding
///         forwards into the pool, and each steward can spend only up to their
///         own domain's funded budget — even though all domains share one pool.
contract DedicatedDomainAllocation is AccessControl, ReentrancyGuard {
    IAlloPool public immutable pool;

    struct Domain {
        address steward;
        uint256 budget; // total ETH funded to this domain
        uint256 spent; // ETH already allocated out
        bool active;
    }

    mapping(bytes32 => Domain) public domains;
    mapping(bytes32 => mapping(address => bool)) public approved;
    bytes32[] public domainIds;

    event DomainCreated(bytes32 indexed id, address indexed steward);
    event DomainFunded(bytes32 indexed id, address indexed from, uint256 amount);
    event RecipientApproved(bytes32 indexed id, address indexed recipient);
    event Allocated(bytes32 indexed id, address indexed recipient, uint256 amount);

    constructor(address _pool) {
        pool = IAlloPool(_pool);
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }

    modifier onlySteward(bytes32 id) {
        require(domains[id].steward == msg.sender, "not steward");
        _;
    }

    function createDomain(bytes32 id, address steward) external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(domains[id].steward == address(0), "exists");
        require(steward != address(0), "steward");
        domains[id] = Domain(steward, 0, 0, true);
        domainIds.push(id);
        emit DomainCreated(id, steward);
    }

    /// @notice Fund a domain; ETH is forwarded into the pool so allocations are backed.
    function fundDomain(bytes32 id) external payable {
        Domain storage d = domains[id];
        require(d.active, "inactive");
        require(msg.value > 0, "zero");
        d.budget += msg.value;
        pool.deposit{value: msg.value}();
        emit DomainFunded(id, msg.sender, msg.value);
    }

    function approveRecipient(bytes32 id, address recipient) external onlySteward(id) {
        require(recipient != address(0), "recipient");
        approved[id][recipient] = true;
        emit RecipientApproved(id, recipient);
    }

    /// @notice Allocate from a domain's budget to an approved recipient, then pay.
    function allocateToRecipient(bytes32 id, address recipient, uint256 amount) external onlySteward(id) nonReentrant {
        Domain storage d = domains[id];
        require(d.active, "inactive");
        require(approved[id][recipient], "not approved");
        require(amount > 0, "amount");
        require(d.spent + amount <= d.budget, "over budget"); // per-domain cap

        d.spent += amount;
        pool.allocate(recipient, amount);
        pool.distribute(recipient, amount);
        emit Allocated(id, recipient, amount);
    }

    function setActive(bytes32 id, bool active) external onlyRole(DEFAULT_ADMIN_ROLE) {
        domains[id].active = active;
    }

    function budgetOf(bytes32 id) external view returns (uint256 budget, uint256 spent) {
        Domain storage d = domains[id];
        return (d.budget, d.spent);
    }
}
