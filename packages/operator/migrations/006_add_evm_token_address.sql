-- Add evm_token_address column to terra_deposits table
-- This stores the corresponding EVM token address for Terra tokens
-- allowing the EVM writer to create correct approvals

ALTER TABLE terra_deposits 
ADD COLUMN IF NOT EXISTS evm_token_address VARCHAR(66) DEFAULT NULL;

-- For native denoms like 'uluna', this will be populated from the Terra bridge config
-- For CW20 tokens, this can be the mapped EVM token address
