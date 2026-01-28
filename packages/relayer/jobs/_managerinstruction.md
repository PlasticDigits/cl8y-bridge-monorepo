# Manager Instructions for Creating Job Files

This document explains how to create job files for WorkSplit when breaking down a feature into implementable chunks.

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
