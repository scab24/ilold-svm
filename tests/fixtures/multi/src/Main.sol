// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Helper} from "./Helper.sol";

contract Main {
    Helper public helper;
    uint256 public total;

    constructor(address _helper) {
        helper = Helper(_helper);
    }

    function bumpAndStore(uint256 amount) external returns (uint256) {
        uint256 newCounter = helper.add(amount);
        total = newCounter * 2;
        return total;
    }

    function clear() external {
        helper.reset();
        total = 0;
    }
}
