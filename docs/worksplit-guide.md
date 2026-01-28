# WorkSplit Guide

This guide explains how to use WorkSplit for AI-assisted code generation in the CL8Y Bridge project.

## Overview

[WorkSplit](https://github.com/PlasticDigits/WorkSplit) is a CLI tool that delegates code generation to a local Ollama LLM, minimizing the work required from the manager (human or AI assistant).

### When to Use WorkSplit

| Lines of Code | Approach |
|---------------|----------|
| < 50 lines | Direct edit (don't use WorkSplit) |
| 50-200 lines | Consider either |
| 200+ lines | Use WorkSplit (cost savings) |

**Use WorkSplit for:**
- New feature implementation
- Large refactoring
- Multi-file generation

**Don't use WorkSplit for:**
- Small fixes (<50 lines)
- Configuration files
- Simple shell scripts

## Project Setup

Each package has its own WorkSplit configuration:

```
packages/
├── contracts-evm/
│   ├── worksplit.toml
│   └── jobs/
├── contracts-terraclassic/
│   ├── worksplit.toml
│   └── jobs/
└── relayer/
    ├── worksplit.toml
    └── jobs/
```

### Initialization

```bash
# EVM Contracts (Solidity)
cd packages/contracts-evm
worksplit init --lang solidity --model worksplit-coder-glm-4.7:32k

# Terra Classic Contracts (Rust)
cd packages/contracts-terraclassic
worksplit init --lang rust --model worksplit-coder-glm-4.7:32k

# Relayer (Rust)
cd packages/relayer
worksplit init --lang rust --model worksplit-coder-glm-4.7:32k
```

### Configuration

Each package's `worksplit.toml`:

#### Rust (Relayer, Terra Contracts)

```toml
[ollama]
url = "http://localhost:11434"
model = "worksplit-coder-glm-4.7:32k"
timeout_seconds = 300

[limits]
max_output_lines = 900
max_context_lines = 1000
max_context_files = 2

[build]
build_command = "cargo check"
test_command = "cargo test"
lint_command = "cargo clippy -- -D warnings"
verify_build = true
verify_tests = true

[behavior]
stream_output = true
create_output_dirs = true
```

#### Solidity (EVM Contracts)

```toml
[ollama]
url = "http://localhost:11434"
model = "worksplit-coder-glm-4.7:32k"
timeout_seconds = 300

[limits]
max_output_lines = 900
max_context_lines = 1000
max_context_files = 2

[build]
build_command = "forge build"
test_command = "forge test"
lint_command = "forge fmt --check"
verify_build = true
verify_tests = true

[behavior]
stream_output = true
create_output_dirs = true
```

## Creating Jobs

### Using Templates

```bash
# New file (replace mode)
worksplit new-job feature_001_name --template replace -o src/ -f filename.rs

# Edit existing file
worksplit new-job fix_001_name --template edit --targets src/main.rs

# With context files
worksplit new-job impl_001_name --template replace -o src/ -f new.rs -c src/types.rs,src/utils.rs
```

### Job File Structure

```markdown
---
context_files:
  - src/types.rs
  - src/config.rs
output_dir: src/watchers/
output_file: evm.rs
verify: true
---

# Implement EVM Watcher

## Requirements
- Subscribe to DepositRequest events from CL8YBridge
- Parse event data into Deposit struct
- Store in PostgreSQL database
- Handle reconnection on RPC failures

## Signatures
\`\`\`rust
pub struct EvmWatcher {
    provider: Arc<Provider>,
    bridge_address: Address,
    db: PgPool,
}

impl EvmWatcher {
    pub async fn new(config: &Config, db: PgPool) -> Result<Self>;
    pub async fn run(&self) -> Result<()>;
    async fn process_event(&self, log: Log) -> Result<()>;
}
\`\`\`

## Constraints
- Use alloy for EVM interactions
- Use sqlx for database operations
- Use tracing for logging
- Handle errors with eyre
- No unwrap() calls
```

## Batching Strategy

Batch jobs by **file dependencies**, not by task:

### Relayer Example

```
Batch 1 (foundational, no dependencies):
├── relayer_001_types.md        → src/types.rs
├── relayer_002_config.md       → src/config.rs
├── relayer_003_db_models.md    → src/db/models.rs
└── relayer_004_db_mod.md       → src/db/mod.rs

Batch 2 (depends on Batch 1):
├── relayer_005_watchers_mod.md → src/watchers/mod.rs
├── relayer_006_watchers_evm.md → src/watchers/evm.rs
└── relayer_007_watchers_terra.md → src/watchers/terra.rs

Batch 3 (depends on Batches 1-2):
├── relayer_008_writers_mod.md  → src/writers/mod.rs
├── relayer_009_writers_evm.md  → src/writers/evm.rs
└── relayer_010_writers_terra.md → src/writers/terra.rs

Batch 4 (depends on all):
└── relayer_011_main.md         → src/main.rs
```

### Running Batches

```bash
cd packages/relayer

# Create batch 1 jobs
worksplit new-job relayer_001_types --template replace -o src/ -f types.rs
# ... create remaining jobs

# Run batch 1
worksplit run
worksplit status

# If all pass, create and run batch 2
# ...
```

## Workflow Commands

```bash
# Validate job files
worksplit validate

# Preview prompt before running
worksplit preview relayer_001_types

# Run all pending jobs
worksplit run

# Run specific job
worksplit run --job relayer_001_types

# Check status
worksplit status
worksplit status -v          # Verbose
worksplit status --summary   # One-line summary

# Reset failed job
worksplit reset relayer_001_types

# Retry failed job
worksplit retry relayer_001_types

# Lint generated code
worksplit lint

# Auto-fix lint errors
worksplit fix relayer_001_types
```

## Best Practices

### Writing Effective Jobs

**DO:**
- Specify exact function signatures
- List all requirements explicitly
- Include constraints (no unwrap, use specific crates)
- Reference context files for types

**DON'T:**
- Write vague requirements
- Assume context is obvious
- Create overly large jobs (>500 lines output)

### Example: Good Job File

```markdown
---
context_files:
  - src/types.rs
output_dir: src/db/
output_file: models.rs
---

# Database Models

## Requirements
- Define Deposit struct for EVM deposits
- Define TerraDeposit struct for Terra deposits
- Define Approval struct for withdrawal approvals
- Implement sqlx::FromRow for all structs

## Structs

### Deposit
\`\`\`rust
pub struct Deposit {
    pub id: i64,
    pub chain_id: i64,
    pub tx_hash: String,
    pub log_index: i32,
    pub nonce: i64,
    pub dest_chain_key: Vec<u8>,
    pub dest_token_address: Vec<u8>,
    pub dest_account: Vec<u8>,
    pub token: String,
    pub amount: BigDecimal,
    pub block_number: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
\`\`\`

## Constraints
- Use sqlx::FromRow derive macro
- Use chrono for timestamps
- Use bigdecimal for amounts
- All fields must be pub
```

### Example: Bad Job File

```markdown
# Create database models

Make structs for the database.
```

## Troubleshooting

### Job Fails Verification

1. Check `worksplit status -v` for error
2. Review generated code in output file
3. Edit job file to add more constraints
4. Run `worksplit retry <job_id>`

### Build Verification Fails

1. Check `build_command` in `worksplit.toml`
2. Ensure dependencies are in `Cargo.toml` / `package.json`
3. Check for missing imports in context files

### Edit Mode Issues

Edit mode has lower success rate (~50-70%). If failing:
1. Switch to replace mode
2. Or make manual edit directly

## Related Documentation

- [Local Development](./local-development.md) - Development environment setup
- [Relayer](./relayer.md) - Relayer architecture
- [WorkSplit README](https://github.com/PlasticDigits/WorkSplit) - Official documentation
