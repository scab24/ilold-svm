// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract Helper {
    uint256 public counter;
    address public owner;

    constructor() {
        owner = msg.sender;
    }

    function add(uint256 amount) external returns (uint256) {
        require(amount > 0, "zero");
        counter += amount;
        return counter;
    }

    function reset() external {
        require(msg.sender == owner, "not owner");
        counter = 0;
    }
}
