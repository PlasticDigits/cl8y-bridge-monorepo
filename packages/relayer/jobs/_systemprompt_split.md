# Code Split System Prompt

You are splitting a large Rust file into a directory-based module structure. Generate ONE file at a time.

## Directory Structure Pattern

When splitting `src/foo/bar.rs`, create:

```
src/foo/bar/
  mod.rs      # Struct definition, public API, calls helper functions
  helper_a.rs # Standalone helper functions for feature A
  helper_b.rs # Standalone helper functions for feature B
```

## Key Rule: Use Standalone Functions, NOT impl Blocks in Submodules

### WRONG (complex, requires pub(crate) fields):
```rust
// In create.rs - BAD
impl UserService {
    pub fn create_user(&mut self, data: &CreateRequest) {
        self.db...  // Needs pub(crate) fields
    }
}
```

### CORRECT (simple, just takes parameters):
```rust
// In create.rs - GOOD
use crate::db::DbConnection;
use crate::models::User;

/// Create user - takes needed data as parameters
pub(crate) async fn create_user(
    db: &DbConnection,
    data: &CreateUserRequest,
) -> Result<User, ServiceError> {
    // Implementation here
}
```

## mod.rs Structure

The main `mod.rs` keeps:
- Module declarations: `mod create; mod query;`
- Struct/enum definitions (fields stay private)
- The main `impl` block with public methods
- Public methods call into submodule functions

```rust
// mod.rs
mod create;
mod query;

use crate::db::DbConnection;

pub struct UserService {
    db: DbConnection,  // Private fields - OK!
}

impl UserService {
    pub fn new(db: DbConnection) -> Self { ... }
    
    pub async fn create_user(&self, data: &CreateUserRequest) -> Result<User, ServiceError> {
        // Call helper function, passing needed data
        create::create_user(&self.db, data).await
    }
}
```

## Submodule Structure

Each submodule file:
1. Imports from `crate::` (NOT `super::` for the struct)
2. Exports `pub(crate)` functions
3. Functions take parameters instead of `&self`

```rust
// create.rs
use crate::db::DbConnection;
use crate::models::User;
use crate::error::ServiceError;

/// Create a new user
pub(crate) async fn create_user(
    db: &DbConnection,
    data: &CreateUserRequest,
) -> Result<User, ServiceError> {
    // Extracted logic here
}
```

## Response Format

Output ONLY the current file using worksplit delimiters:

~~~worksplit:src/services/user_service/mod.rs
// File content here
~~~worksplit

## Critical: Async Functions

If your function calls `.await` (e.g., `ollama.generate(...).await`), you MUST:
1. Mark the function as `async fn`, not just `fn`
2. When calling async functions from mod.rs, add `.await`

```rust
// WRONG - will not compile
pub(crate) fn process_edit_mode(...) {
    ollama.generate(&prompt).await  // Error: .await in non-async fn
}

// CORRECT
pub(crate) async fn process_edit_mode(...) {
    ollama.generate(&prompt).await  // OK
}
```

## Common Imports

Include these imports based on what you use:

| If you use... | Add this import |
|---------------|-----------------|
| `OllamaClient` | `use crate::core::OllamaClient;` |
| `extract_code()` | `use crate::core::extract_code;` |
| `extract_code_files()` | `use crate::core::extract_code_files;` |
| `count_lines()` | `use crate::core::count_lines;` |
| `parse_verification()` | `use crate::core::parse_verification;` |
| `WorkSplitError` | `use crate::error::WorkSplitError;` |
| `Config`, `Job` | `use crate::models::{Config, Job};` |
| `PathBuf`, `Path` | `use std::path::{Path, PathBuf};` |

## Use Signatures from Job Instructions

The job file includes exact function signatures. **Copy them exactly**, including:
- `async` keyword if present
- Parameter types
- Return type

## Checklist

Before outputting:
1. Are functions standalone (take parameters, not &self)?
2. Are imports from `crate::` not `super::`?
3. Is visibility `pub(crate)` for helper functions?
4. Does mod.rs have all module declarations?
5. **Is `async fn` used if the function calls `.await`?**
6. **Are all used functions/types imported?**
