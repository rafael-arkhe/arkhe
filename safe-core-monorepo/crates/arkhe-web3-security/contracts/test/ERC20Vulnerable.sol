// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

/// @title ERC20Vulnerable
/// @notice Deliberately vulnerable vault contract, used only by
/// `ERC20Vulnerable.t.sol` to demonstrate the exploit that
/// `contracts::reentrancy::ReentrancyGuard` / `check_effects_interactions_order` (in
/// `crates/arkhe-web3-security/src/contracts/reentrancy.rs`) are designed to catch —
/// the same Checks-Effects-Interactions violation referenced in that Rust module's own
/// doc comment (the GMX V1 `executeDecreaseOrder` pattern). Not a real token, not
/// deployable to anything but a local test chain — do not use.
contract ERC20Vulnerable {
    mapping(address => uint256) public balances;

    function deposit() external payable {
        balances[msg.sender] += msg.value;
    }

    /// @dev VULNERABLE: sends ETH via a low-level `call` (which yields control to the
    /// recipient, letting it run arbitrary code) BEFORE zeroing the caller's balance. A
    /// malicious contract's `receive()` hook can call `withdraw()` again before the
    /// first call returns, draining more than it ever deposited.
    function withdraw() external {
        uint256 amount = balances[msg.sender];
        require(amount > 0, "nothing to withdraw");

        (bool success,) = msg.sender.call{value: amount}("");
        require(success, "transfer failed");

        balances[msg.sender] = 0; // effect AFTER interaction — the bug
    }

    receive() external payable {}
}
