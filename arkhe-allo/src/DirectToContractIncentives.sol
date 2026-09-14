// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {ECDSA} from "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import {MessageHashUtils} from "@openzeppelin/contracts/utils/cryptography/MessageHashUtils.sol";

interface IAlloPool {
    function allocate(address, uint256) external;
    function distribute(address, uint256) external;
    function availableBalance() external view returns (uint256);
}

/// @title DirectToContractIncentives — rewards gated by an off-chain oracle signature.
/// @notice There is NO open claim path. Every claim must carry an oracle signature
///         over (claimant, ruleId, units, nonce, address(this), chainid). This fixes
///         the original bug #6, where a public `claimReward(units)` let anyone drain
///         the budget. Per-user nonces prevent signature replay; the domain binding
///         (this + chainid) prevents cross-contract / cross-chain replay.
contract DirectToContractIncentives is AccessControl, ReentrancyGuard {
    using ECDSA for bytes32;
    using MessageHashUtils for bytes32;

    IAlloPool public immutable pool;
    address public oracle;

    struct Rule {
        uint256 rewardPerUnit;
        uint256 maxReward;
        uint256 totalRewarded;
        bool active;
    }

    mapping(bytes32 => Rule) public rules;
    mapping(address => mapping(uint256 => bool)) public usedNonce;

    event RuleCreated(bytes32 indexed ruleId, uint256 rewardPerUnit, uint256 maxReward);
    event Claimed(bytes32 indexed ruleId, address indexed claimant, uint256 units, uint256 reward);

    constructor(address _pool, address _oracle) {
        require(_oracle != address(0), "oracle");
        pool = IAlloPool(_pool);
        oracle = _oracle;
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }

    function setOracle(address _oracle) external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(_oracle != address(0), "oracle");
        oracle = _oracle;
    }

    function createRule(bytes32 ruleId, uint256 rewardPerUnit, uint256 maxReward)
        external
        onlyRole(DEFAULT_ADMIN_ROLE)
    {
        require(!rules[ruleId].active, "exists");
        require(rewardPerUnit > 0, "rate");
        rules[ruleId] = Rule(rewardPerUnit, maxReward, 0, true);
        emit RuleCreated(ruleId, rewardPerUnit, maxReward);
    }

    /// @notice The only claim path. `signature` must be from `oracle` over the digest.
    function claim(bytes32 ruleId, uint256 units, uint256 nonce, bytes calldata signature) external nonReentrant {
        Rule storage r = rules[ruleId];
        require(r.active, "inactive");
        require(units > 0, "units");
        require(!usedNonce[msg.sender][nonce], "nonce used");

        bytes32 h = keccak256(abi.encodePacked(msg.sender, ruleId, units, nonce, address(this), block.chainid));
        address signer = h.toEthSignedMessageHash().recover(signature);
        require(signer == oracle, "bad signature");

        uint256 reward = units * r.rewardPerUnit;
        require(r.totalRewarded + reward <= r.maxReward, "cap");

        // checks-effects-interactions
        usedNonce[msg.sender][nonce] = true;
        r.totalRewarded += reward;

        pool.allocate(msg.sender, reward);
        pool.distribute(msg.sender, reward);
        emit Claimed(ruleId, msg.sender, units, reward);
    }

    /// @notice Helper so off-chain oracles and tests build the identical digest.
    function digestFor(address claimant, bytes32 ruleId, uint256 units, uint256 nonce) external view returns (bytes32) {
        bytes32 h = keccak256(abi.encodePacked(claimant, ruleId, units, nonce, address(this), block.chainid));
        return h.toEthSignedMessageHash();
    }
}
