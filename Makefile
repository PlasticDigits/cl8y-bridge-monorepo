.PHONY: start stop reset deploy operator test-transfer logs help status gitleaks gitleaks-scan setup-hooks

# Default target
help:
	@echo "CL8Y Bridge Development Commands"
	@echo ""
	@echo "Infrastructure:"
	@echo "  make start          - Start all services (Anvil, LocalTerra, PostgreSQL)"
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
	@echo "  make build-evm      - Build EVM contracts"
	@echo "  make build-terra    - Build Terra contracts"
	@echo "  make build-operator - Build operator"
	@echo "  make build          - Build all packages"
	@echo ""
	@echo "Testing:"
	@echo "  make test-evm       - Run EVM contract tests"
	@echo "  make test-terra     - Run Terra contract tests"
	@echo "  make test-operator  - Run operator tests"
	@echo "  make test-frontend  - Run frontend unit tests"
	@echo "  make test           - Run all tests"
	@echo "  make e2e-test       - Run E2E connectivity tests"
	@echo "  make e2e-test-full  - Run full E2E with transfers and services"
	@echo "  make e2e-test-transfers - Run full E2E with operator"
	@echo "  make e2e-test-canceler  - Run full E2E with canceler"
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

# Building
build-evm:
	cd packages/contracts-evm && forge build

build-terra:
	cd packages/contracts-terraclassic && cargo build --release --target wasm32-unknown-unknown

build-operator:
	cd packages/operator && cargo build

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
	./scripts/e2e-setup.sh

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

# E2E automated test
e2e-test:
	./scripts/e2e-test.sh

e2e-test-quick:
	./scripts/e2e-test.sh --quick

e2e-test-full:
	./scripts/e2e-test.sh --with-all --full

e2e-test-transfers:
	./scripts/e2e-test.sh --full --with-operator

e2e-test-canceler:
	./scripts/e2e-test.sh --full --with-canceler

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
