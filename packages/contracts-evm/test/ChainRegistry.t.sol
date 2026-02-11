// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import {ChainRegistry} from "../src/ChainRegistry.sol";
import {IChainRegistry} from "../src/interfaces/IChainRegistry.sol";

contract ChainRegistryTest is Test {
    ChainRegistry public chainRegistry;
    address public admin = address(1);
    address public operator = address(2);
    address public user = address(3);

    function setUp() public {
        // Deploy implementation
        ChainRegistry implementation = new ChainRegistry();

        // Deploy proxy
        bytes memory initData = abi.encodeCall(ChainRegistry.initialize, (admin));
        ERC1967Proxy proxy = new ERC1967Proxy(address(implementation), initData);

        chainRegistry = ChainRegistry(address(proxy));
    }

    function test_Initialize() public view {
        assertEq(chainRegistry.owner(), admin);
        assertEq(chainRegistry.VERSION(), 1);
    }

    function test_RegisterChain() public {
        bytes4 chainId = bytes4(uint32(1));

        vm.prank(admin);
        chainRegistry.registerChain("evm_1", chainId);

        assertTrue(chainRegistry.isChainRegistered(chainId));

        // Check hash mapping
        bytes32 expectedHash = keccak256(abi.encode("evm_1"));
        assertEq(chainRegistry.getChainHash(chainId), expectedHash);
        assertEq(chainRegistry.getChainIdFromHash(expectedHash), chainId);
    }

    function test_RegisterMultipleChains() public {
        bytes4 chain1 = bytes4(uint32(1));
        bytes4 chain2 = bytes4(uint32(2));
        bytes4 chain3 = bytes4(uint32(3));

        vm.startPrank(admin);
        chainRegistry.registerChain("evm_1", chain1);
        chainRegistry.registerChain("evm_56", chain2);
        chainRegistry.registerChain("terraclassic_columbus-5", chain3);
        vm.stopPrank();

        assertTrue(chainRegistry.isChainRegistered(chain1));
        assertTrue(chainRegistry.isChainRegistered(chain2));
        assertTrue(chainRegistry.isChainRegistered(chain3));

        assertEq(chainRegistry.getChainCount(), 3);

        bytes4[] memory chains = chainRegistry.getRegisteredChains();
        assertEq(chains.length, 3);
        assertEq(chains[0], chain1);
        assertEq(chains[1], chain2);
        assertEq(chains[2], chain3);
    }

    function test_RegisterChain_RevertsDuplicateIdentifier() public {
        vm.prank(admin);
        chainRegistry.registerChain("evm_1", bytes4(uint32(1)));

        vm.prank(admin);
        vm.expectRevert(abi.encodeWithSelector(IChainRegistry.ChainAlreadyRegistered.selector, "evm_1"));
        chainRegistry.registerChain("evm_1", bytes4(uint32(2)));
    }

    function test_RegisterChain_RevertsDuplicateChainId() public {
        bytes4 chainId = bytes4(uint32(1));

        vm.prank(admin);
        chainRegistry.registerChain("evm_1", chainId);

        vm.prank(admin);
        vm.expectRevert(abi.encodeWithSelector(IChainRegistry.ChainIdAlreadyInUse.selector, chainId));
        chainRegistry.registerChain("evm_56", chainId);
    }

    function test_RegisterChain_RevertsZeroChainId() public {
        vm.prank(admin);
        vm.expectRevert(IChainRegistry.InvalidChainId.selector);
        chainRegistry.registerChain("evm_1", bytes4(0));
    }

    function test_RegisterChain_RevertsIfNotOwner() public {
        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, user));
        chainRegistry.registerChain("evm_1", bytes4(uint32(1)));
    }

    function test_AdminCanRegisterChain() public {
        bytes4 chainId = bytes4(uint32(1));
        vm.prank(admin);
        chainRegistry.registerChain("evm_1", chainId);
        assertTrue(chainRegistry.isChainRegistered(chainId));
    }

    function test_UnregisterChain() public {
        bytes4 chainId = bytes4(uint32(1));

        vm.prank(admin);
        chainRegistry.registerChain("evm_1", chainId);
        assertTrue(chainRegistry.isChainRegistered(chainId));

        vm.prank(admin);
        chainRegistry.unregisterChain(chainId);

        // Verify all mappings are cleared
        assertFalse(chainRegistry.isChainRegistered(chainId));
        assertEq(chainRegistry.getChainHash(chainId), bytes32(0));
        assertEq(chainRegistry.getChainIdFromHash(keccak256(abi.encode("evm_1"))), bytes4(0));
        assertEq(chainRegistry.getChainCount(), 0);
    }

    function test_UnregisterChain_CanReRegisterIdentifier() public {
        bytes4 chainId1 = bytes4(uint32(1));
        bytes4 chainId2 = bytes4(uint32(2));

        vm.startPrank(admin);
        chainRegistry.registerChain("evm_1", chainId1);
        chainRegistry.unregisterChain(chainId1);

        // Can re-register the same identifier with a different chain ID
        chainRegistry.registerChain("evm_1", chainId2);
        vm.stopPrank();

        assertTrue(chainRegistry.isChainRegistered(chainId2));
        assertFalse(chainRegistry.isChainRegistered(chainId1));
    }

    function test_UnregisterChain_CanReUseChainId() public {
        bytes4 chainId = bytes4(uint32(1));

        vm.startPrank(admin);
        chainRegistry.registerChain("evm_1", chainId);
        chainRegistry.unregisterChain(chainId);

        // Can re-use the same chain ID with a different identifier
        chainRegistry.registerChain("evm_56", chainId);
        vm.stopPrank();

        assertTrue(chainRegistry.isChainRegistered(chainId));
        assertEq(chainRegistry.getChainHash(chainId), keccak256(abi.encode("evm_56")));
    }

    function test_UnregisterChain_RevertsIfNotRegistered() public {
        bytes4 unregistered = bytes4(uint32(99));
        vm.prank(admin);
        vm.expectRevert(abi.encodeWithSelector(IChainRegistry.ChainNotRegistered.selector, unregistered));
        chainRegistry.unregisterChain(unregistered);
    }

    function test_UnregisterChain_RevertsIfNotOwner() public {
        bytes4 chainId = bytes4(uint32(1));
        vm.prank(admin);
        chainRegistry.registerChain("evm_1", chainId);

        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, user));
        chainRegistry.unregisterChain(chainId);
    }

    function test_UnregisterChain_MiddleOfArray() public {
        bytes4 chain1 = bytes4(uint32(1));
        bytes4 chain2 = bytes4(uint32(2));
        bytes4 chain3 = bytes4(uint32(3));

        vm.startPrank(admin);
        chainRegistry.registerChain("evm_1", chain1);
        chainRegistry.registerChain("evm_56", chain2);
        chainRegistry.registerChain("terraclassic_columbus-5", chain3);

        // Unregister the middle chain
        chainRegistry.unregisterChain(chain2);
        vm.stopPrank();

        assertEq(chainRegistry.getChainCount(), 2);
        bytes4[] memory chains = chainRegistry.getRegisteredChains();
        assertEq(chains.length, 2);
        // After swap-remove: [chain1, chain3]
        assertEq(chains[0], chain1);
        assertEq(chains[1], chain3);
    }

    function test_RevertIfChainNotRegistered() public {
        bytes4 unregisteredChain = bytes4(uint32(99));

        vm.expectRevert(abi.encodeWithSelector(IChainRegistry.ChainNotRegistered.selector, unregisteredChain));
        chainRegistry.revertIfChainNotRegistered(unregisteredChain);
    }

    function test_ComputeIdentifierHash() public view {
        bytes32 hash = chainRegistry.computeIdentifierHash("evm_1");
        assertEq(hash, keccak256(abi.encode("evm_1")));
    }

    function test_Upgrade() public {
        // Register a chain before upgrade
        bytes4 chainId = bytes4(uint32(1));
        vm.prank(admin);
        chainRegistry.registerChain("evm_1", chainId);

        // Deploy new implementation
        ChainRegistry newImplementation = new ChainRegistry();

        // Upgrade
        vm.prank(admin);
        chainRegistry.upgradeToAndCall(address(newImplementation), "");

        // Verify state preserved
        assertTrue(chainRegistry.isChainRegistered(chainId));
        assertEq(chainRegistry.VERSION(), 1);
    }

    function test_Upgrade_RevertsIfNotOwner() public {
        ChainRegistry newImplementation = new ChainRegistry();

        vm.prank(operator);
        vm.expectRevert();
        chainRegistry.upgradeToAndCall(address(newImplementation), "");
    }
}
