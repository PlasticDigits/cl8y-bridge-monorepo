-- Add V2 chain ID column to evm_deposits
-- This stores the 4-byte ChainRegistry chain ID (e.g., 0x00000001)
-- as opposed to the native chain ID (e.g., 31337 for Anvil).
-- The V2 chain ID is critical for computing correct transfer hashes
-- in cross-chain operations.
ALTER TABLE evm_deposits ADD COLUMN IF NOT EXISTS src_v2_chain_id BYTEA DEFAULT NULL;

-- Also add to approvals table for consistency
ALTER TABLE approvals ADD COLUMN IF NOT EXISTS src_v2_chain_id BYTEA DEFAULT NULL;
ALTER TABLE approvals ADD COLUMN IF NOT EXISTS dest_v2_chain_id BYTEA DEFAULT NULL;
