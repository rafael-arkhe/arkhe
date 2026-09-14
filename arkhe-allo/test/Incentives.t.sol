// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AlloPool} from "../src/AlloPool.sol";
import {DirectToContractIncentives} from "../src/DirectToContractIncentives.sol";

interface Vm {
    function deal(address, uint256) external;
    function prank(address) external;
    function addr(uint256) external returns (address);
    function sign(uint256, bytes32) external returns (uint8, bytes32, bytes32);
    function expectRevert(bytes calldata) external;
}

contract IncentivesTest {
    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);

    AlloPool pool;
    DirectToContractIncentives inc;

    uint256 constant ORACLE_PK = 0xA11CE;
    address oracle;
    bytes32 constant RULE = keccak256("github-star");

    function setUp() public {
        oracle = vm.addr(ORACLE_PK);
        pool = new AlloPool(address(this));
        inc = new DirectToContractIncentives(address(pool), oracle);
        pool.setStrategy(address(inc));
        vm.deal(address(this), 1000 ether);
        pool.deposit{value: 100 ether}();
        inc.createRule(RULE, 1 ether, 100 ether); // 1 ETH per unit, 100 ETH cap
    }

    function _req(bool c, string memory why) internal pure {
        require(c, why);
    }

    /// Build an oracle signature for (claimant, RULE, units, nonce).
    function _sign(uint256 pk, address claimant, uint256 units, uint256 nonce) internal returns (bytes memory) {
        bytes32 digest = inc.digestFor(claimant, RULE, units, nonce);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(pk, digest);
        return abi.encodePacked(r, s, v);
    }

    /// Happy path: a valid oracle signature pays the claimant via the pool.
    function testValidSignedClaimPays() public {
        address alice = address(0xA1);
        bytes memory sig = _sign(ORACLE_PK, alice, 5, 1);
        vm.prank(alice);
        inc.claim(RULE, 5, 1, sig);
        _req(alice.balance == 5 ether, "claimant not paid 5 ETH");
    }

    /// THE FIX: an attacker with no valid oracle signature cannot claim anything.
    /// (This is what the old public `claimReward` allowed — see the drain demo.)
    function testUnsignedAttackerReverts() public {
        address attacker = address(0xBAD);
        // attacker self-signs with their own key (not the oracle)
        uint256 attackerPk = 0xB0B;
        bytes memory forged = _sign(attackerPk, attacker, 100, 1);
        vm.prank(attacker);
        vm.expectRevert(bytes("bad signature"));
        inc.claim(RULE, 100, 1, forged);
        _req(attacker.balance == 0, "attacker should have nothing");
    }

    /// A valid signature cannot be replayed (per-user nonce).
    function testSignatureReplayReverts() public {
        address alice = address(0xA1);
        bytes memory sig = _sign(ORACLE_PK, alice, 5, 7);
        vm.prank(alice);
        inc.claim(RULE, 5, 7, sig);
        vm.prank(alice);
        vm.expectRevert(bytes("nonce used"));
        inc.claim(RULE, 5, 7, sig);
    }

    /// A signature issued for Alice cannot be used by Bob (claimant is bound in).
    function testSignatureBoundToClaimant() public {
        address alice = address(0xA1);
        address bob = address(0xB2);
        bytes memory sigForAlice = _sign(ORACLE_PK, alice, 5, 3);
        vm.prank(bob);
        vm.expectRevert(bytes("bad signature"));
        inc.claim(RULE, 5, 3, sigForAlice);
    }

    /// The reward cap is enforced even with a valid signature.
    function testCapEnforced() public {
        address whale = address(0xA1);
        bytes memory sig = _sign(ORACLE_PK, whale, 101, 1); // 101 ETH > 100 cap
        vm.prank(whale);
        vm.expectRevert(bytes("cap"));
        inc.claim(RULE, 101, 1, sig);
    }
}
