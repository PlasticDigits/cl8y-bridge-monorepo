// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {TokenCl8yBridged} from "./TokenCl8yBridged.sol";

/// @title TokenCl8yBridgedCustomDecimals
/// @notice Extension of TokenCl8yBridged that supports non-18 decimal configurations.
/// @dev Used when a bridged token needs different decimals on different chains
///      (e.g. 18 on BSC, 12 on opBNB, 6 on Terra). Deploy directly instead of
///      via FactoryTokenCl8yBridged which always creates 18-decimal tokens.
contract TokenCl8yBridgedCustomDecimals is TokenCl8yBridged {
    uint8 private immutable _customDecimals;

    constructor(
        string memory name,
        string memory symbol,
        address initialAuthority,
        string memory _logoLink,
        uint8 decimals_
    ) TokenCl8yBridged(name, symbol, initialAuthority, _logoLink) {
        _customDecimals = decimals_;
    }

    function decimals() public view override returns (uint8) {
        return _customDecimals;
    }
}
