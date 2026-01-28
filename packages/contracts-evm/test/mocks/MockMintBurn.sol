// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {AccessManaged} from "@openzeppelin/contracts/access/manager/AccessManaged.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

contract MockMintBurn is AccessManaged, ReentrancyGuard {
    mapping(address => mapping(address => uint256)) public mintCalls;
    mapping(address => mapping(address => uint256)) public burnCalls;

    bool public shouldRevertOnMint = false;
    bool public shouldRevertOnBurn = false;
    uint256 public mintCallCount = 0;
    uint256 public burnCallCount = 0;

    event MintCalled(address to, address token, uint256 amount);
    event BurnCalled(address from, address token, uint256 amount);

    constructor(address initialAuthority) AccessManaged(initialAuthority) {}

    function mint(address to, address token, uint256 amount) external restricted nonReentrant {
        if (shouldRevertOnMint) {
            revert("Mock mint failed");
        }
        mintCalls[to][token] += amount;
        mintCallCount++;
        emit MintCalled(to, token, amount);
    }

    function burn(address from, address token, uint256 amount) external restricted nonReentrant {
        if (shouldRevertOnBurn) {
            revert("Mock burn failed");
        }
        burnCalls[from][token] += amount;
        burnCallCount++;
        emit BurnCalled(from, token, amount);
    }

    function setShouldRevertOnMint(bool shouldRevert) external {
        shouldRevertOnMint = shouldRevert;
    }

    function setShouldRevertOnBurn(bool shouldRevert) external {
        shouldRevertOnBurn = shouldRevert;
    }

    function resetCallCounts() external {
        mintCallCount = 0;
        burnCallCount = 0;
    }
}
