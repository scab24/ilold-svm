// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract SimpleStorage {
    uint256 private value;

    event ValueChanged(uint256 newValue);

    function get() public view returns (uint256) {
        return value;
    }

    function set(uint256 newValue) public {
        require(newValue > 0, "Value must be positive");
        value = newValue;
        emit ValueChanged(newValue);
    }
}
