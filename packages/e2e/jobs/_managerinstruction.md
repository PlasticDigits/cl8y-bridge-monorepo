# Manager Instructions for Creating Job Files

This document explains how to create job files for WorkSplit when breaking down a feature into implementable chunks.

## CRITICAL: Job Files Are Requirements, Not Code

**Ollama generates the code. You provide requirements and references.**

DO NOT write full implementation code in job files. This defeats the purpose of using WorkSplit.

### What to Include in Job Files

| Include | Don't Include |
|---------|---------------|
| Requirements (what to implement) | Full implementation code |
| Function signatures as references | Every line of output verbatim |
| Code patterns/snippets to follow | Code you want copy-pasted |
| Constraints and error handling | Long code blocks (50+ lines) |

### Example: Good Job Content

```markdown
## Function: test_database_connectivity
- Connect to DATABASE_URL from config.operator.database_url
- Use tokio::time::timeout with 5s limit
- Return TestResult::pass/fail based on connection success
- Log connection status with tracing::info!

Pattern to follow:
```rust
match tokio::time::timeout(Duration::from_secs(5), connect()).await {
    Ok(Ok(_)) => TestResult::pass(...),
    _ => TestResult::fail(...),
}
```
```

Check `jobs/archive/` for examples of properly formatted job files.

---

## REQUIRED READING

Before creating jobs, read the **Success Rate by Job Type** table in README.md.
Edit mode has **20-50% success rate** for most use cases - prefer replace mode.

---

## CRITICAL: When to Use WorkSplit vs Direct Editing

**WorkSplit has overhead** (job creation, validation, verification, retries). Only use it when the cost savings outweigh this overhead.

### Cost Decision Matrix

| Task Size | Lines Changed | Recommendation | Reason |
|-----------|---------------|----------------|--------|
| Tiny | < 20 lines | **Direct edit** | Job overhead far exceeds savings |
| Small | 20-100 lines | **Direct edit** | Still faster to edit directly |
| Medium | 100-300 lines | **Evaluate** | Break-even zone; use WorkSplit for complex logic |
| Large | 300-500 lines | **WorkSplit** | Clear cost savings from free Ollama tokens |
| Very Large | 500+ lines | **WorkSplit strongly** | Significant savings; split into multiple jobs |

### Quick Decision Guide

```
STOP - Before creating a WorkSplit job, ask:

1. Is this < 100 lines of changes?
   → YES: Edit directly, don't use WorkSplit
   
2. Is this a simple, surgical change?
   → YES: Edit directly, WorkSplit overhead not worth it
   
3. Will this generate 300+ lines of NEW code?
   → YES: Use WorkSplit, clear savings
   
4. Is the logic complex enough to benefit from verification?
   → YES: Use WorkSplit
   → NO: Edit directly
```

---

## Quick Job Creation with Templates

**Preferred method**: Use `worksplit new-job` to scaffold job files quickly:

```bash
# Replace mode - generate a new file
worksplit new-job feature_001 --template replace -o src/services/ -f my_service.rs

# Edit mode - modify existing files  
worksplit new-job fix_001 --template edit --targets src/main.rs

# With context files
worksplit new-job impl_001 --template replace -c src/types.rs -o src/ -f api.rs

# Split mode - break large file into modules
worksplit new-job split_001 --template split --targets src/large_file.rs

# Sequential mode - multi-file with context accumulation
worksplit new-job big_001 --template sequential -o src/
```

After running, edit the generated `jobs/<name>.md` to add specific requirements.

### When to Use Each Template

| Template | Use When | Success Rate |
|----------|----------|--------------|
| `replace` | Creating new files or completely rewriting existing ones | ~95% |
| `edit` | Making 1-2 small changes to EXISTING code (not adding new code) | ~50-70% |
| `split` | A file exceeds 900 lines and needs to be modularized | ~90% |
| `sequential` | Generating multiple interdependent files | ~85% |
| `tdd` | You want tests generated before implementation | ~90% |

---

## CRITICAL: Edit Mode Limitations

Edit mode has a **high failure rate**. Before using it, complete this checklist:

### Edit Mode Checklist

```
STOP - Before using edit mode, ask:

1. Am I EDITING existing code or ADDING new code?
   - Adding new structs/functions/impl blocks → Use REPLACE mode
   - Modifying existing lines only → Edit mode MAY work

2. How many lines total am I changing?
   - < 10 lines → Do it MANUALLY (faster than job creation)
   - 10-50 lines in ONE location → Edit mode okay
   - > 50 lines → Use REPLACE mode

3. Are my changes isolated or interconnected?
   - Interconnected (struct + impl + tests) → Use REPLACE mode
   - Single isolated change → Edit mode okay

4. How many FIND/REPLACE blocks will this need?
   - 1-2 blocks → Edit mode okay (~70% success)
   - 3-5 blocks → Edit mode risky (~50% success)
   - 5+ blocks → Use REPLACE mode (edit WILL fail)

5. Am I modifying multiple files?
   - YES → Use REPLACE mode or separate jobs (edit ~30% success)
   - NO → Continue
```

### Edit Mode Failure Recovery

If edit mode fails:

1. **Do NOT retry edit mode more than once**
2. **Switch to replace mode** - regenerate the entire file
3. **Or do it manually** - often faster for small changes

Common edit mode failure causes:
- Too many FIND/REPLACE blocks
- Adding new code instead of editing existing code
- Interconnected changes across multiple locations
- Whitespace/indentation mismatches

---

## The 1 Job = 1 File Rule

**CRITICAL: Each job creates or replaces exactly ONE file.**

Never ask a job to "implement some functions" or "modify part of a file". This fails because:
- The LLM regenerates the entire file anyway
- Auto-fix often reverts partial changes
- Context gets lost between what to keep vs change

### Correct Approaches

| Scenario | Solution |
|----------|----------|
| Implement 5 of 25 functions | Regenerate entire file with 5 implemented |
| Add code to existing file | Replace mode with full file content |
| Modify multiple files | Create separate jobs (1 per file) |
| Large file (500+ lines) | Split into modules first, then 1 job per module |

### Example: Implementing Tests Incrementally

```
# BAD - Partial modification
Job: "Implement tests 6-10 in stubs.rs, keep tests 1-5 and 11-25 unchanged"
Result: LLM forgets context, reverts to stubs

# GOOD - Full file replacement
Job: "Create stubs.rs with 25 tests. Tests 1-10 implemented, tests 11-25 as stubs"
Result: Complete file generated correctly
```

## Job File Format

Each job file uses YAML frontmatter followed by markdown instructions:

```markdown
---
context_files:
  - src/models/user.rs
  - src/db/connection.rs
output_dir: src/services/
output_file: user_service.rs
---

# Create User Service

## Requirements
- Implement UserService struct
- Add CRUD methods for User model

## Methods to Implement
- `new(db: DbConnection) -> Self`
- `create_user(user: NewUser) -> Result<User, ServiceError>`
```

## Frontmatter Fields

| Field | Required | Description |
|-------|----------|-------------|
| `context_files` | No | List of files to include as context (max 2, each under 1000 lines) |
| `output_dir` | Yes | Directory where the output file will be created |
| `output_file` | Yes | Name of the generated file (default if multi-file output is used) |
| `output_files` | No | List of files to generate in sequential mode |
| `sequential` | No | Enable sequential mode (one LLM call per file) |
| `mode` | No | Output mode: "replace" (default) or "edit" for surgical changes |
| `target_files` | No | Files to edit when using edit mode |

## Output Modes

### 1. Replace Mode (Default) - PREFERRED

Standard mode that generates complete files. **Use this for most cases.**

### 2. Edit Mode (Surgical Changes) - USE WITH CAUTION

For making small, surgical changes to existing files. **Read the checklist above first.**

```markdown
---
mode: edit
target_files:
  - src/main.rs
output_dir: src/
output_file: main.rs
---

# Add New CLI Flag

Add the `--verbose` flag to the run command.
```

### 3. Split Mode (Breaking Up Large Files)

For splitting a large file into a directory-based module structure:

```markdown
---
mode: split
target_file: src/services/user_service.rs
output_dir: src/services/user_service/
output_file: mod.rs
output_files:
  - src/services/user_service/mod.rs
  - src/services/user_service/create.rs
  - src/services/user_service/query.rs
---
```

### 4. Sequential Multi-File

For bigger changes that exceed token limits:

```markdown
---
output_files:
  - src/main.rs
  - src/commands/run.rs
  - src/core/runner.rs
sequential: true
---
```

## Best Practices

### 1. Size Jobs Appropriately

Each job should generate **at most 900 lines of code**. If a feature requires more:
- Split into multiple jobs
- Each job handles one concern (model, service, API, etc.)
- Order jobs by dependency (use alphabetical naming)

### 2. Choose Context Files Wisely

Context files should:
- Define types the generated code will use
- Show patterns to follow (error handling, naming conventions)
- Contain interfaces to implement

### 3. Write Clear Instructions

Good instructions include:
- **What** to create (structs, functions, traits)
- **How** it should behave (expected logic, edge cases)
- **Why** (context helps the LLM make good decisions)

### 4. Naming Convention

```
feature_order_component.md

Examples:
- auth_001_user_model.md
- auth_002_password_hasher.md
- auth_003_session_service.md
```

This ensures jobs run in dependency order (alphabetically).

## Cost-Reduction Tools

WorkSplit provides several tools to catch issues early and reduce expensive retries:

### `worksplit preview <job>` - Preview Before Running

Show the full prompt that would be sent to Ollama without actually running the job.

```bash
worksplit preview my_job_001
```

**When to use**:
- Before running jobs with large context files
- To verify the prompt looks correct before spending LLM tokens
- When debugging why a job isn't generating expected output

**Output includes**:
- Job mode and output path
- Context files with line counts
- System prompt preview
- Job instructions
- Estimated token count

### `worksplit lint [--job <job>]` - Check Generated Code

Run linters on generated code immediately after generation.

```bash
# Lint a specific job's output
worksplit lint --job my_job_001

# Lint all passed jobs
worksplit lint
```

**Requires** `lint_command` in `worksplit.toml`:
```toml
[build]
lint_command = "cargo clippy -- -D warnings"
```

**When to use**:
- After `worksplit run` completes to catch Rust errors
- Before committing generated code
- To verify code quality without manual review

### `worksplit fix <job>` - Auto-Fix Linter Errors

Automatically fix common linter issues using LLM.

```bash
worksplit fix my_job_001
```

**How it works**:
1. Runs the configured `lint_command` on the job's output
2. Sends linter output + source to LLM with `_systemprompt_fix.md`
3. LLM generates FIND/REPLACE blocks for mechanical fixes
4. Applies the fixes and re-runs linter to verify

**Best for fixing**:
- Unused variables (removes or prefixes with `_`)
- Dead code warnings (adds `#[allow(dead_code)]`)
- Missing imports
- Simple type errors

**Not suitable for**:
- Complex logic errors
- Design issues
- Anything requiring architectural decisions

### Recommended Workflow

```bash
# 1. Create and validate job
worksplit new-job feat_001 --template replace -o src/ -f my_module.rs
# (edit the job file to add requirements)
worksplit validate

# 2. Preview before running (optional but recommended for large jobs)
worksplit preview feat_001

# 3. Run the job
worksplit run --job feat_001

# 4. Check status
worksplit status

# 5. If passed, run linter
worksplit lint --job feat_001

# 6. If lint errors, auto-fix
worksplit fix feat_001

# 7. Verify fix worked
worksplit lint --job feat_001
```

---

# E2E Package Specific Instructions

This package migrates bash E2E scripts to Rust. See `/SPRINT14.md` for full context.

## File Types

**Only use WorkSplit for `.rs` files.** Write other files manually:
- `Cargo.toml` - write manually
- `README.md` - write manually  
- Config files (`.toml`, `.json`, `.yaml`) - write manually

## Job Naming Convention

Use prefix: `e2e_NNN_description.md`

- `e2e_001_xxx` through `e2e_003_xxx`: Batch 1 (foundation)
- `e2e_004_xxx` through `e2e_007_xxx`: Batch 2 (infrastructure)
- `e2e_008_xxx` through `e2e_012_xxx`: Batch 3-5 (interactions & tests)
- `e2e_013_xxx`: Batch 6 (CLI runner)

## Type Safety Requirements (CRITICAL)

These are the primary goals of this migration:

1. **All contract addresses MUST be typed as `Address`, not `String`**
2. **All chain keys MUST be typed as `B256`, not `String`**
3. **No `.unwrap()` calls** - use `?` or proper error handling with `eyre`
4. **All JSON parsing MUST use typed deserialization** with `serde`

## Crate Requirements

Use these specific crates:
- `alloy` for EVM interactions (replaces cast/forge calls)
- `cosmrs` + `tendermint-rpc` for Terra interactions
- `bollard` for Docker API
- `tokio` for async runtime
- `eyre` for error handling
- `tracing` for logging
- `serde` + `serde_json` for serialization
- `clap` for CLI parsing

## Bash → Rust Mapping Reference

| Bash | Rust |
|------|------|
| `cast call` | `provider.call()` |
| `cast send` | `provider.send_transaction()` |
| `jq` | `serde_json::from_str::<T>()` |
| `curl` | `reqwest::get()` |
| `sleep` | `tokio::time::sleep()` |
| `docker compose` | `bollard::Docker` |

---

# Lessons Learned from This Sprint

## WorkSplit-Specific Issues Encountered

### 1. Auto-Fix Can Corrupt Files
**Issue:** WorkSplit auto-fix (via `worksplit fix`) left `~~~` markdown code block markers in generated `.rs` files, causing compilation failures.

**Symptoms:**
- `error: unknown start of token` errors pointing to triple backticks
- Files ending with `~~~worksplit` or similar markers

**Prevention:**
- Always verify generated files after auto-fix runs
- Check the last few lines of generated files for stray markers
- If markers found, manually remove them

**Fix:** Simple string replacement to remove `~~~` lines at end of file.

### 2. sol! Macro Creates Conflicting Type Names
**Issue:** The `alloy::sol!` macro generates struct types from contract definitions. If you also define custom wrapper structs with the same names (e.g., `AccessManager`, `ChainRegistry`), you get `E0428: the name is defined multiple times`.

**Prevention:**
- Use `I` prefix convention for sol! contract names: `IBridge`, `IAccessManager`, `IChainRegistry`
- Use `Client` suffix for wrapper structs: `EvmBridgeClient`, `AccessManagerClient`
- Keep sol! macro types and wrapper structs clearly distinguished

### 3. Incomplete Match Patterns in Async Code
**Issue:** When using `tokio::time::timeout` with `Result<Result<T, E>>`, the generated code sometimes missed the `Ok(Ok(false))` pattern.

**Symptoms:**
- `error[E0004]: non-exhaustive patterns: Ok(Ok(false)) not covered`

**Fix:** Add explicit `Ok(Ok(false)) => Ok(false)` arm to match blocks.

### 4. Version Incompatibility with alloy
**Issue:** `alloy` version 0.9+ had `serde` compatibility issues (`could not find __private in serde`).

**Fix:** Pin to `alloy = "0.8"` which is more stable with current toolchain.

### 5. Unused Variable Warnings for Placeholder Tests
**Issue:** Generated test stubs had `config` and `start` variables that weren't used, triggering warnings.

**Prevention:** In job requirements, specify that placeholder tests should prefix unused params with `_`:
```rust
pub async fn test_placeholder(_config: &E2eConfig) -> TestResult {
    let _start = Instant::now();
    // TODO: Implement
}
```

## General Recommendations

1. **Review ALL generated files** - Don't trust WorkSplit status alone
2. **Check file endings** - Stray markdown markers are common
3. **Keep jobs focused** - Complex jobs with 5+ types are more likely to have issues
4. **Use explicit type annotations** - Helps LLM generate correct code
5. **Provide concrete examples** - Show expected function signatures in job requirements
