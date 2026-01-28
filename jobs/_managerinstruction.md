# Manager Instructions for Creating Job Files

This document explains how to create job files for WorkSplit when breaking down a Solidity/Foundry feature into implementable chunks.

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
# Replace mode - generate a new contract
worksplit new-job feature_001 --template replace -o src/ -f MyContract.sol

# Edit mode - modify existing contracts
worksplit new-job fix_001 --template edit --targets src/Token.sol

# With context files
worksplit new-job impl_001 --template replace -c src/interfaces/IToken.sol -o src/ -f Token.sol

# Split mode - break large contract into modules
worksplit new-job split_001 --template split --targets src/LargeContract.sol

# Sequential mode - multi-file with context accumulation
worksplit new-job big_001 --template sequential -o src/
```

After running, edit the generated `jobs/<name>.md` to add specific requirements.

### When to Use Each Template

| Template | Use When | Success Rate |
|----------|----------|--------------|
| `replace` | Creating new contracts or completely rewriting existing ones | ~95% |
| `edit` | Making 1-2 small changes to EXISTING code (not adding new code) | ~50-70% |
| `split` | A contract exceeds 500 lines and needs to be modularized | ~90% |
| `sequential` | Generating multiple interdependent contracts | ~85% |
| `tdd` | You want Foundry tests generated before implementation | ~90% |

---

## CRITICAL: Edit Mode Limitations

Edit mode has a **high failure rate**. Before using it, complete this checklist:

### Edit Mode Checklist

```
STOP - Before using edit mode, ask:

1. Am I EDITING existing code or ADDING new code?
   - Adding new functions/modifiers/events → Use REPLACE mode
   - Modifying existing lines only → Edit mode MAY work

2. How many lines total am I changing?
   - < 10 lines → Do it MANUALLY (faster than job creation)
   - 10-50 lines in ONE location → Edit mode okay
   - > 50 lines → Use REPLACE mode

3. Are my changes isolated or interconnected?
   - Interconnected (struct + function + modifier) → Use REPLACE mode
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
  - src/interfaces/IToken.sol
  - src/libraries/SafeMath.sol
output_dir: src/
output_file: Token.sol
---

# Create ERC20 Token

## Requirements
- Implement ERC20 standard
- Add minting capability for owner
- Include pause functionality

## Functions to Implement
- `mint(address to, uint256 amount) external onlyOwner`
- `pause() external onlyOwner`
- `unpause() external onlyOwner`
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
  - src/Token.sol
output_dir: src/
output_file: Token.sol
---

# Add Burn Function

Add a `burn(uint256 amount)` function to the Token contract.
```

### 3. Split Mode (Breaking Up Large Files)

For splitting a large contract into a modular structure:

```markdown
---
mode: split
target_file: src/LargeContract.sol
output_dir: src/modules/
output_file: Main.sol
output_files:
  - src/modules/Main.sol
  - src/modules/Storage.sol
  - src/modules/Logic.sol
---
```

### 4. Sequential Multi-File

For bigger changes that exceed token limits:

```markdown
---
output_files:
  - src/interfaces/IToken.sol
  - src/Token.sol
  - test/Token.t.sol
sequential: true
---
```

## Solidity-Specific Best Practices

### 1. Size Jobs Appropriately

Each job should generate **at most 900 lines of code**. Smart contracts should typically be:
- Single responsibility (one main purpose per contract)
- Interface + Implementation pattern
- Use inheritance to split large contracts

### 2. Choose Context Files Wisely

Context files should:
- Define interfaces the contract will implement
- Show library functions to use (OpenZeppelin, etc.)
- Contain base contracts to inherit from

### 3. Write Clear Instructions

Good instructions include:
- **What** to create (contract, interface, library)
- **Security requirements** (access control, reentrancy guards)
- **Events** to emit
- **Modifiers** to implement
- **Storage layout** considerations

### 4. Naming Convention

```
feature_order_component.md

Examples:
- token_001_interface.md
- token_002_base.md
- token_003_implementation.md
- token_004_tests.md
```

This ensures jobs run in dependency order (alphabetically).

### 5. Foundry Project Structure

Standard Foundry layout:
```
project/
├── src/                    # Contract source files
│   ├── interfaces/         # Interface definitions
│   ├── libraries/          # Library contracts
│   └── Token.sol           # Main contracts
├── test/                   # Foundry tests
│   └── Token.t.sol
├── script/                 # Deployment scripts
│   └── Deploy.s.sol
└── foundry.toml
```

## TDD Workflow

To enable Test-Driven Development with Foundry, add the `test_file` field:

```yaml
---
context_files: []
output_dir: src/
output_file: Token.sol
test_file: test/Token.t.sol
---
```

When `test_file` is specified:
1. Foundry tests are generated FIRST based on requirements
2. Implementation is then generated to pass tests
3. Implementation is verified against requirements

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
lint_command = "forge fmt --check"
```

**When to use**:
- After `worksplit run` completes to catch Solidity formatting/style issues
- Before committing generated contracts
- To verify code follows Foundry conventions

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
- Missing visibility modifiers (`public`, `private`, etc.)
- State mutability warnings (`view`, `pure`)
- SPDX license identifier issues
- Missing `override`/`virtual` keywords
- Unused variable warnings

**Not suitable for**:
- Security vulnerabilities
- Gas optimization issues
- Complex logic errors
- Architectural decisions

### Recommended Workflow

```bash
# 1. Create and validate job
worksplit new-job token_001 --template replace -o src/ -f Token.sol
# (edit the job file to add requirements)
worksplit validate

# 2. Preview before running (optional but recommended for large jobs)
worksplit preview token_001

# 3. Run the job
worksplit run --job token_001

# 4. Check status
worksplit status

# 5. If passed, run linter
worksplit lint --job token_001

# 6. If lint errors, auto-fix
worksplit fix token_001

# 7. Verify fix worked
worksplit lint --job token_001

# 8. Run Foundry tests
forge test
```
