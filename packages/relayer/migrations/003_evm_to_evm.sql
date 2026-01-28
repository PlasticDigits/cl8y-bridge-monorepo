-- Add source/destination chain tracking for EVM-to-EVM transfers

-- Add source_chain_id to evm_deposits (already has chain_id which is the source)
-- Add dest_chain_id tracking for deposits
ALTER TABLE evm_deposits ADD COLUMN IF NOT EXISTS dest_chain_id BIGINT;

-- Create index for destination chain queries
CREATE INDEX IF NOT EXISTS idx_evm_deposits_dest_chain ON evm_deposits(dest_chain_id) 
    WHERE dest_chain_id IS NOT NULL;

-- Add dest_chain_type to distinguish EVM vs Cosmos destinations
ALTER TABLE evm_deposits ADD COLUMN IF NOT EXISTS dest_chain_type VARCHAR(10) DEFAULT 'cosmos';

COMMENT ON COLUMN evm_deposits.dest_chain_id IS 'Destination chain ID for EVM-to-EVM transfers';
COMMENT ON COLUMN evm_deposits.dest_chain_type IS 'Destination chain type: evm or cosmos';

-- Add source chain type tracking to approvals for EVM-to-EVM
ALTER TABLE approvals ADD COLUMN IF NOT EXISTS src_chain_type VARCHAR(10) DEFAULT 'cosmos';

COMMENT ON COLUMN approvals.src_chain_type IS 'Source chain type: evm or cosmos';
