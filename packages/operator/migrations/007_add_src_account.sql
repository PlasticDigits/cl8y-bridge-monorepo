-- Add src_account column for EVM→EVM hash computation
-- The src_account stores the depositor's address as 32-byte universal address,
-- which is required for computing the correct transfer hash in EVM→EVM transfers.

ALTER TABLE evm_deposits ADD COLUMN IF NOT EXISTS src_account BYTEA;

COMMENT ON COLUMN evm_deposits.src_account IS 'Source account (depositor) encoded as 32-byte universal address for V2 hash computation';

CREATE INDEX IF NOT EXISTS idx_evm_deposits_dest_chain_type ON evm_deposits(dest_chain_type)
    WHERE dest_chain_type IS NOT NULL;
