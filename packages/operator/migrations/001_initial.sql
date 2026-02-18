-- Initial schema for CL8Y Bridge Relayer

-- Deposits from EVM chains
CREATE TABLE evm_deposits (
    id SERIAL PRIMARY KEY,
    chain_id BIGINT NOT NULL,
    tx_hash VARCHAR(66) NOT NULL,
    log_index INTEGER NOT NULL,
    nonce BIGINT NOT NULL,
    dest_chain_key BYTEA NOT NULL,
    dest_token_address BYTEA NOT NULL,
    dest_account BYTEA NOT NULL,
    token VARCHAR(42) NOT NULL,
    amount NUMERIC(78, 0) NOT NULL,
    block_number BIGINT NOT NULL,
    block_hash VARCHAR(66) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (chain_id, tx_hash, log_index)
);

-- Deposits from Terra Classic
CREATE TABLE terra_deposits (
    id SERIAL PRIMARY KEY,
    tx_hash VARCHAR(64) NOT NULL,
    nonce BIGINT NOT NULL,
    sender VARCHAR(44) NOT NULL,
    recipient VARCHAR(42) NOT NULL,
    token VARCHAR(64) NOT NULL,
    amount NUMERIC(78, 0) NOT NULL,
    dest_chain_id BIGINT NOT NULL,
    block_height BIGINT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tx_hash, nonce)
);

-- Approval submissions to EVM chains
CREATE TABLE approvals (
    id SERIAL PRIMARY KEY,
    src_chain_key BYTEA NOT NULL,
    nonce BIGINT NOT NULL,
    dest_chain_id BIGINT NOT NULL,
    xchain_hash_id BYTEA NOT NULL,
    token VARCHAR(42) NOT NULL,
    recipient VARCHAR(42) NOT NULL,
    amount NUMERIC(78, 0) NOT NULL,
    fee NUMERIC(78, 0) NOT NULL DEFAULT 0,
    fee_recipient VARCHAR(42),
    deduct_from_amount BOOLEAN NOT NULL DEFAULT FALSE,
    tx_hash VARCHAR(66),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TIMESTAMPTZ,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (src_chain_key, nonce, dest_chain_id)
);

-- Release submissions to Terra Classic
CREATE TABLE releases (
    id SERIAL PRIMARY KEY,
    src_chain_key BYTEA NOT NULL,
    nonce BIGINT NOT NULL,
    sender VARCHAR(42) NOT NULL,
    recipient VARCHAR(44) NOT NULL,
    token VARCHAR(64) NOT NULL,
    amount NUMERIC(78, 0) NOT NULL,
    source_chain_id BIGINT NOT NULL,
    tx_hash VARCHAR(64),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TIMESTAMPTZ,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (src_chain_key, nonce)
);

-- Processed block tracking for EVM chains
CREATE TABLE evm_blocks (
    chain_id BIGINT PRIMARY KEY,
    last_processed_block BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Processed block tracking for Terra Classic
CREATE TABLE terra_blocks (
    chain_id VARCHAR(32) PRIMARY KEY,
    last_processed_height BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for efficient queries
CREATE INDEX idx_evm_deposits_status ON evm_deposits(status);
CREATE INDEX idx_evm_deposits_chain_nonce ON evm_deposits(chain_id, nonce);
CREATE INDEX idx_terra_deposits_status ON terra_deposits(status);
CREATE INDEX idx_terra_deposits_nonce ON terra_deposits(nonce);
CREATE INDEX idx_approvals_status ON approvals(status);
CREATE INDEX idx_approvals_src_nonce ON approvals(src_chain_key, nonce);
CREATE INDEX idx_releases_status ON releases(status);
CREATE INDEX idx_releases_src_nonce ON releases(src_chain_key, nonce);

-- Trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_evm_deposits_updated_at
    BEFORE UPDATE ON evm_deposits
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_terra_deposits_updated_at
    BEFORE UPDATE ON terra_deposits
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_approvals_updated_at
    BEFORE UPDATE ON approvals
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_releases_updated_at
    BEFORE UPDATE ON releases
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
