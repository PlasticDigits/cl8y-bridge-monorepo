-- Fix recipient column length for cross-chain addresses
--
-- The recipient field in terra_deposits stores destination chain addresses which
-- can be hex-encoded (64 chars for 32-byte addresses). The original VARCHAR(42)
-- was sized for EVM addresses but cross-chain recipients may be longer.
--
-- Also fix the sender field in releases table which stores EVM addresses
-- but may receive longer hex-encoded addresses.

-- Expand terra_deposits.recipient to accommodate 64-char hex addresses + buffer
ALTER TABLE terra_deposits ALTER COLUMN recipient TYPE VARCHAR(128);

-- Expand releases.sender to be consistent (EVM sender addresses)
ALTER TABLE releases ALTER COLUMN sender TYPE VARCHAR(128);

-- Also expand releases.recipient for Terra addresses (44 chars but add buffer)
ALTER TABLE releases ALTER COLUMN recipient TYPE VARCHAR(128);
