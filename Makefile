.PHONY: start stop reset deploy operator test-transfer logs help status gitleaks gitleaks-scan setup-hooks fmt fmt-check lint

# Default target
help:
	@echo "CL8Y Bridge Development Commands"
	@echo ""
	@echo "Infrastructure:"
	@echo "  make start          - Start all services (Anvil, Anvil1, LocalTerra, PostgreSQL)"
	@echo "  make stop           - Stop all services"
	@echo "  make reset          - Stop and remove all volumes"
	@echo "  make status         - Check status of all services"
	@echo "  make logs           - View service logs"
	@echo ""
	@echo "Development:"
	@echo "  make deploy         - Deploy contracts to local chains"
	@echo "  make operator       - Run the bridge operator service"
	@echo "  make test-transfer  - Run a test crosschain transfer"
	@echo ""
	@echo "Building:"
	@echo "  make build-evm            - Build EVM contracts"
	@echo "  make build-terra          - Build Terra contracts (cargo)"
	@echo "  make build-terra-optimized - Build Terra WASM via Docker optimizer"
	@echo "  make build-operator       - Build operator"
	@echo "  make build                - Build all packages"
	@echo ""
	@echo "Formatting & Linting:"
	@echo "  make fmt            - Format all packages (Rust + Solidity)"
	@echo "  make fmt-check      - Check formatting without modifying files"
	@echo "  make lint           - Run clippy on all Rust packages"
	@echo "  make ci-check       - Run all CI checks (fmt + clippy)"
	@echo ""
	@echo "Testing:"
	@echo "  make test-evm       - Run EVM contract tests"
	@echo "  make test-terra     - Run Terra contract tests"
	@echo "  make test-operator  - Run operator tests"
	@echo "  make test-frontend  - Run frontend unit tests"
	@echo "  make test           - Run all unit tests"
	@echo ""
	@echo "Frontend E2E Testing:"
	@echo "  make test-frontend-e2e              - Run Playwright E2E tests"
	@echo "  make test-frontend-e2e-ui           - Open Playwright UI mode"
	@echo "  make test-frontend-e2e-headed       - Run E2E tests with visible browser"
	@echo "  make test-frontend-e2e-setup        - Setup E2E infrastructure only"
	@echo "  make test-frontend-e2e-teardown     - Teardown E2E infrastructure"
	@echo "  make test-frontend-integration-chains - Run vitest integration tests with real chains"
	@echo ""
	@echo "Bridge Verification Testing:"
	@echo "  make test-bridge-integration        - Vitest bridge tests (full transfer lifecycle)"
	@echo "  make test-e2e-verify                - Playwright verification (auto-submit UX + balance)"
	@echo ""
	@echo "E2E Testing (Bash - Legacy):"
	@echo "  make e2e-test           - MASTER TEST: Run ALL E2E tests (bash)"
	@echo "  make e2e-test-quick     - Quick connectivity tests only (bash)"
	@echo "  make e2e-test-transfers - Transfer tests with operator only (bash)"
	@echo "  make e2e-test-canceler  - Canceler fraud detection tests (bash)"
	@echo ""
	@echo "E2E Testing (Rust - Recommended):"
	@echo "  make e2e-full-rust      - RECOMMENDED: Build WASM + full atomic cycle (setup->test->teardown)"
	@echo "  make e2e-full-quick     - Quick connectivity tests in full cycle"
	@echo "  make e2e-setup-rust     - Start infrastructure only"
	@echo "  make e2e-test-rust      - Run all E2E tests"
	@echo "  make e2e-quick-rust     - Quick connectivity tests only"
	@echo "  make e2e-teardown-rust  - Teardown infrastructure only"
	@echo "  make e2e-status         - Show infrastructure status"
	@echo "  make e2e-single TEST=x  - Run single test by name"
	@echo ""
	@echo "Deployment:"
	@echo "  make deploy             - Deploy all contracts locally"
	@echo "  make deploy-test-token  - Deploy test ERC20 for integration tests"
	@echo "  make deploy-terra-cw20  - Deploy Terra bridge and CW20 token"
	@echo "  make deploy-tokens      - Deploy test tokens on both chains"
	@echo "  make register-tokens    - Register tokens on bridges"
	@echo "  make e2e-setup          - Full E2E infrastructure setup"
	@echo "  make e2e-setup-full     - E2E setup with tokens registered"
	@echo ""
	@echo "Security:"
	@echo "  make setup-hooks    - Configure git hooks for pre-commit checks"
	@echo "  make gitleaks       - Check staged changes for secrets"
	@echo "  make gitleaks-scan  - Scan entire repository for secrets"

# Infrastructure
start:
	docker-compose up -d
	@echo "Waiting for services to be healthy..."
	@sleep 5
	docker-compose ps

stop:
	docker-compose down

reset:
	docker-compose down -v

logs:
	docker-compose logs -f

status:
	./scripts/status.sh

logs-anvil:
	docker-compose logs -f anvil

logs-terra:
	docker-compose logs -f localterra

logs-postgres:
	docker-compose logs -f postgres

# Formatting
fmt:
	@echo "Formatting all packages..."
	cd packages/operator && cargo fmt
	cd packages/canceler && cargo fmt
	cd packages/contracts-terraclassic/bridge && cargo fmt
	cd packages/contracts-evm && forge fmt
	@echo "✅ All packages formatted"

fmt-check:
	@echo "Checking formatting..."
	@FAILED=0; \
	(cd packages/operator && cargo fmt --check) || FAILED=1; \
	(cd packages/canceler && cargo fmt --check) || FAILED=1; \
	(cd packages/contracts-terraclassic/bridge && cargo fmt --check) || FAILED=1; \
	(cd packages/contracts-evm && forge fmt --check) || FAILED=1; \
	if [ $$FAILED -eq 1 ]; then echo "❌ Formatting issues found. Run 'make fmt' to fix."; exit 1; fi
	@echo "✅ All formatting checks passed"

lint:
	@echo "Running clippy on all Rust packages..."
	cd packages/operator && cargo clippy -- -D warnings
	cd packages/canceler && cargo clippy -- -D warnings
	cd packages/contracts-terraclassic/bridge && cargo clippy -- -D warnings
	@echo "✅ All clippy checks passed"

ci-check: fmt-check lint
	@echo "✅ All CI checks passed (formatting + clippy)"

# Building
build-evm:
	cd packages/contracts-evm && forge build

build-terra:
	cd packages/contracts-terraclassic && cargo build --release --target wasm32-unknown-unknown -p bridge --features cosmwasm_1_2 && \
		mkdir -p artifacts && \
		cp target/wasm32-unknown-unknown/release/bridge.wasm artifacts/

build-terra-optimized:
	@echo "Building optimized Terra WASM via Docker (cosmwasm_1_2 + BankQuery::Supply)..."
	docker run --rm -v "$$(pwd)/packages/contracts-terraclassic":/code \
		--mount type=volume,source=cl8y_terra_cache,target=/target \
		--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
		--entrypoint /bin/sh \
		cosmwasm/workspace-optimizer:0.17.0 \
		-c '\
			set -e && \
			echo "Rust: $$(rustc --version)" && \
			cd /code && \
			RUSTFLAGS="-C link-arg=-s" cargo build --release -p bridge --lib --target wasm32-unknown-unknown --features cosmwasm_1_2 --target-dir=/target --locked && \
			RUSTFLAGS="-C link-arg=-s" cargo build --release -p faucet --lib --target wasm32-unknown-unknown --target-dir=/target --locked && \
			mkdir -p artifacts && \
			echo "Optimizing bridge.wasm ..." && \
			wasm-opt -Os --signext-lowering /target/wasm32-unknown-unknown/release/bridge.wasm -o artifacts/bridge.wasm && \
			echo "Optimizing faucet.wasm ..." && \
			wasm-opt -Os --signext-lowering /target/wasm32-unknown-unknown/release/faucet.wasm -o artifacts/faucet.wasm && \
			cd artifacts && sha256sum -- *.wasm | tee checksums.txt \
		'
	@echo "✅ Optimized WASM written to packages/contracts-terraclassic/artifacts/"

build-operator:
	cd packages/operator && cargo build

build-operator-release:
	cd packages/operator && cargo build --release

build-canceler-release:
	cd packages/canceler && cargo build --release

build: build-evm build-terra build-operator

# Testing
test-evm:
	cd packages/contracts-evm && forge test -vvv

test-terra:
	cd packages/contracts-terraclassic/bridge && cargo test

test-operator:
	cd packages/operator && cargo test

test-canceler:
	cd packages/canceler && cargo test

test-frontend:
	cd packages/frontend && npm run test:unit

test-frontend-integration:
	cd packages/frontend && npm run test:integration

test: test-evm test-terra test-operator test-canceler test-frontend

# Deployment - Local
deploy: deploy-evm deploy-terra setup-bridge
	@echo "Deployment complete!"

deploy-evm:
	@echo "Deploying EVM contracts to Anvil..."
	cd packages/contracts-evm && forge script script/DeployLocal.s.sol:DeployLocal \
		--broadcast \
		--rpc-url http://localhost:8545

deploy-test-token:
	@echo "Deploying test ERC20 token to Anvil..."
	cd packages/contracts-evm && forge script script/DeployTestToken.s.sol:DeployTestToken \
		--broadcast \
		--rpc-url http://localhost:8545
	@echo ""
	@echo "Set these in packages/frontend/.env.local:"
	@echo "  VITE_BRIDGE_TOKEN_ADDRESS=<address from output>"
	@echo "  VITE_LOCK_UNLOCK_ADDRESS=<LockUnlock from deploy-evm>"

deploy-terra:
	@echo "Deploying Terra contracts to LocalTerra..."
	./scripts/deploy-terra-local.sh

deploy-terra-local: deploy-terra
	@echo "Terra local deployment complete"

deploy-terra-cw20:
	@echo "Deploying Terra bridge and CW20 token to LocalTerra..."
	./scripts/deploy-terra-local.sh --cw20

deploy-tokens: deploy-test-token deploy-terra-cw20
	@echo "Test tokens deployed on both chains"

register-tokens:
	@echo "Registering test tokens on bridges..."
	./scripts/register-test-tokens.sh

setup-bridge:
	@echo "Configuring bridge connections..."
	./scripts/setup-bridge.sh

# Full E2E setup (infrastructure + contracts + tokens)
e2e-setup:
	cd packages/frontend && npx tsx src/test/e2e-infra/setup.ts

e2e-setup-full: e2e-setup deploy-tokens register-tokens
	@echo "Full E2E setup complete with tokens"

# Deployment - Testnet
deploy-evm-bsc-testnet:
	./scripts/deploy-evm-testnet.sh bsc

deploy-evm-opbnb-testnet:
	./scripts/deploy-evm-testnet.sh opbnb

deploy-terra-testnet:
	./scripts/deploy-terra-testnet.sh

# Deployment - Mainnet (DANGER!)
deploy-evm-bsc-mainnet:
	./scripts/deploy-evm-mainnet.sh bsc

deploy-evm-opbnb-mainnet:
	./scripts/deploy-evm-mainnet.sh opbnb

deploy-terra-mainnet:
	./scripts/deploy-terra-mainnet.sh

# Operator
operator:
	cd packages/operator && cargo run

operator-start:
	./scripts/operator-ctl.sh start

operator-stop:
	./scripts/operator-ctl.sh stop

operator-status:
	./scripts/operator-ctl.sh status

operator-migrate:
	cd packages/operator && sqlx migrate run

# Canceler
canceler:
	cd packages/canceler && cargo run

canceler-start:
	./scripts/canceler-ctl.sh start

canceler-stop:
	./scripts/canceler-ctl.sh stop

canceler-status:
	./scripts/canceler-ctl.sh status

# Test transfer
test-transfer:
	./scripts/test-transfer.sh

# E2E automated test - MASTER TEST (runs everything)
e2e-test:
	@echo "========================================"
	@echo "  CL8Y Bridge Master E2E Test Suite"
	@echo "========================================"
	@echo ""
	@echo "This runs ALL E2E tests including:"
	@echo "  - Infrastructure connectivity"
	@echo "  - Operator (started automatically)"
	@echo "  - Canceler (started automatically)"
	@echo "  - Real token transfers with balance verification"
	@echo "  - EVM → Terra transfers"
	@echo "  - Terra → EVM transfers"
	@echo "  - Fraud detection tests"
	@echo ""
	cd packages/e2e && cargo run --release -- full

# Quick connectivity tests only (no operator, no transfers)
e2e-test-quick:
	cd packages/e2e && cargo run --release -- full --quick

# Alias for master test
e2e-test-full: e2e-test

# Run only transfer tests (operator on, no canceler)
e2e-test-transfers:
	cd packages/e2e && cargo run --release -- full

# Run only canceler fraud detection tests
e2e-test-canceler:
	cd packages/e2e && cargo run --release -- full

# Individual real token transfer tests
e2e-evm-to-terra:
	@echo "Testing EVM → Terra transfer..."
	cd packages/e2e && cargo run --release -- run --test evm_to_terra_transfer

e2e-terra-to-evm:
	@echo "Testing Terra → EVM transfer..."
	cd packages/e2e && cargo run --release -- run --test terra_to_evm_transfer

# E2E without any services (connectivity tests only)
e2e-connectivity:
	cd packages/e2e && cargo run --release -- run --quick

# =============================================================================
# Rust E2E Package (replaces bash scripts)
# =============================================================================

# Build the Rust E2E test binary
e2e-build:
	cd packages/e2e && cargo build --release

# Full E2E setup using Rust package
e2e-setup-rust:
	cd packages/e2e && cargo run --release -- setup

# Run all E2E tests using Rust package
e2e-test-rust:
	cd packages/e2e && cargo run --release -- run

# Quick connectivity tests using Rust package
e2e-quick-rust:
	cd packages/e2e && cargo run --release -- run --quick

# Run E2E tests without Terra
e2e-test-no-terra:
	cd packages/e2e && cargo run --release -- run --no-terra

# Run a single E2E test
e2e-single:
	@echo "Usage: make e2e-single TEST=<test_name>"
	@echo "Example: make e2e-single TEST=evm_connectivity"
	@test -n "$(TEST)" && cd packages/e2e && cargo run --release -- run --test $(TEST) || true

# Teardown E2E infrastructure using Rust package
e2e-teardown-rust:
	cd packages/e2e && cargo run --release -- teardown

# Teardown but keep volumes for faster restart
e2e-teardown-keep:
	cd packages/e2e && cargo run --release -- teardown --keep-volumes

# Show E2E infrastructure status
e2e-status:
	cd packages/e2e && cargo run --release -- status

# Full E2E cycle: setup -> test -> teardown (atomic, teardown always runs)
# Builds ALL projects first so the latest code is deployed and run:
#   - EVM contracts (forge build, deployed via forge script)
#   - Terra WASM (docker optimizer, deployed via terrad)
#   - Operator service (release binary, spawned by E2E runner)
#   - Canceler service (release binary, spawned by E2E runner)
e2e-full-rust: build-evm build-terra-optimized build-operator-release build-canceler-release
	cd packages/e2e && cargo run --release -- full

# Full E2E cycle with quick tests only (for faster CI)
e2e-full-quick: build-evm build-terra-optimized build-operator-release build-canceler-release
	cd packages/e2e && cargo run --release -- full --quick

# Full E2E cycle keeping volumes (faster restart)
e2e-full-keep: build-evm build-terra-optimized build-operator-release build-canceler-release
	cd packages/e2e && cargo run --release -- full --keep-volumes

# Integration tests
test-integration:
	cd packages/operator && cargo test --test integration_test -- --nocapture

# Integration tests (with infrastructure)
test-integration-full:
	cd packages/operator && cargo test --test integration_test -- --ignored --nocapture

# Frontend integration tests (requires Anvil + LocalTerra)
test-frontend-all:
	cd packages/frontend && npm run test:run

test-frontend-coverage:
	cd packages/frontend && npm run test:coverage

# Frontend E2E tests (requires Anvil, Anvil1, LocalTerra)
test-frontend-e2e:
	cd packages/frontend && npx playwright test

test-frontend-e2e-ui:
	cd packages/frontend && npx playwright test --ui

test-frontend-e2e-headed:
	cd packages/frontend && npx playwright test --headed

test-frontend-e2e-setup:
	cd packages/frontend && npx tsx src/test/e2e-infra/setup.ts

test-frontend-e2e-teardown:
	cd packages/frontend && npx tsx src/test/e2e-infra/teardown.ts

# Frontend integration tests with real chains (vitest + globalSetup)
test-frontend-integration-chains:
	cd packages/frontend && npx vitest run --config vitest.config.integration.ts

# Bridge integration tests (Vitest - tests full transfer lifecycle via CLI)
test-bridge-integration:
	cd packages/frontend && npm run test:bridge

# Playwright verification tests (E2E - tests auto-submit UX with balance verification)
test-e2e-verify:
	cd packages/frontend && npm run test:e2e:verify

# Bundle analysis
analyze-bundle:
	@echo "Analyzing frontend bundle size..."
	cd packages/frontend && npm run build
	@echo ""
	@echo "Bundle analysis complete. Check dist/assets for chunk sizes."
	@ls -lh packages/frontend/dist/assets/*.js 2>/dev/null || echo "Run 'make build-frontend' first"

# Monitoring
start-monitoring:
	docker compose --profile monitoring up -d prometheus grafana
	@echo "Prometheus: http://localhost:9091"
	@echo "Grafana: http://localhost:3000 (admin/admin)"

stop-monitoring:
	docker compose --profile monitoring down

# Security - Gitleaks
setup-hooks:
	@echo "Setting up git hooks..."
	git config core.hooksPath .githooks
	@echo "✅ Git hooks configured. Pre-commit hook will now run gitleaks."

gitleaks:
	@echo "Running gitleaks on staged changes..."
	gitleaks protect --staged --config .gitleaks.toml --verbose

gitleaks-scan:
	@echo "Scanning entire repository for secrets..."
	gitleaks detect --config .gitleaks.toml --verbose

# WorkSplit
worksplit-init:
	cd packages/operator && worksplit init --lang rust --model worksplit-coder-glm-4.7:32k
	cd packages/contracts-evm && worksplit init --lang solidity --model worksplit-coder-glm-4.7:32k
	cd packages/contracts-terraclassic && worksplit init --lang rust --model worksplit-coder-glm-4.7:32k

worksplit-status:
	@echo "=== Operator ===" && cd packages/operator && worksplit status || true
	@echo "=== EVM Contracts ===" && cd packages/contracts-evm && worksplit status || true
	@echo "=== Terra Contracts ===" && cd packages/contracts-terraclassic && worksplit status || true
