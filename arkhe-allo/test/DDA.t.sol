// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AlloPool} from "../src/AlloPool.sol";
import {DedicatedDomainAllocation} from "../src/DedicatedDomainAllocation.sol";

interface Vm {
    function deal(address, uint256) external;
    function prank(address) external;
    function expectRevert(bytes calldata) external;
}

contract DDATest {
    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);

    AlloPool pool;
    DedicatedDomainAllocation dda;

    bytes32 constant DOM_A = keccak256("engineering");
    bytes32 constant DOM_B = keccak256("research");
    address constant STEWARD_A = address(0x5731A);
    address constant STEWARD_B = address(0x5731B);
    address constant RCPT = address(0x4EC1);

    function setUp() public {
        pool = new AlloPool(address(this));
        dda = new DedicatedDomainAllocation(address(pool));
        pool.setStrategy(address(dda));
        vm.deal(address(this), 1000 ether);
        dda.createDomain(DOM_A, STEWARD_A);
        dda.createDomain(DOM_B, STEWARD_B);
    }

    function _req(bool c, string memory why) internal pure {
        require(c, why);
    }

    /// THE FIX: funding a domain forwards ETH into the pool (original bug left it stuck).
    function testFundForwardsToPool() public {
        dda.fundDomain{value: 40 ether}(DOM_A);
        _req(pool.availableBalance() == 40 ether, "funds not forwarded to pool");
        (uint256 budget, uint256 spent) = dda.budgetOf(DOM_A);
        _req(budget == 40 ether && spent == 0, "budget accounting wrong");
    }

    /// Steward allocates within budget; recipient is actually paid.
    function testStewardAllocatesAndPays() public {
        dda.fundDomain{value: 40 ether}(DOM_A);
        vm.prank(STEWARD_A);
        dda.approveRecipient(DOM_A, RCPT);
        vm.prank(STEWARD_A);
        dda.allocateToRecipient(DOM_A, RCPT, 40 ether);
        _req(RCPT.balance == 40 ether, "recipient not paid");
        (, uint256 spent) = dda.budgetOf(DOM_A);
        _req(spent == 40 ether, "spent not tracked");
    }

    /// Over-budget allocation reverts.
    function testOverBudgetReverts() public {
        dda.fundDomain{value: 40 ether}(DOM_A);
        vm.prank(STEWARD_A);
        dda.approveRecipient(DOM_A, RCPT);
        vm.prank(STEWARD_A);
        vm.expectRevert(bytes("over budget"));
        dda.allocateToRecipient(DOM_A, RCPT, 41 ether);
    }

    /// KEY: a steward cannot spend another domain's budget, even though the pool
    /// physically holds both domains' funds.
    function testBudgetIsolationAcrossDomains() public {
        dda.fundDomain{value: 30 ether}(DOM_A);
        dda.fundDomain{value: 50 ether}(DOM_B); // pool now holds 80 total
        vm.prank(STEWARD_A);
        dda.approveRecipient(DOM_A, RCPT);

        // Steward A can spend its 30, but not a wei more — despite 50 of B's in the pool.
        vm.prank(STEWARD_A);
        dda.allocateToRecipient(DOM_A, RCPT, 30 ether);
        _req(RCPT.balance == 30 ether, "A should have paid 30");
        vm.prank(STEWARD_A);
        vm.expectRevert(bytes("over budget"));
        dda.allocateToRecipient(DOM_A, RCPT, 1 ether);
    }

    /// Only the domain's steward may allocate.
    function testOnlyStewardAllocates() public {
        dda.fundDomain{value: 40 ether}(DOM_A);
        vm.prank(STEWARD_A);
        dda.approveRecipient(DOM_A, RCPT);
        vm.prank(STEWARD_B); // wrong steward
        vm.expectRevert(bytes("not steward"));
        dda.allocateToRecipient(DOM_A, RCPT, 10 ether);
    }

    /// Unapproved recipients cannot be paid.
    function testUnapprovedRecipientReverts() public {
        dda.fundDomain{value: 40 ether}(DOM_A);
        vm.prank(STEWARD_A);
        vm.expectRevert(bytes("not approved"));
        dda.allocateToRecipient(DOM_A, RCPT, 10 ether);
    }
}
