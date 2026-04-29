// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Bridge} from "../src/Bridge.sol";
import {BscParityNonce10InnerProxy} from "./BscParityNonce10InnerProxy.sol";
import {ChainRegistry} from "../src/ChainRegistry.sol";
import {LockUnlock} from "../src/LockUnlock.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";

/// @notice BSC parity: one EOA `CREATE` (nonce 10) whose runtime is EIP-1967 UUPS proxy bytecode to a fresh `Bridge` impl.
/// @dev Inner `new` uses the contract-under-construction as deployer, so EOA nonces 0–9 stay aligned with legacy.
///      An extra inner proxy is deployed at an ephemeral address and left unused (same pattern as frozen initcode).
///      For `forge script --broadcast`, use a forge built with `scripts/evm/patches/foundry-5e88010-local-identify-creation-prefix.patch`
///      (`scripts/evm/install-foundry-parity-fix.sh`) or set `FORGE` to that binary in `parity-replay.sh`: stock forge
///      can mis-identify the nonce-10 CREATE when matching by runtime-only (constructor `return`s copied proxy code).
contract BridgeParityNonce10Outer {
    constructor(
        address admin,
        address operator,
        address feeRecipient,
        address wrappedNative,
        ChainRegistry chainRegistry,
        TokenRegistry tokenRegistry,
        LockUnlock lockUnlock,
        MintBurn mintBurn,
        bytes4 thisChainId
    ) {
        Bridge impl = new Bridge();
        bytes memory initData = abi.encodeCall(
            Bridge.initialize,
            (
                admin,
                operator,
                feeRecipient,
                wrappedNative,
                chainRegistry,
                tokenRegistry,
                lockUnlock,
                mintBurn,
                thisChainId
            )
        );
        address innerProxy = address(new BscParityNonce10InnerProxy(abi.encode(address(impl), initData)));

        uint256 sz;
        assembly {
            sz := extcodesize(innerProxy)
        }
        bytes memory runtime = new bytes(sz);
        assembly {
            extcodecopy(innerProxy, add(runtime, 0x20), 0, sz)
        }
        assembly {
            return(add(runtime, 0x20), mload(runtime))
        }
    }
}
