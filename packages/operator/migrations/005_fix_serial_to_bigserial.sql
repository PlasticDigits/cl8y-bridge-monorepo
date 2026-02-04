-- Fix SERIAL to BIGSERIAL for i64 compatibility
--
-- The Rust models use i64 for id fields, but SERIAL creates INT4 (32-bit).
-- PostgreSQL requires the types to match exactly for sqlx decoding.
-- This migration converts SERIAL columns to BIGSERIAL (INT8/64-bit).

-- Note: ALTER TABLE ... ALTER COLUMN ... TYPE uses existing sequence values,
-- so this is safe for existing data.

-- Fix evm_deposits.id
ALTER TABLE evm_deposits ALTER COLUMN id TYPE BIGINT;
ALTER SEQUENCE evm_deposits_id_seq AS BIGINT;

-- Fix terra_deposits.id
ALTER TABLE terra_deposits ALTER COLUMN id TYPE BIGINT;
ALTER SEQUENCE terra_deposits_id_seq AS BIGINT;

-- Fix approvals.id
ALTER TABLE approvals ALTER COLUMN id TYPE BIGINT;
ALTER SEQUENCE approvals_id_seq AS BIGINT;

-- Fix releases.id
ALTER TABLE releases ALTER COLUMN id TYPE BIGINT;
ALTER SEQUENCE releases_id_seq AS BIGINT;
