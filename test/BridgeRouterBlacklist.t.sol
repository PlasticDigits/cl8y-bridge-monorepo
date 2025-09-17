// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";

import {Cl8YBridge} from "../src/CL8YBridge.sol";
import {BridgeRouter} from "../src/BridgeRouter.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {ChainRegistry} from "../src/ChainRegistry.sol";
import {TokenCl8yBridged} from "../src/TokenCl8yBridged.sol";
import {FactoryTokenCl8yBridged} from "../src/FactoryTokenCl8yBridged.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {LockUnlock} from "../src/LockUnlock.sol";
import {IWETH} from "../src/interfaces/IWETH.sol";
import {DatastoreSetAddress} from "../src/DatastoreSetAddress.sol";
import {GuardBridge} from "../src/GuardBridge.sol";
import {BlacklistBasic} from "../src/BlacklistBasic.sol";
import {MockWETH} from "./mocks/MockWETH.sol";

contract BridgeRouterBlacklistTest is Test {
    AccessManager public accessManager;
    ChainRegistry public chainRegistry;
    TokenRegistry public tokenRegistry;
    MintBurn public mintBurn;
    LockUnlock public lockUnlock;
    Cl8YBridge public bridge;
    BridgeRouter public router;
    IWETH public weth;
    DatastoreSetAddress public datastore;
    GuardBridge public guard;
    BlacklistBasic public blacklist;

    address public owner = address(1);
    address public bridgeOperator = address(2);
    address public tokenAdmin = address(3);
    address public user = address(4);

    function setUp() public {
        vm.prank(owner);
        accessManager = new AccessManager(owner);

        chainRegistry = new ChainRegistry(address(accessManager));
        tokenRegistry = new TokenRegistry(address(accessManager), chainRegistry);
        mintBurn = new MintBurn(address(accessManager));
        lockUnlock = new LockUnlock(address(accessManager));
        bridge = new Cl8YBridge(address(accessManager), tokenRegistry, mintBurn, lockUnlock);
        weth = IWETH(address(new MockWETH()));

        datastore = new DatastoreSetAddress();
        guard = new GuardBridge(address(accessManager), datastore);
        blacklist = new BlacklistBasic(address(accessManager));

        // Grant roles
        vm.startPrank(owner);
        accessManager.grantRole(1, bridgeOperator, 0);
        // Allow this test contract to call restricted functions configured for role 1
        accessManager.grantRole(1, address(this), 0);
        accessManager.grantRole(1, address(bridge), 0);
        accessManager.grantRole(1, address(mintBurn), 0);
        accessManager.grantRole(1, address(lockUnlock), 0);

        bytes4[] memory bridgeSelectors = new bytes4[](2);
        bridgeSelectors[0] = bridge.deposit.selector;
        bridgeSelectors[1] = bridge.withdraw.selector;
        accessManager.setTargetFunctionRole(address(bridge), bridgeSelectors, 1);

        // Allow router pause/unpause only
        router = new BridgeRouter(address(accessManager), bridge, tokenRegistry, mintBurn, lockUnlock, weth, guard);
        bytes4[] memory routerSelectors = new bytes4[](2);
        routerSelectors[0] = router.pause.selector;
        routerSelectors[1] = router.unpause.selector;
        accessManager.setTargetFunctionRole(address(router), routerSelectors, 1);

        // Allow mint/burn/lock/unlock
        bytes4[] memory mb = new bytes4[](2);
        mb[0] = mintBurn.mint.selector;
        mb[1] = mintBurn.burn.selector;
        accessManager.setTargetFunctionRole(address(mintBurn), mb, 1);
        bytes4[] memory lu = new bytes4[](2);
        lu[0] = lockUnlock.lock.selector;
        lu[1] = lockUnlock.unlock.selector;
        accessManager.setTargetFunctionRole(address(lockUnlock), lu, 1);

        // Allow Blacklist admin
        bytes4[] memory bl = new bytes4[](2);
        bl[0] = blacklist.setIsBlacklistedToTrue.selector;
        bl[1] = blacklist.setIsBlacklistedToFalse.selector;
        accessManager.setTargetFunctionRole(address(blacklist), bl, 1);

        // Allow GuardBridge module mgmt
        bytes4[] memory gb = new bytes4[](3);
        gb[0] = guard.addGuardModuleAccount.selector;
        gb[1] = guard.addGuardModuleDeposit.selector;
        gb[2] = guard.addGuardModuleWithdraw.selector;
        accessManager.setTargetFunctionRole(address(guard), gb, 1);
        vm.stopPrank();

        // Configure registry function roles so this test (role 1) can call them
        vm.startPrank(owner);
        bytes4[] memory cr = new bytes4[](1);
        cr[0] = chainRegistry.addEVMChainKey.selector;
        accessManager.setTargetFunctionRole(address(chainRegistry), cr, 1);

        bytes4[] memory tr = new bytes4[](2);
        tr[0] = tokenRegistry.addToken.selector;
        tr[1] = tokenRegistry.addTokenDestChainKey.selector;
        accessManager.setTargetFunctionRole(address(tokenRegistry), tr, 1);
        vm.stopPrank();

        // Register WETH in registry and add guard modules using role 1 (this test contract)
        vm.startPrank(address(this));
        chainRegistry.addEVMChainKey(1);
        tokenRegistry.addToken(address(weth), TokenRegistry.BridgeTypeLocal.LockUnlock);
        tokenRegistry.addTokenDestChainKey(
            address(weth), chainRegistry.getChainKeyEVM(1), bytes32(uint256(uint160(address(0x1234)))), 18
        );
        guard.addGuardModuleAccount(address(blacklist));
        guard.addGuardModuleDeposit(address(blacklist));
        guard.addGuardModuleWithdraw(address(blacklist));
        vm.stopPrank();
    }

    function test_Deposit_RevertsWhenSenderBlacklisted() public {
        address[] memory arr = new address[](1);
        arr[0] = user;
        blacklist.setIsBlacklistedToTrue(arr);
        // Precompute args so expectRevert applies to the router call itself
        bytes32 evmKey = chainRegistry.getChainKeyEVM(1);
        bytes32 destAccount = bytes32(uint256(uint160(user)));
        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(BlacklistBasic.Blacklisted.selector, user));
        router.deposit(address(weth), 1, evmKey, destAccount);
    }

    function test_Withdraw_RevertsWhenToBlacklisted() public {
        address[] memory arr = new address[](1);
        arr[0] = user;
        blacklist.setIsBlacklistedToTrue(arr);
        // Precompute args so expectRevert applies to the router call itself
        bytes32 evmKey = chainRegistry.getChainKeyEVM(1);
        Cl8YBridge.Withdraw memory w = Cl8YBridge.Withdraw({
            srcChainKey: evmKey,
            token: address(weth),
            destAccount: bytes32(uint256(uint160(user))),
            to: user,
            amount: 1,
            nonce: 1
        });
        bytes32 h = bridge.getWithdrawHash(w);
        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(BlacklistBasic.Blacklisted.selector, user));
        router.withdraw(h);
    }

    function test_DepositNative_RevertsWhenSenderBlacklisted() public {
        address[] memory arr = new address[](1);
        arr[0] = user;
        blacklist.setIsBlacklistedToTrue(arr);
        vm.deal(user, 1 ether);
        // Precompute args so expectRevert applies to the router call itself
        bytes32 evmKey = chainRegistry.getChainKeyEVM(1);
        bytes32 destAccount = bytes32(uint256(uint160(user)));
        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(BlacklistBasic.Blacklisted.selector, user));
        router.depositNative{value: 1}(evmKey, destAccount);
    }
}
