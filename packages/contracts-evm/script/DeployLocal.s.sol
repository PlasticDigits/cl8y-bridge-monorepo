// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console2} from "forge-std/Script.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import {AccessManagerEnumerable} from "../src/AccessManagerEnumerable.sol";
import {ChainRegistry} from "../src/ChainRegistry.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {LockUnlock} from "../src/LockUnlock.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {Bridge} from "../src/Bridge.sol";

/// @title DeployLocal
/// @notice Simplified deployment script for local testing and E2E tests
/// @dev Uses msg.sender as admin, operator, and fee recipient (no env vars needed).
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
            address(new ERC1967Proxy(address(crImpl), abi.encodeCall(ChainRegistry.initialize, (deployer, deployer))));

        // 3. Deploy TokenRegistry (implementation + proxy)
        TokenRegistry trImpl = new TokenRegistry();
        address tokenRegistryProxy = address(
            new ERC1967Proxy(
                address(trImpl),
                abi.encodeCall(TokenRegistry.initialize, (deployer, deployer, ChainRegistry(chainRegistryProxy)))
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

        // 6. Deploy Bridge (implementation + proxy)
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
                        ChainRegistry(chainRegistryProxy),
                        TokenRegistry(tokenRegistryProxy),
                        LockUnlock(lockUnlockProxy),
                        MintBurn(mintBurnProxy)
                    )
                )
            )
        );

        // 7. Configure LockUnlock and MintBurn to authorize Bridge
        LockUnlock(lockUnlockProxy).addAuthorizedCaller(bridgeProxy);
        MintBurn(mintBurnProxy).addAuthorizedCaller(bridgeProxy);

        vm.stopBroadcast();

        // Output proxy addresses in parseable KEY=VALUE format
        console2.log("=== DeployLocal Addresses ===");
        console2.log("DEPLOYED_ACCESS_MANAGER", address(accessManager));
        console2.log("DEPLOYED_CHAIN_REGISTRY", chainRegistryProxy);
        console2.log("DEPLOYED_TOKEN_REGISTRY", tokenRegistryProxy);
        console2.log("DEPLOYED_LOCK_UNLOCK", lockUnlockProxy);
        console2.log("DEPLOYED_MINT_BURN", mintBurnProxy);
        console2.log("DEPLOYED_BRIDGE", bridgeProxy);
    }
}
