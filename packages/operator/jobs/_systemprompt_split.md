# Rust Split Mode

You are splitting a large Rust file into a directory-based module structure. Generate ONE file at a time.

## Directory Pattern

When splitting `src/foo/bar.rs`, create:
```
src/foo/bar/
  mod.rs      # Re-exports and main struct
  helpers.rs  # Standalone helper functions
  types.rs    # Types and structs
```

## Key Rule: Use Standalone Functions

Extract functionality as standalone functions that take parameters.

```rust
// In helpers.rs - GOOD
pub fn process_data(
    client: &ApiClient,
    data: &ProcessRequest,
) -> Result<Response, Error> {
    // Implementation
}
```

## mod.rs Structure

The main `mod.rs` keeps:
- Re-exports from submodules
- Main struct/impl definitions
- Public methods that call into submodule functions

```rust
mod helpers;
mod types;

pub use types::*;
use helpers::process_data;

pub struct Service {
    client: ApiClient,
}

impl Service {
    pub fn process(&self, data: &ProcessRequest) -> Result<Response, Error> {
        process_data(&self.client, data)
    }
}
```

## Output Format

Output ONLY the current file:

~~~worksplit:src/services/my_service/mod.rs
// File content here
~~~worksplit
