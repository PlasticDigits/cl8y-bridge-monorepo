.PHONY: start stop reset deploy operator test-transfer logs help status

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
	@echo "  make test           - Run all tests"
	@echo "  make e2e-test       - Run E2E watchtower pattern tests"

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

deploy-terra:
	@echo "Deploying Terra contracts to LocalTerra..."
	./scripts/deploy-terra-local.sh

deploy-terra-local: deploy-terra
	@echo "Terra local deployment complete"

setup-bridge:
	@echo "Configuring bridge connections..."
	./scripts/setup-bridge.sh

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

# Integration tests
test-integration:
	cd packages/operator && cargo test --test integration_test -- --nocapture

# Integration tests (with infrastructure)
test-integration-full:
	cd packages/operator && cargo test --test integration_test -- --ignored --nocapture

# Monitoring
start-monitoring:
	docker compose --profile monitoring up -d prometheus grafana
	@echo "Prometheus: http://localhost:9091"
	@echo "Grafana: http://localhost:3000 (admin/admin)"

stop-monitoring:
	docker compose --profile monitoring down

# WorkSplit
worksplit-init:
	cd packages/operator && worksplit init --lang rust --model worksplit-coder-glm-4.7:32k
	cd packages/contracts-evm && worksplit init --lang solidity --model worksplit-coder-glm-4.7:32k
	cd packages/contracts-terraclassic && worksplit init --lang rust --model worksplit-coder-glm-4.7:32k

worksplit-status:
	@echo "=== Operator ===" && cd packages/operator && worksplit status || true
	@echo "=== EVM Contracts ===" && cd packages/contracts-evm && worksplit status || true
	@echo "=== Terra Contracts ===" && cd packages/contracts-terraclassic && worksplit status || true
