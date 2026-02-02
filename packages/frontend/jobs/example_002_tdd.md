---
context_files: []
output_dir: src/
output_file: calculator.ts
test_file: calculator.test.ts
---

# Create Calculator Module (TDD Example)

This job demonstrates TDD workflow - tests will be generated first!

## Requirements
- Create a calculator module with basic arithmetic operations
- Support add, subtract, multiply, divide functions
- Handle division by zero by throwing an error
- Use proper TypeScript types

## Functions to Implement

1. `add(a: number, b: number): number` - Returns sum
2. `subtract(a: number, b: number): number` - Returns difference
3. `multiply(a: number, b: number): number` - Returns product
4. `divide(a: number, b: number): number` - Returns quotient, throws on division by zero

## Expected Behavior

```typescript
add(2, 3)        // returns 5
subtract(5, 3)   // returns 2
multiply(4, 5)   // returns 20
divide(10, 2)    // returns 5
divide(10, 0)    // throws Error("Division by zero")
```

## Type Definitions

Consider creating:
```typescript
export type Operation = 'add' | 'subtract' | 'multiply' | 'divide';

export interface CalculatorResult {
  operation: Operation;
  operands: [number, number];
  result: number;
}
```
