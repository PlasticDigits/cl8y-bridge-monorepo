// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
import {EvmParityReplay} from "../script/EvmParityReplay.s.sol";

/// @notice CI guard: golden EOA CREATE + factory CREATE3 predictions must stay aligned with BSC history.
contract BscParityReplayDryRunTest is Test {
    function test_DryCheck_PASS_against_golden_JSON() public {
        vm.setEnv("DEPLOYER_ADDRESS", "0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e");
        EvmParityReplay replay = new EvmParityReplay();
        replay.runDryCheck();
    }
}
