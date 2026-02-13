// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console2} from "forge-std/Script.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import {WETH} from "solady/tokens/WETH.sol";
import {AccessManagerEnumerable} from "../src/AccessManagerEnumerable.sol";
import {ChainRegistry} from "../src/ChainRegistry.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {LockUnlock} from "../src/LockUnlock.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {Bridge} from "../src/Bridge.sol";

/// @title DeployLocal
/// @notice Simplified deployment script for local testing and E2E tests
/// @dev Uses msg.sender as admin, operator, and fee recipient.
///      Reads optional env vars:
///        THIS_V2_CHAIN_ID  – the globally-unique V2 chain ID for this chain (default 1)
///        THIS_CHAIN_LABEL  – human-readable label registered in ChainRegistry (default "evm_31337")
///      Outputs proxy addresses in KEY=VALUE format for Rust-side parsing.
contract DeployLocal is Script {
    function run() public {
        vm.startBroadcast();

        address deployer = msg.sender;

        // 1. Deploy AccessManagerEnumerable
        AccessManagerEnumerable accessManager = new AccessManagerEnumerable(deployer);

        // 2. Deploy ChainRegistry (implementation + proxy)
        ChainRegistry crImpl = new ChainRegistry();
        address chainRegistryProxy =
            address(new ERC1967Proxy(address(crImpl), abi.encodeCall(ChainRegistry.initialize, (deployer))));

        // 3. Deploy TokenRegistry (implementation + proxy)
        TokenRegistry trImpl = new TokenRegistry();
        address tokenRegistryProxy = address(
            new ERC1967Proxy(
                address(trImpl), abi.encodeCall(TokenRegistry.initialize, (deployer, ChainRegistry(chainRegistryProxy)))
            )
        );

        // 4. Deploy LockUnlock (implementation + proxy)
        LockUnlock luImpl = new LockUnlock();
        address lockUnlockProxy =
            address(new ERC1967Proxy(address(luImpl), abi.encodeCall(LockUnlock.initialize, (deployer))));

        // 5. Deploy MintBurn (implementation + proxy)
        MintBurn mbImpl = new MintBurn();
        address mintBurnProxy =
            address(new ERC1967Proxy(address(mbImpl), abi.encodeCall(MintBurn.initialize, (deployer))));

        // 6. Register this chain with its globally-unique V2 chain ID.
        //    Defaults to 1 for backward compat with existing Rust E2E tests.
        //    For multi-EVM setups, set THIS_V2_CHAIN_ID env var (e.g. 3 for anvil1).
        uint32 thisChainIdNum = uint32(vm.envOr("THIS_V2_CHAIN_ID", uint256(1)));
        bytes4 evmChainId = bytes4(thisChainIdNum);
        string memory chainLabel = vm.envOr("THIS_CHAIN_LABEL", string("evm_31337"));
        ChainRegistry(chainRegistryProxy).registerChain(chainLabel, evmChainId);

        // 7. Deploy WETH for native deposits (Solady WETH)
        WETH weth = new WETH();

        // 8. Deploy Bridge (implementation + proxy)
        Bridge bridgeImpl = new Bridge();
        address bridgeProxy = address(
            new ERC1967Proxy(
                address(bridgeImpl),
                abi.encodeCall(
                    Bridge.initialize,
                    (
                        deployer,
                        deployer,
                        deployer,
                        address(weth),
                        ChainRegistry(chainRegistryProxy),
                        TokenRegistry(tokenRegistryProxy),
                        LockUnlock(lockUnlockProxy),
                        MintBurn(mintBurnProxy),
                        evmChainId
                    )
                )
            )
        );

        // 9. Configure LockUnlock and MintBurn to authorize Bridge
        LockUnlock(lockUnlockProxy).addAuthorizedCaller(bridgeProxy);
        MintBurn(mintBurnProxy).addAuthorizedCaller(bridgeProxy);

        vm.stopBroadcast();

        // Output proxy addresses in parseable KEY=VALUE format
        console2.log("=== DeployLocal Addresses ===");
        console2.log("DEPLOYED_ACCESS_MANAGER", address(accessManager));
        console2.log("DEPLOYED_WETH", address(weth));
        console2.log("DEPLOYED_CHAIN_REGISTRY", chainRegistryProxy);
        console2.log("DEPLOYED_TOKEN_REGISTRY", tokenRegistryProxy);
        console2.log("DEPLOYED_LOCK_UNLOCK", lockUnlockProxy);
        console2.log("DEPLOYED_MINT_BURN", mintBurnProxy);
        console2.log("DEPLOYED_BRIDGE", bridgeProxy);
        console2.log("DEPLOYED_V2_CHAIN_ID", thisChainIdNum);
    }
}
