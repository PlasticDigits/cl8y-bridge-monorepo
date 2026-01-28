// SPDX-License-Identifier: AGPL-3.0-only
// Authored by Plastic Digits
pragma solidity ^0.8.30;

interface IBlacklist {
    function isBlacklisted(address account) external view returns (bool);

    function setIsBlacklistedToTrue(address[] calldata accounts) external;

    function setIsBlacklistedToFalse(address[] calldata accounts) external;
}
