// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {IPool} from "./IPool.sol";
import {SafeMath} from "./SafeMath.sol";

contract Vault {
    using SafeMath for uint256;

    IPool public pool;

    constructor(IPool pool_) {
        pool = pool_;
    }

    // Cross-contract call through a typed state variable.
    function depositVia(uint256 amount) external returns (uint256) {
        uint256 total = amount.safeAdd(1);
        return pool.supply(total);
    }

    // Cross-contract call through interface casting of a raw address.
    function depositCast(address poolAddr, uint256 amount) external returns (uint256) {
        return IPool(poolAddr).supply(amount);
    }
}
