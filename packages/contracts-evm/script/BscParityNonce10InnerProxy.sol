// SPDX-License-Identifier: MIT
pragma solidity ^0.8.22;

import {Proxy} from "@openzeppelin/contracts/proxy/Proxy.sol";
import {ERC1967Utils} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Utils.sol";

/// @notice EIP-1967 UUPS proxy with the same runtime as OZ `ERC1967Proxy`, but `constructor(bytes)` so the init
///         template differs from `constructor(address,bytes)` (used only inside `BridgeParityNonce10Outer`).
contract BscParityNonce10InnerProxy is Proxy {
    constructor(bytes memory initPayload) payable {
        (address implementation, bytes memory _data) = abi.decode(initPayload, (address, bytes));
        ERC1967Utils.upgradeToAndCall(implementation, _data);
    }

    function _implementation() internal view virtual override returns (address) {
        return ERC1967Utils.getImplementation();
    }
}
