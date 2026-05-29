// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

interface IPool {
    function supply(uint256 amount) external returns (uint256);
    function withdraw(uint256 amount) external returns (uint256);
}
