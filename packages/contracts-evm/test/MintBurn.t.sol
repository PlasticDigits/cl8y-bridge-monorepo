// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test, console2} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {ERC20Burnable} from "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import {MintBurn} from "../src/MintBurn.sol";

contract MockMintableToken is ERC20, ERC20Burnable {
    address public minter;

    constructor(string memory name, string memory symbol) ERC20(name, symbol) {
        minter = msg.sender;
    }

    function mint(address to, uint256 amount) external {
        require(msg.sender == minter, "Not minter");
        _mint(to, amount);
    }

    function setMinter(address _minter) external {
        minter = _minter;
    }
}

contract MintBurnTest is Test {
    MintBurn public mintBurn;
    MockMintableToken public token;
    address public admin = address(1);
    address public bridge = address(2);
    address public user = address(3);

    function setUp() public {
        // Deploy MintBurn
        MintBurn implementation = new MintBurn();
        bytes memory initData = abi.encodeCall(MintBurn.initialize, (admin));
        ERC1967Proxy proxy = new ERC1967Proxy(address(implementation), initData);
        mintBurn = MintBurn(address(proxy));

        // Deploy token
        token = new MockMintableToken("Bridge Token", "BTK");

        // Give user some tokens (while we're still the minter)
        token.mint(user, 1000 ether);

        // Now set mintBurn as minter
        token.setMinter(address(mintBurn));

        // Authorize bridge
        vm.prank(admin);
        mintBurn.addAuthorizedCaller(bridge);
    }

    function test_Initialize() public view {
        assertEq(mintBurn.owner(), admin);
        assertTrue(mintBurn.isAuthorizedCaller(bridge));
        assertEq(mintBurn.VERSION(), 1);
    }

    function test_Mint() public {
        vm.prank(bridge);
        mintBurn.mint(user, address(token), 100 ether);

        assertEq(token.balanceOf(user), 1100 ether);
    }

    function test_Burn() public {
        vm.prank(user);
        token.approve(address(mintBurn), 100 ether);

        vm.prank(bridge);
        mintBurn.burn(user, address(token), 100 ether);

        assertEq(token.balanceOf(user), 900 ether);
    }

    function test_Mint_RevertsIfNotAuthorized() public {
        vm.prank(user);
        vm.expectRevert(MintBurn.Unauthorized.selector);
        mintBurn.mint(user, address(token), 100 ether);
    }

    function test_Burn_RevertsIfNotAuthorized() public {
        vm.prank(user);
        vm.expectRevert(MintBurn.Unauthorized.selector);
        mintBurn.burn(user, address(token), 100 ether);
    }

    function test_AddRemoveAuthorizedCaller() public {
        address newCaller = address(4);

        vm.prank(admin);
        mintBurn.addAuthorizedCaller(newCaller);
        assertTrue(mintBurn.isAuthorizedCaller(newCaller));

        vm.prank(admin);
        mintBurn.removeAuthorizedCaller(newCaller);
        assertFalse(mintBurn.isAuthorizedCaller(newCaller));
    }

    function test_Upgrade() public {
        MintBurn newImplementation = new MintBurn();
        vm.prank(admin);
        mintBurn.upgradeToAndCall(address(newImplementation), "");

        assertTrue(mintBurn.isAuthorizedCaller(bridge));
        assertEq(mintBurn.VERSION(), 1);
    }
}
