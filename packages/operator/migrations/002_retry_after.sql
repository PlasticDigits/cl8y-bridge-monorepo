-- Add retry_after column for delayed retry support

ALTER TABLE approvals ADD COLUMN IF NOT EXISTS retry_after TIMESTAMPTZ;
ALTER TABLE releases ADD COLUMN IF NOT EXISTS retry_after TIMESTAMPTZ;

-- Add index for efficient retry queries
CREATE INDEX IF NOT EXISTS idx_approvals_retry ON approvals(status, retry_after) 
    WHERE status IN ('pending', 'failed');
CREATE INDEX IF NOT EXISTS idx_releases_retry ON releases(status, retry_after) 
    WHERE status IN ('pending', 'failed');

COMMENT ON COLUMN approvals.retry_after IS 'Earliest time to retry this approval after a failure';
COMMENT ON COLUMN releases.retry_after IS 'Earliest time to retry this release after a failure';
