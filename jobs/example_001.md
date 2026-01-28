---
context_files: []
output_dir: src/
output_file: Counter.sol
---

# Create Counter Contract

## Requirements
- Create a simple counter smart contract
- Allow incrementing and decrementing the count
- Allow setting the count to a specific value
- Emit events on state changes

## Functions to Implement

1. `increment() external` - Increases count by 1, emits `CountChanged`
2. `decrement() external` - Decreases count by 1, emits `CountChanged`
3. `setCount(uint256 _count) external` - Sets count to specific value, emits `CountChanged`
4. `getCount() external view returns (uint256)` - Returns current count

## Events

- `CountChanged(uint256 oldValue, uint256 newValue)`

## Example Usage

```solidity
Counter counter = new Counter();
counter.increment(); // count = 1
counter.increment(); // count = 2
counter.decrement(); // count = 1
counter.setCount(10); // count = 10
uint256 value = counter.getCount(); // returns 10
```
