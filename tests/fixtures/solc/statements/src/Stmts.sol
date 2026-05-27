// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

// Exercises every statement kind so the solc frontend mapping is covered.
contract Stmts {
    event Done(uint256 x);
    error Bad(uint256 x);
    uint256 public total;

    modifier gated() {
        require(total >= 0, "g");
        _;
    }

    function run(uint256 n) external gated returns (uint256) {
        uint256 acc = 0;
        if (n > 10) {
            acc = n;
        } else {
            acc = 10;
        }
        for (uint256 i = 0; i < n; i++) {
            if (i == 5) break;
            if (i == 2) continue;
            acc += i;
        }
        while (acc > 100) {
            acc -= 1;
        }
        do {
            acc += 1;
        } while (acc < 5);
        unchecked {
            acc += 1;
        }
        (uint256 a, uint256 b) = split(acc);
        acc = a + b;
        assembly {
            let z := 1
        }
        try this.ext(acc) returns (uint256 r) {
            acc = r;
        } catch Error(string memory) {
            acc = 0;
        } catch {
            acc = 1;
        }
        if (acc == 0) {
            revert Bad(acc);
        }
        emit Done(acc);
        return acc;
    }

    function ext(uint256 x) external pure returns (uint256) {
        return x;
    }

    function split(uint256 x) internal pure returns (uint256, uint256) {
        return (x, x);
    }

    function slice(bytes calldata d) external pure returns (bytes memory) {
        return d[1:3];
    }
}
