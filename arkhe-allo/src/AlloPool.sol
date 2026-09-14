// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @title AlloPool — capital pool with available-balance accounting (one per strategy)
/// @notice `allocate` only reserves against *unlocked* balance, so it cannot promise
///         more than the pool actually holds. `distribute` cannot exceed what was
///         allocated. Funds reach the pool via `deposit`/`receive`.
contract AlloPool is AccessControl, ReentrancyGuard {
    bytes32 public constant STRATEGY_ROLE = keccak256("STRATEGY_ROLE");

    uint256 public totalAllocated;
    uint256 public totalDistributed;
    address public strategy;
    mapping(address => bool) public isRecipient;

    event Deposit(address indexed from, uint256 amount);
    event Allocated(address indexed recipient, uint256 amount);
    event Distributed(address indexed recipient, uint256 amount);
    event StrategyUpdated(address indexed oldStrategy, address indexed newStrategy);

    constructor(address manager) {
        _grantRole(DEFAULT_ADMIN_ROLE, manager);
    }

    function deposit() external payable {
        require(msg.value > 0, "zero");
        emit Deposit(msg.sender, msg.value);
    }

    receive() external payable {
        emit Deposit(msg.sender, msg.value);
    }

    /// @notice Balance not already reserved by outstanding allocations.
    function availableBalance() public view returns (uint256) {
        return address(this).balance - (totalAllocated - totalDistributed);
    }

    function setStrategy(address newStrategy) external onlyRole(DEFAULT_ADMIN_ROLE) {
        emit StrategyUpdated(strategy, newStrategy);
        if (strategy != address(0)) _revokeRole(STRATEGY_ROLE, strategy);
        strategy = newStrategy;
        if (newStrategy != address(0)) _grantRole(STRATEGY_ROLE, newStrategy);
    }

    function allocate(address recipient, uint256 amount) external onlyRole(STRATEGY_ROLE) {
        require(recipient != address(0), "recipient");
        require(amount > 0, "amount");
        require(amount <= availableBalance(), "insufficient available"); // no double-spend
        isRecipient[recipient] = true;
        totalAllocated += amount;
        emit Allocated(recipient, amount);
    }

    function distribute(address recipient, uint256 amount) external onlyRole(STRATEGY_ROLE) nonReentrant {
        require(isRecipient[recipient], "not recipient");
        require(amount > 0, "amount");
        require(totalDistributed + amount <= totalAllocated, "over-allocated");
        totalDistributed += amount;
        (bool ok,) = payable(recipient).call{value: amount}("");
        require(ok, "xfer");
        emit Distributed(recipient, amount);
    }
}
