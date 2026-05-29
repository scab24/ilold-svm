// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {IPool} from "./IPool.sol";
import {BasePool} from "./BasePool.sol";

contract LendingPool is BasePool, IPool {
    mapping(address => uint256) public balances;

    function supply(uint256 amount) external returns (uint256) {
        _accrue(amount);
        balances[msg.sender] += amount;
        return balances[msg.sender];
    }

    function withdraw(uint256 amount) external returns (uint256) {
        require(balances[msg.sender] >= amount, "insufficient");
        balances[msg.sender] -= amount;
        return balances[msg.sender];
    }
}
