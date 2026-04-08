// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {TokenRateLimit} from "../src/TokenRateLimit.sol";

/// @title SetTokenRateLimitTestTokens
/// @notice Calls `TokenRateLimit.setLimitsBatch` for testa / testb / tdec (24h deposit + withdraw caps only).
/// @dev Required env: `TOKEN_RATE_LIMIT`, `TOKEN_TESTA`, `TOKEN_TESTB`, `TOKEN_TDEC` (addresses on this chain).
///      Default: **1000 whole tokens** per 24h window per token, scaled by decimals (`TESTA_DECIMALS` /
///      `TESTB_DECIMALS` / `TDEC_DECIMALS`, default 18; use `TDEC_DECIMALS=12` on opBNB).
///      Override window caps in base units: `LIMIT_WEI_TESTA`, `LIMIT_WEI_TESTB`, `LIMIT_WEI_TDEC`, or `LIMIT_WEI_AB`.
///
///      On-chain `TokenRateLimit` has a single limit per direction (no separate per-tx cap in this contract).
contract SetTokenRateLimitTestTokens is Script {
    uint256 internal constant WINDOW_WHOLE_UNITS = 1000;

    function run() external {
        address trl = vm.envAddress("TOKEN_RATE_LIMIT");
        address tokenTestA = vm.envAddress("TOKEN_TESTA");
        address tokenTestB = vm.envAddress("TOKEN_TESTB");
        address tokenTdec = vm.envAddress("TOKEN_TDEC");

        uint8 decA = uint8(vm.envOr("TESTA_DECIMALS", uint256(18)));
        uint8 decB = uint8(vm.envOr("TESTB_DECIMALS", uint256(18)));
        uint8 decTdec = uint8(vm.envOr("TDEC_DECIMALS", uint256(18)));

        uint256 winA =
            vm.envOr("LIMIT_WEI_TESTA", vm.envOr("LIMIT_WEI_AB", _wholeTokensToBaseUnits(WINDOW_WHOLE_UNITS, decA)));
        uint256 winB =
            vm.envOr("LIMIT_WEI_TESTB", vm.envOr("LIMIT_WEI_AB", _wholeTokensToBaseUnits(WINDOW_WHOLE_UNITS, decB)));
        uint256 winTdec = vm.envOr("LIMIT_WEI_TDEC", _wholeTokensToBaseUnits(WINDOW_WHOLE_UNITS, decTdec));

        address[] memory tokens = new address[](3);
        tokens[0] = tokenTestA;
        tokens[1] = tokenTestB;
        tokens[2] = tokenTdec;

        uint256[] memory depositLimits = new uint256[](3);
        depositLimits[0] = winA;
        depositLimits[1] = winB;
        depositLimits[2] = winTdec;

        uint256[] memory withdrawLimits = new uint256[](3);
        withdrawLimits[0] = winA;
        withdrawLimits[1] = winB;
        withdrawLimits[2] = winTdec;

        vm.startBroadcast();
        TokenRateLimit(trl).setLimitsBatch(tokens, depositLimits, withdrawLimits);
        vm.stopBroadcast();

        console.log("setLimitsBatch (24h deposit + withdraw caps)");
        console.log("testa=%s limit=%s", tokenTestA, winA);
        console.log("testb=%s limit=%s", tokenTestB, winB);
        console.log("tdec=%s limit=%s", tokenTdec, winTdec);
    }

    function _wholeTokensToBaseUnits(uint256 whole, uint8 decimals_) internal pure returns (uint256) {
        require(decimals_ <= 18, "decimals>18");
        return whole * (10 ** uint256(decimals_));
    }
}
