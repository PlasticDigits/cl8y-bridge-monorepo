// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test, console2} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
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
        bytes memory initData = abi.encodeCall(ChainRegistry.initialize, (admin, operator));
        ERC1967Proxy proxy = new ERC1967Proxy(address(implementation), initData);

        chainRegistry = ChainRegistry(address(proxy));
    }

    function test_Initialize() public view {
        assertEq(chainRegistry.owner(), admin);
        assertTrue(chainRegistry.operators(operator));
        assertEq(chainRegistry.nextChainId(), bytes4(uint32(1)));
        assertEq(chainRegistry.VERSION(), 1);
    }

    function test_RegisterChain() public {
        vm.prank(operator);
        bytes4 chainId = chainRegistry.registerChain("evm_1");

        assertEq(chainId, bytes4(uint32(1)));
        assertTrue(chainRegistry.isChainRegistered(chainId));
        assertEq(chainRegistry.nextChainId(), bytes4(uint32(2)));

        // Check hash mapping
        bytes32 expectedHash = keccak256(abi.encode("evm_1"));
        assertEq(chainRegistry.getChainHash(chainId), expectedHash);
        assertEq(chainRegistry.getChainIdFromHash(expectedHash), chainId);
    }

    function test_RegisterMultipleChains() public {
        vm.startPrank(operator);

        bytes4 chain1 = chainRegistry.registerChain("evm_1");
        bytes4 chain2 = chainRegistry.registerChain("evm_56");
        bytes4 chain3 = chainRegistry.registerChain("terraclassic_columbus-5");

        vm.stopPrank();

        assertEq(chain1, bytes4(uint32(1)));
        assertEq(chain2, bytes4(uint32(2)));
        assertEq(chain3, bytes4(uint32(3)));

        assertEq(chainRegistry.getChainCount(), 3);

        bytes4[] memory chains = chainRegistry.getRegisteredChains();
        assertEq(chains.length, 3);
        assertEq(chains[0], chain1);
        assertEq(chains[1], chain2);
        assertEq(chains[2], chain3);
    }

    function test_RegisterChain_RevertsDuplicateRegistration() public {
        vm.prank(operator);
        chainRegistry.registerChain("evm_1");

        vm.prank(operator);
        vm.expectRevert(abi.encodeWithSelector(IChainRegistry.ChainAlreadyRegistered.selector, "evm_1"));
        chainRegistry.registerChain("evm_1");
    }

    function test_RegisterChain_RevertsIfNotOperator() public {
        vm.prank(user);
        vm.expectRevert(IChainRegistry.Unauthorized.selector);
        chainRegistry.registerChain("evm_1");
    }

    function test_AdminCanRegisterChain() public {
        vm.prank(admin);
        bytes4 chainId = chainRegistry.registerChain("evm_1");
        assertTrue(chainRegistry.isChainRegistered(chainId));
    }

    function test_AddOperator() public {
        address newOperator = address(4);

        vm.prank(admin);
        chainRegistry.addOperator(newOperator);

        assertTrue(chainRegistry.operators(newOperator));

        vm.prank(newOperator);
        bytes4 chainId = chainRegistry.registerChain("evm_1");
        assertTrue(chainRegistry.isChainRegistered(chainId));
    }

    function test_RemoveOperator() public {
        vm.prank(admin);
        chainRegistry.removeOperator(operator);

        assertFalse(chainRegistry.operators(operator));

        vm.prank(operator);
        vm.expectRevert(IChainRegistry.Unauthorized.selector);
        chainRegistry.registerChain("evm_1");
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

    function test_IsOperator() public view {
        assertTrue(chainRegistry.isOperator(operator));
        assertTrue(chainRegistry.isOperator(admin)); // Admin is always operator
        assertFalse(chainRegistry.isOperator(user));
    }

    function test_Upgrade() public {
        // Register a chain before upgrade
        vm.prank(operator);
        bytes4 chainId = chainRegistry.registerChain("evm_1");

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
