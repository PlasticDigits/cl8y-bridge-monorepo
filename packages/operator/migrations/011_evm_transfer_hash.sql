-- V2 cross-chain hash (matches compute_xchain_hash_id / Solana transfer_hash) for EVM-origin
-- deposits. Required by the Solana writer when joining evm_deposits to approvals.
ALTER TABLE evm_deposits ADD COLUMN IF NOT EXISTS transfer_hash BYTEA;

COMMENT ON COLUMN evm_deposits.transfer_hash IS
    'Keccak256 xchain hash ID (32 bytes) for V2 deposits; NULL for legacy V1 rows';
