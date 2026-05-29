// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

abstract contract BasePool {
    uint256 internal totalSupplied;

    function _accrue(uint256 amount) internal {
        totalSupplied += amount;
    }
}
