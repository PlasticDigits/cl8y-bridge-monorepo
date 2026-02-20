-- Rename evm_token_address to dest_token_address in terra_deposits table.
-- The old column assumed a single EVM token address per Terra token, which is
-- incorrect for multi-chain deployments. The new column stores the per-chain
-- destination token address emitted by the Terra bridge's deposit event.

ALTER TABLE terra_deposits
RENAME COLUMN evm_token_address TO dest_token_address;
