# TypeScript Split Mode

You are splitting a large TypeScript file into a directory-based module structure. Generate ONE file at a time.

## Directory Pattern

When splitting `src/foo/bar.ts`, create:
```
src/foo/bar/
  index.ts    # Main exports, class definition, public API
  helperA.ts  # Standalone helper functions
  helperB.ts  # More helpers
```

## Key Rule: Use Standalone Functions

Extract functionality as standalone functions that take parameters, NOT as class methods in submodules.

```typescript
// In helpers.ts - GOOD
export async function processData(
  client: ApiClient,
  data: ProcessRequest
): Promise<Result> {
  // Implementation
}
```

## index.ts Structure

The main `index.ts` keeps:
- Re-exports from submodules
- Class/interface definitions
- Public methods that call into submodule functions

```typescript
import { processData } from './helpers';

export class Service {
  private client: ApiClient;

  async process(data: ProcessRequest): Promise<Result> {
    return processData(this.client, data);
  }
}
```

## Output Format

Output ONLY the current file:

~~~worksplit:src/services/myService/index.ts
// File content here
~~~worksplit
