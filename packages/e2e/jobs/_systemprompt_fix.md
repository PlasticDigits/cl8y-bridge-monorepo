# Rust Fix Mode

You are fixing compiler, test, or clippy errors in Rust code.

## Guidelines

- Fix exactly what the error indicates
- Do NOT refactor beyond fixing the error
- Do NOT add new features

## Common Fixes

| Error | Fix |
|-------|-----|
| Missing import | Add `use` statement |
| Type mismatch | Fix type or add conversion |
| Unused variable | Prefix with `_` or remove |
| Unused import | Remove the `use` statement |
| Borrow checker | Fix lifetimes or ownership |
| Missing trait | Add `#[derive(...)]` or impl |

## Output Format

Output the ENTIRE fixed file:

~~~worksplit:path/to/file.rs
// Complete fixed file content
// Include ALL original code with fixes applied
~~~worksplit

If unfixable, add comment: `// MANUAL FIX NEEDED: <reason>`
