// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";

import {EvmParityReplay} from "../script/EvmParityReplay.s.sol";
import {MockWETH} from "./mocks/MockWETH.sol";

/// @notice Rehearses `runBroadcastHead` on a local Anvil fork without `forge script --broadcast`, avoiding Foundry’s
///         script path where `LocalTraceIdentifier` can attribute a CREATE to the wrong artifact when runtime matches
///         a nested proxy but artifact `deployedBytecode` length does not match trace output (`BridgeParityNonce10Outer`).
/// @dev Run from `packages/contracts-evm` with Anvil up and `PARITY_HEAD_REHEARSAL_RPC` (default `http://127.0.0.1:18545`).
///      Inherits `EvmParityReplay` so `runBroadcastHead` runs as `this` without consuming the historical deployer’s nonce.
contract ParityHeadAnvilRehearsalTest is Test, EvmParityReplay {
    address internal constant HISTORICAL = 0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e;
    address internal constant GOLDEN_BRIDGE = 0x7D3903d07C4267d2Ec5730Bc2340450e3fAa8F3D;

    function setUp() public {
        string memory rpc = vm.envOr("PARITY_HEAD_REHEARSAL_RPC", string("http://127.0.0.1:18545"));
        vm.createSelectFork(rpc);
    }

    function _exportParityHeadEnv(address legacyWeth) internal {
        vm.setEnv("ADMIN_ADDRESS", vm.toString(address(0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c)));
        vm.setEnv("OPERATOR_ADDRESS", vm.toString(address(0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD)));
        vm.setEnv("FEE_RECIPIENT_ADDRESS", vm.toString(address(0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD)));
        vm.setEnv("PARITY_LEGACY_WETH_ADDRESS", vm.toString(legacyWeth));
        vm.setEnv("PARITY_LEGACY_CHAIN_IDENTIFIER", "BSC");
        vm.setEnv("PARITY_LEGACY_THIS_CHAIN_ID", "56");
        vm.setEnv("DEPLOYER_ADDRESS", vm.toString(HISTORICAL));
    }

    /// @dev Historical deployer must start at `ENTRY_NONCE` (0); `deal` + `startBroadcast(deployer)` (see `runBroadcastHead`).
    function test_runBroadcastHead_goldenBridgeAndNonce18() public {
        MockWETH weth = new MockWETH();
        _exportParityHeadEnv(address(weth));

        vm.deal(HISTORICAL, 1000 ether);
        runBroadcastHead();

        assertEq(vm.getNonce(HISTORICAL), 18, "deployer nonce after head");
        assertEq(bridgeProxy, GOLDEN_BRIDGE, "BSC golden bridge proxy");
    }
}
