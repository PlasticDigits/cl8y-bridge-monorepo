// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {AccessManaged} from "@openzeppelin/contracts/access/manager/AccessManaged.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

contract MockLockUnlock is AccessManaged, ReentrancyGuard {
    mapping(address => mapping(address => uint256)) public lockCalls;
    mapping(address => mapping(address => uint256)) public unlockCalls;

    bool public shouldRevertOnLock = false;
    bool public shouldRevertOnUnlock = false;
    uint256 public lockCallCount = 0;
    uint256 public unlockCallCount = 0;

    event LockCalled(address from, address token, uint256 amount);
    event UnlockCalled(address to, address token, uint256 amount);

    constructor(address initialAuthority) AccessManaged(initialAuthority) {}

    function lock(address from, address token, uint256 amount) external restricted nonReentrant {
        if (shouldRevertOnLock) {
            revert("Mock lock failed");
        }
        lockCalls[from][token] += amount;
        lockCallCount++;
        emit LockCalled(from, token, amount);
    }

    function unlock(address to, address token, uint256 amount) external restricted nonReentrant {
        if (shouldRevertOnUnlock) {
            revert("Mock unlock failed");
        }
        unlockCalls[to][token] += amount;
        unlockCallCount++;
        emit UnlockCalled(to, token, amount);
    }

    function setShouldRevertOnLock(bool shouldRevert) external {
        shouldRevertOnLock = shouldRevert;
    }

    function setShouldRevertOnUnlock(bool shouldRevert) external {
        shouldRevertOnUnlock = shouldRevert;
    }

    function resetCallCounts() external {
        lockCallCount = 0;
        unlockCallCount = 0;
    }
}
