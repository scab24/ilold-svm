// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {IPool} from "./IPool.sol";
import {SafeMath} from "./SafeMath.sol";

contract Vault {
    using SafeMath for uint256;

    struct DepositInfo {
        uint256 amount;
        address from;
    }

    IPool public pool;

    constructor(IPool pool_) {
        pool = pool_;
    }

    // Cross-contract call through a typed state variable.
    function depositVia(uint256 amount) external returns (uint256) {
        uint256 total = amount.safeAdd(1);
        return pool.supply(total);
    }

    function record(uint256 amount) external view returns (uint256) {
        DepositInfo memory info = DepositInfo({amount: amount.safeAdd(0), from: msg.sender});
        return info.amount;
    }

    // Cross-contract call through interface casting of a raw address.
    function depositCast(address poolAddr, uint256 amount) external returns (uint256) {
        return IPool(poolAddr).supply(amount);
    }

    // Call to a contract method literally named `push` — not an array push.
    function pushVia(uint256 amount) external returns (uint256) {
        return pool.push(amount);
    }
}
