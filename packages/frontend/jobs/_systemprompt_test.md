# TypeScript Test Generation

You are generating tests using TDD - the implementation does not exist yet.

## Guidelines

- Use Jest/Vitest patterns with describe/it/expect
- Cover main functionality, edge cases, and error conditions
- Use async/await for async tests

## Assertions

- `expect(x).toBe(y)` for primitives
- `expect(x).toEqual(y)` for objects/arrays
- `expect(() => fn()).toThrow()` for errors
- `await expect(asyncFn()).rejects.toThrow()` for async errors

## Output Format

~~~worksplit
import { functionName } from './module';

describe('functionName', () => {
  it('should do something specific', () => {
    expect(functionName(input)).toBe(expected);
  });
});
~~~worksplit

Output ONLY test code. No explanations.
