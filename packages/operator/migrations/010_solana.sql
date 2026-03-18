CREATE TABLE IF NOT EXISTS solana_deposits (
    id BIGSERIAL PRIMARY KEY,
    nonce BIGINT NOT NULL UNIQUE,
    transfer_hash BYTEA NOT NULL,
    src_account BYTEA NOT NULL,
    dest_chain BYTEA NOT NULL,
    dest_account BYTEA NOT NULL,
    token BYTEA NOT NULL,
    amount NUMERIC NOT NULL,
    fee NUMERIC NOT NULL,
    slot BIGINT NOT NULL,
    signature TEXT NOT NULL,
    processed BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS solana_blocks (
    slot BIGINT PRIMARY KEY,
    block_hash TEXT NOT NULL,
    processed_at TIMESTAMPTZ DEFAULT NOW()
);
