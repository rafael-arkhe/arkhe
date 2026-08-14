pragma solidity ^0.8.0;
contract Vault {
    mapping(address => uint256) public balances;
    function withdraw(uint256 amount) external {
        require(balances[msg.sender] >= amount, "insufficient");
        balances[msg.sender] -= amount; // state write BEFORE external call
        msg.sender.call{value: amount}(""); // reentrancy: unchecked call
    }
    function add(uint256 a, uint256 b) public pure returns (uint256) {
        return a + b; // potential overflow
    }
}
