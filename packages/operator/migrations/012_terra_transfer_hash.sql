-- V2 unified transfer hash for Terra-origin deposits (matches compute_xchain_hash_id / Solana transfer_hash).
-- Emitted as `xchain_hash_id` in Terra bridge wasm events; used by SolanaWriter for TerraClassic→Solana approvals.
ALTER TABLE terra_deposits ADD COLUMN IF NOT EXISTS transfer_hash BYTEA;

COMMENT ON COLUMN terra_deposits.transfer_hash IS
    '32-byte keccak xchain hash from Terra deposit event (xchain_hash_id attribute); required for Solana-destination routing.';
