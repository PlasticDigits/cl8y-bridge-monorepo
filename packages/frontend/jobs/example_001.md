---
context_files: []
output_dir: src/
output_file: hello.ts
---

# Create Hello World Module

## Requirements
- Create a simple TypeScript module with a greeting function
- The function should accept a name parameter
- Return a formatted greeting string
- Use proper TypeScript types

## Functions to Implement

1. `greet(name: string): string` - Returns "Hello, {name}!"
2. `greetWithTime(name: string, morning: boolean): string` - Returns appropriate greeting based on time

## Example Usage

```typescript
const greeting = greet("World");
// Returns: "Hello, World!"

const morningGreeting = greetWithTime("Alice", true);
// Returns: "Good morning, Alice!"
```

## Type Definitions

Consider exporting these types:
```typescript
export interface GreetingOptions {
  name: string;
  formal?: boolean;
}
```
