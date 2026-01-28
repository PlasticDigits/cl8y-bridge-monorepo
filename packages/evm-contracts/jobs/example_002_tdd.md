---
context_files: []
output_dir: src/
output_file: Vault.sol
test_file: test/Vault.t.sol
---

# Create Vault Contract (TDD Example)

This job demonstrates TDD workflow with Foundry - tests will be generated first!

## Requirements
- Create a vault contract that accepts ETH deposits
- Track balances per user
- Allow withdrawals of deposited ETH
- Prevent withdrawing more than deposited
- Use reentrancy protection

## Functions to Implement

1. `deposit() external payable` - Deposits ETH, updates user balance
2. `withdraw(uint256 amount) external` - Withdraws ETH, reverts if insufficient balance
3. `getBalance(address user) external view returns (uint256)` - Returns user's balance

## Events

- `Deposited(address indexed user, uint256 amount)`
- `Withdrawn(address indexed user, uint256 amount)`

## Expected Behavior

```solidity
vault.deposit{value: 1 ether}();  // Deposits 1 ETH
vault.getBalance(msg.sender);      // Returns 1 ether
vault.withdraw(0.5 ether);         // Withdraws 0.5 ETH
vault.withdraw(1 ether);           // Reverts: insufficient balance
```

## Security Requirements

- Use `ReentrancyGuard` from OpenZeppelin
- Use checks-effects-interactions pattern
- Validate all inputs
