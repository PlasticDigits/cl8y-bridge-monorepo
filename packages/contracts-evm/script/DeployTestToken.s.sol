// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";

/// @title DeployTestToken - Deploys a test ERC20 token for integration testing
/// @notice Creates a bridged token with initial supply for testing EVM deposits
contract DeployTestToken is Script {
    function run() public {
        // Anvil default deployer private key
        uint256 deployerKey = 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;
        address deployer = vm.addr(deployerKey);

        console.log("Deployer:", deployer);

        vm.startBroadcast(deployerKey);

        // Deploy test token (using TokenCl8yBridged as ERC20)
        // This token has mint/burn capabilities controlled by AccessManager
        // For testing, we'll create a simple ERC20 instead
        TestToken token = new TestToken("Test LUNC", "tLUNC", 6);
        console.log("TestToken:", address(token));

        // Mint initial supply to deployer for testing
        uint256 initialSupply = 1_000_000 * 10 ** 6; // 1 million tokens with 6 decimals
        token.mint(deployer, initialSupply);
        console.log("Minted %s tokens to deployer", initialSupply);

        // Also mint to common test accounts
        address testAccount = 0x70997970C51812dc3A010C7d01b50e0d17dc79C8; // Anvil account #1
        token.mint(testAccount, initialSupply);
        console.log("Minted %s tokens to test account %s", initialSupply, testAccount);

        vm.stopBroadcast();

        // Output for scripts to parse
        console.log("=== Test Token Deployment Complete ===");
        console.log("TEST_TOKEN_ADDRESS=%s", address(token));
        console.log("TEST_TOKEN_DECIMALS=6");
        console.log("TEST_TOKEN_SYMBOL=tLUNC");
    }
}

/// @title TestToken - Simple ERC20 for testing
/// @notice A mintable ERC20 token for integration tests
contract TestToken {
    string public name;
    string public symbol;
    uint8 public decimals;
    uint256 public totalSupply;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    address public owner;

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);

    constructor(string memory _name, string memory _symbol, uint8 _decimals) {
        name = _name;
        symbol = _symbol;
        decimals = _decimals;
        owner = msg.sender;
    }

    function mint(address to, uint256 amount) external {
        require(msg.sender == owner, "Only owner");
        totalSupply += amount;
        balanceOf[to] += amount;
        emit Transfer(address(0), to, amount);
    }

    function burn(address from, uint256 amount) external {
        require(msg.sender == owner || msg.sender == from, "Not authorized");
        require(balanceOf[from] >= amount, "Insufficient balance");
        totalSupply -= amount;
        balanceOf[from] -= amount;
        emit Transfer(from, address(0), amount);
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        emit Approval(msg.sender, spender, amount);
        return true;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        return _transfer(msg.sender, to, amount);
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        uint256 allowed = allowance[from][msg.sender];
        if (allowed != type(uint256).max) {
            require(allowed >= amount, "Insufficient allowance");
            allowance[from][msg.sender] = allowed - amount;
        }
        return _transfer(from, to, amount);
    }

    function _transfer(address from, address to, uint256 amount) internal returns (bool) {
        require(balanceOf[from] >= amount, "Insufficient balance");
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        emit Transfer(from, to, amount);
        return true;
    }
}
