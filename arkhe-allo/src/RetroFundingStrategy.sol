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

/// @title RetroFundingStrategy — retroactive rewards proportional to evaluator scores.
/// @notice Fixes the original bug where the evaluator set was hard-capped at exactly 3
///         (evaluators 4+ were locked out). Here the evaluator set is dynamic and
///         unbounded; each evaluator scores each project once, and the funding pool is
///         split in proportion to total score.
contract RetroFundingStrategy is AccessControl, ReentrancyGuard {
    bytes32 public constant EVALUATOR_ROLE = keccak256("EVALUATOR_ROLE");

    IAlloPool public immutable pool;
    uint256 public fundingPool; // ETH held in the AlloPool, to be split
    uint256 public totalScoreSum;
    bool public finalized;

    address[] public evaluators;
    mapping(address => bool) public isEvaluator;

    address[] public projects;
    mapping(address => bool) public isProject;
    mapping(address => uint256) public totalScore;
    mapping(address => uint256) public allocated;
    mapping(address => uint256) public distributed;
    mapping(address => mapping(address => bool)) public hasScored; // evaluator => project

    event EvaluatorAdded(address indexed evaluator);
    event ProjectRegistered(address indexed project);
    event Scored(address indexed evaluator, address indexed project, uint256 score);
    event Finalized(uint256 fundingPool, uint256 totalScoreSum);
    event Claimed(address indexed project, uint256 amount);

    constructor(address _pool) {
        pool = IAlloPool(_pool);
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }

    /// @notice Add an evaluator. No cap — the set grows dynamically (bug #8 fix).
    function addEvaluator(address e) external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(e != address(0), "evaluator");
        require(!isEvaluator[e], "exists");
        isEvaluator[e] = true;
        evaluators.push(e);
        _grantRole(EVALUATOR_ROLE, e);
        emit EvaluatorAdded(e);
    }

    function registerProject(address p) external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(p != address(0), "project");
        require(!isProject[p], "exists");
        isProject[p] = true;
        projects.push(p);
        emit ProjectRegistered(p);
    }

    /// @notice Fund the retro pool; ETH forwarded into the AlloPool.
    function fund() external payable onlyRole(DEFAULT_ADMIN_ROLE) {
        require(!finalized, "finalized");
        require(msg.value > 0, "zero");
        fundingPool += msg.value;
        pool.deposit{value: msg.value}();
    }

    function submitScore(address project, uint256 score) external onlyRole(EVALUATOR_ROLE) {
        require(!finalized, "finalized");
        require(isProject[project], "no project");
        require(score <= 100, "score range");
        require(!hasScored[msg.sender][project], "scored");
        hasScored[msg.sender][project] = true;
        totalScore[project] += score;
        totalScoreSum += score;
        emit Scored(msg.sender, project, score);
    }

    /// @notice Split the funding pool across projects proportionally to total score.
    function finalize() external onlyRole(DEFAULT_ADMIN_ROLE) nonReentrant {
        require(!finalized, "finalized");
        require(totalScoreSum > 0, "no scores");
        finalized = true;
        uint256 n = projects.length;
        for (uint256 i = 0; i < n; i++) {
            address p = projects[i];
            uint256 s = totalScore[p];
            if (s == 0) continue;
            uint256 amt = (fundingPool * s) / totalScoreSum;
            if (amt > 0) {
                allocated[p] = amt;
                pool.allocate(p, amt);
            }
        }
        emit Finalized(fundingPool, totalScoreSum);
    }

    /// @notice Projects pull their allocation after finalization.
    function claim(address project) external nonReentrant {
        require(finalized, "not finalized");
        uint256 owed = allocated[project] - distributed[project];
        require(owed > 0, "nothing");
        distributed[project] += owed;
        pool.distribute(project, owed);
        emit Claimed(project, owed);
    }

    function evaluatorCount() external view returns (uint256) {
        return evaluators.length;
    }

    function projectCount() external view returns (uint256) {
        return projects.length;
    }
}
