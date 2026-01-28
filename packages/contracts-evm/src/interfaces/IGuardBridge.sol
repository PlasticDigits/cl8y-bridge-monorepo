// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

interface IGuardBridge {
    function checkAccount(address account) external;
    function checkDeposit(address token, uint256 amount, address sender) external;

    function checkWithdraw(address token, uint256 amount, address sender) external;
}
