#![allow(dead_code)]

use eyre::{Result, WrapErr};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;
use tracing::error;

pub mod models;

pub use models::*;

/// Create a database connection pool
pub async fn create_pool(database_url: &str) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .wrap_err("Failed to connect to database")
}

/// Run pending migrations (uses the migration files in migrations/)
pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .wrap_err("Failed to run database migrations")?;
    Ok(())
}

/// Insert a new EVM deposit
pub async fn insert_evm_deposit(pool: &PgPool, deposit: &NewEvmDeposit) -> Result<i64> {
    // Note: amount is stored as NUMERIC(78,0) in the database, so we cast the text value
    let row = sqlx::query(
        r#"
        INSERT INTO evm_deposits (chain_id, tx_hash, log_index, nonce, dest_chain_key, 
            dest_token_address, dest_account, token, amount, block_number, block_hash, 
            dest_chain_type, src_account, src_v2_chain_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::NUMERIC, $10, $11, $12, $13, $14)
        RETURNING id
        "#,
    )
    .bind(deposit.chain_id)
    .bind(&deposit.tx_hash)
    .bind(deposit.log_index)
    .bind(deposit.nonce)
    .bind(&deposit.dest_chain_key)
    .bind(&deposit.dest_token_address)
    .bind(&deposit.dest_account)
    .bind(&deposit.token)
    .bind(&deposit.amount)
    .bind(deposit.block_number)
    .bind(&deposit.block_hash)
    .bind(&deposit.dest_chain_type)
    .bind(&deposit.src_account)
    .bind(&deposit.src_v2_chain_id)
    .fetch_one(pool)
    .await
    .wrap_err("Failed to insert EVM deposit")?;

    Ok(row.get("id"))
}

/// Get pending EVM deposits (for creating releases on Terra)
pub async fn get_pending_evm_deposits(pool: &PgPool) -> Result<Vec<EvmDeposit>> {
    // Cast amount to TEXT since sqlx can't automatically convert NUMERIC to String
    let rows = sqlx::query_as::<_, EvmDeposit>(
        r#"SELECT id, chain_id, tx_hash, log_index, nonce, dest_chain_key, dest_token_address, 
                  dest_account, token, amount::TEXT as amount, block_number, block_hash, status, 
                  created_at, updated_at, dest_chain_id, dest_chain_type, src_account, src_v2_chain_id 
           FROM evm_deposits WHERE status = 'pending'"#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("SQL error getting pending EVM deposits: {:?}", e);
        e
    })
    .wrap_err("Failed to get pending EVM deposits")?;

    Ok(rows)
}

/// Get pending EVM deposits destined for Cosmos/Terra chains
pub async fn get_pending_evm_deposits_for_cosmos(pool: &PgPool) -> Result<Vec<EvmDeposit>> {
    // Filter by dest_chain_type = 'cosmos' to only get deposits going to Terra
    let rows = sqlx::query_as::<_, EvmDeposit>(
        r#"SELECT id, chain_id, tx_hash, log_index, nonce, dest_chain_key, dest_token_address, 
                  dest_account, token, amount::TEXT as amount, block_number, block_hash, status, 
                  created_at, updated_at, dest_chain_id, dest_chain_type, src_account, src_v2_chain_id 
           FROM evm_deposits WHERE status = 'pending' AND dest_chain_type = 'cosmos'"#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("SQL error getting pending EVM deposits for Cosmos: {:?}", e);
        e
    })
    .wrap_err("Failed to get pending EVM deposits for Cosmos")?;

    Ok(rows)
}

/// Get pending EVM deposits destined for EVM chains
pub async fn get_pending_evm_deposits_for_evm(pool: &PgPool) -> Result<Vec<EvmDeposit>> {
    // Filter by dest_chain_type = 'evm' to only get deposits going to EVM chains
    let rows = sqlx::query_as::<_, EvmDeposit>(
        r#"SELECT id, chain_id, tx_hash, log_index, nonce, dest_chain_key, dest_token_address, 
                  dest_account, token, amount::TEXT as amount, block_number, block_hash, status, 
                  created_at, updated_at, dest_chain_id, dest_chain_type, src_account, src_v2_chain_id 
           FROM evm_deposits WHERE status = 'pending' AND dest_chain_type = 'evm'"#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("SQL error getting pending EVM deposits for EVM: {:?}", e);
        e
    })
    .wrap_err("Failed to get pending EVM deposits for EVM")?;

    Ok(rows)
}

/// Find EVM deposit ID by source chain (V2 4-byte) and nonce for cosmos-bound deposits.
/// Used when Terra writer approves an EVM→Terra withdrawal to update shared state.
/// Uses src_v2_chain_id so it works with multiple EVM chains (BSC, opBNB, etc.).
pub async fn find_evm_deposit_id_by_src_v2_chain_nonce_for_cosmos(
    pool: &PgPool,
    src_v2_chain_id: &[u8; 4],
    nonce: i64,
) -> Result<Option<i64>> {
    let row = sqlx::query_as::<_, (i64,)>(
        r#"SELECT id FROM evm_deposits 
           WHERE src_v2_chain_id = $1 AND nonce = $2 AND dest_chain_type = 'cosmos' AND status = 'pending'
           LIMIT 1"#,
    )
    .bind(src_v2_chain_id)
    .bind(nonce)
    .fetch_optional(pool)
    .await
    .wrap_err("Failed to find EVM deposit by src_v2_chain_id and nonce")?;

    Ok(row.map(|r| r.0))
}

/// Update EVM deposit status
pub async fn update_evm_deposit_status(pool: &PgPool, id: i64, status: &str) -> Result<()> {
    sqlx::query(r#"UPDATE evm_deposits SET status = $1 WHERE id = $2"#)
        .bind(status)
        .bind(id)
        .execute(pool)
        .await
        .wrap_err_with(|| format!("Failed to update EVM deposit {} status to {}", id, status))?;

    Ok(())
}

/// Check if EVM deposit exists by tx_hash and log_index
pub async fn evm_deposit_exists(
    pool: &PgPool,
    chain_id: i64,
    tx_hash: &str,
    log_index: i32,
) -> Result<bool> {
    let row: (bool,) = sqlx::query_as(
        r#"SELECT EXISTS(SELECT 1 FROM evm_deposits WHERE chain_id = $1 AND tx_hash = $2 AND log_index = $3)"#,
    )
    .bind(chain_id)
    .bind(tx_hash)
    .bind(log_index)
    .fetch_one(pool)
    .await
    .wrap_err("Failed to check EVM deposit existence")?;

    Ok(row.0)
}

/// Insert a new Terra deposit
pub async fn insert_terra_deposit(pool: &PgPool, deposit: &NewTerraDeposit) -> Result<i64> {
    // Note: amount is stored as NUMERIC(78,0) in the database, so we cast the text value
    let row = sqlx::query(
        r#"
        INSERT INTO terra_deposits (tx_hash, nonce, sender, recipient, token, amount, dest_chain_id, block_height, evm_token_address)
        VALUES ($1, $2, $3, $4, $5, $6::NUMERIC, $7, $8, $9)
        RETURNING id
        "#,
    )
    .bind(&deposit.tx_hash)
    .bind(deposit.nonce)
    .bind(&deposit.sender)
    .bind(&deposit.recipient)
    .bind(&deposit.token)
    .bind(&deposit.amount)
    .bind(deposit.dest_chain_id)
    .bind(deposit.block_height)
    .bind(&deposit.evm_token_address)
    .fetch_one(pool)
    .await
    .wrap_err("Failed to insert Terra deposit")?;

    Ok(row.get("id"))
}

/// Get pending Terra deposits (for creating approvals on EVM)
pub async fn get_pending_terra_deposits(pool: &PgPool) -> Result<Vec<TerraDeposit>> {
    // Cast amount to TEXT since sqlx can't automatically convert NUMERIC to String
    let rows = sqlx::query_as::<_, TerraDeposit>(
        r#"SELECT id, tx_hash, nonce, sender, recipient, token, amount::TEXT as amount, 
                  dest_chain_id, block_height, status, created_at, updated_at, evm_token_address 
           FROM terra_deposits WHERE status = 'pending'"#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("SQL error getting pending Terra deposits: {:?}", e);
        e
    })
    .wrap_err("Failed to get pending Terra deposits")?;

    Ok(rows)
}

/// Update Terra deposit status
pub async fn update_terra_deposit_status(pool: &PgPool, id: i64, status: &str) -> Result<()> {
    sqlx::query(r#"UPDATE terra_deposits SET status = $1 WHERE id = $2"#)
        .bind(status)
        .bind(id)
        .execute(pool)
        .await
        .wrap_err_with(|| format!("Failed to update Terra deposit {} status to {}", id, status))?;

    Ok(())
}

/// Check if Terra deposit exists by tx_hash and nonce
pub async fn terra_deposit_exists(pool: &PgPool, tx_hash: &str, nonce: i64) -> Result<bool> {
    let row: (bool,) = sqlx::query_as(
        r#"SELECT EXISTS(SELECT 1 FROM terra_deposits WHERE tx_hash = $1 AND nonce = $2)"#,
    )
    .bind(tx_hash)
    .bind(nonce)
    .fetch_one(pool)
    .await
    .wrap_err("Failed to check Terra deposit existence")?;

    Ok(row.0)
}

/// Insert a new approval
pub async fn insert_approval(pool: &PgPool, approval: &NewApproval) -> Result<i64> {
    use tracing::{debug, warn};

    // Pre-validate fields that have VARCHAR limits in the schema
    if approval.token.len() > 42 {
        warn!(
            token_len = approval.token.len(),
            token = %approval.token,
            "Approval token exceeds VARCHAR(42) limit, truncating to 42 chars"
        );
    }
    if approval.recipient.len() > 42 {
        warn!(
            recipient_len = approval.recipient.len(),
            recipient = %approval.recipient,
            "Approval recipient exceeds VARCHAR(42) limit, truncating to 42 chars"
        );
    }
    if let Some(ref fr) = approval.fee_recipient {
        if fr.len() > 42 {
            warn!(
                fee_recipient_len = fr.len(),
                fee_recipient = %fr,
                "Approval fee_recipient exceeds VARCHAR(42) limit, truncating to 42 chars"
            );
        }
    }

    debug!(
        src_chain_key = %hex::encode(&approval.src_chain_key),
        nonce = approval.nonce,
        dest_chain_id = approval.dest_chain_id,
        withdraw_hash = %hex::encode(&approval.withdraw_hash),
        token = %approval.token,
        recipient = %approval.recipient,
        amount = %approval.amount,
        fee = %approval.fee,
        fee_recipient = ?approval.fee_recipient,
        "Inserting approval into database"
    );

    // Truncate fields to fit VARCHAR(42) — prevents silent DB errors
    let token = if approval.token.len() > 42 {
        &approval.token[..42]
    } else {
        &approval.token
    };
    let recipient = if approval.recipient.len() > 42 {
        &approval.recipient[..42]
    } else {
        &approval.recipient
    };
    let fee_recipient = approval.fee_recipient.as_ref().map(|fr| {
        if fr.len() > 42 {
            &fr[..42]
        } else {
            fr.as_str()
        }
    });

    // Note: amount and fee are stored as NUMERIC(78,0) in the database, so we cast the text values
    // Use ON CONFLICT to handle duplicate inserts gracefully:
    // - Stale DB from previous run (volumes not wiped)
    // - Previous attempt that was marked 'failed' (e.g., withdrawSubmit not called yet)
    // The upsert resets a failed approval to 'pending' with fresh data so it can be retried.
    let row = sqlx::query(
        r#"
        INSERT INTO approvals (src_chain_key, nonce, dest_chain_id, withdraw_hash, token, recipient, 
            amount, fee, fee_recipient, deduct_from_amount)
        VALUES ($1, $2, $3, $4, $5, $6, $7::NUMERIC, $8::NUMERIC, $9, $10)
        ON CONFLICT (src_chain_key, nonce, dest_chain_id) DO UPDATE SET
            withdraw_hash = EXCLUDED.withdraw_hash,
            token = EXCLUDED.token,
            recipient = EXCLUDED.recipient,
            amount = EXCLUDED.amount,
            fee = EXCLUDED.fee,
            fee_recipient = EXCLUDED.fee_recipient,
            deduct_from_amount = EXCLUDED.deduct_from_amount,
            status = 'pending',
            error_message = NULL,
            attempts = 0,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(&approval.src_chain_key)
    .bind(approval.nonce)
    .bind(approval.dest_chain_id)
    .bind(&approval.withdraw_hash)
    .bind(token)
    .bind(recipient)
    .bind(&approval.amount)
    .bind(&approval.fee)
    .bind(fee_recipient)
    .bind(approval.deduct_from_amount)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        // Log the full sqlx error for diagnostics
        error!(
            error = %e,
            error_debug = ?e,
            src_chain_key = %hex::encode(&approval.src_chain_key),
            nonce = approval.nonce,
            dest_chain_id = approval.dest_chain_id,
            withdraw_hash_len = approval.withdraw_hash.len(),
            token = %token,
            token_len = token.len(),
            recipient = %recipient,
            recipient_len = recipient.len(),
            amount = %approval.amount,
            fee = %approval.fee,
            "Database error inserting approval"
        );
        e
    })
    .wrap_err_with(|| {
        format!(
            "Failed to insert approval (src_chain_key={}, nonce={}, dest_chain_id={}, \
             withdraw_hash_len={}, token_len={}, recipient_len={})",
            hex::encode(&approval.src_chain_key),
            approval.nonce,
            approval.dest_chain_id,
            approval.withdraw_hash.len(),
            token.len(),
            recipient.len(),
        )
    })?;

    Ok(row.get("id"))
}

/// SQL SELECT columns for Approval table (casting NUMERIC to TEXT)
const APPROVAL_SELECT: &str = r#"id, src_chain_key, nonce, dest_chain_id, withdraw_hash, token, 
    recipient, amount::TEXT as amount, fee::TEXT as fee, fee_recipient, deduct_from_amount, 
    tx_hash, status, attempts, last_attempt_at, error_message, created_at, updated_at"#;

/// SQL SELECT columns for Release table (casting NUMERIC to TEXT)
const RELEASE_SELECT: &str = r#"id, src_chain_key, nonce, sender, recipient, token, 
    amount::TEXT as amount, source_chain_id, tx_hash, status, attempts, last_attempt_at, 
    error_message, created_at, updated_at"#;

/// Get pending approvals for submission
pub async fn get_pending_approvals(pool: &PgPool, dest_chain_id: i64) -> Result<Vec<Approval>> {
    let query = format!(
        "SELECT {} FROM approvals WHERE status = 'pending' AND dest_chain_id = $1",
        APPROVAL_SELECT
    );
    let rows = sqlx::query_as::<_, Approval>(&query)
        .bind(dest_chain_id)
        .fetch_all(pool)
        .await
        .wrap_err("Failed to get pending approvals")?;

    Ok(rows)
}

/// Update approval status with tx_hash
pub async fn update_approval_submitted(pool: &PgPool, id: i64, tx_hash: &str) -> Result<()> {
    sqlx::query(
        r#"UPDATE approvals SET status = 'submitted', tx_hash = $1, attempts = attempts + 1, last_attempt_at = NOW() WHERE id = $2"#,
    )
    .bind(tx_hash)
    .bind(id)
    .execute(pool)
    .await
    .wrap_err_with(|| format!("Failed to update approval {} as submitted", id))?;

    Ok(())
}

/// Update approval status to confirmed
pub async fn update_approval_confirmed(pool: &PgPool, id: i64) -> Result<()> {
    sqlx::query(r#"UPDATE approvals SET status = 'confirmed' WHERE id = $1"#)
        .bind(id)
        .execute(pool)
        .await
        .wrap_err_with(|| format!("Failed to update approval {} as confirmed", id))?;

    Ok(())
}

/// Update approval status to failed with error
pub async fn update_approval_failed(pool: &PgPool, id: i64, error: &str) -> Result<()> {
    sqlx::query(
        r#"UPDATE approvals SET status = 'failed', error_message = $1, attempts = attempts + 1, last_attempt_at = NOW() WHERE id = $2"#,
    )
    .bind(error)
    .bind(id)
    .execute(pool)
    .await
    .wrap_err_with(|| format!("Failed to update approval {} as failed", id))?;

    Ok(())
}

/// Check if a non-failed approval exists for (src_chain_key, nonce, dest_chain_id).
///
/// Only considers approvals that are NOT in 'failed' or 'rejected' status.
/// This prevents prematurely marking deposits as "processed" when a previous
/// approval attempt failed (e.g., due to withdrawSubmit not being called yet).
pub async fn approval_exists(
    pool: &PgPool,
    src_chain_key: &[u8],
    nonce: i64,
    dest_chain_id: i64,
) -> Result<bool> {
    let row: (bool,) = sqlx::query_as(
        r#"SELECT EXISTS(
            SELECT 1 FROM approvals
            WHERE src_chain_key = $1 AND nonce = $2 AND dest_chain_id = $3
              AND status NOT IN ('failed', 'rejected')
        )"#,
    )
    .bind(src_chain_key)
    .bind(nonce)
    .bind(dest_chain_id)
    .fetch_one(pool)
    .await
    .wrap_err("Failed to check approval existence")?;

    Ok(row.0)
}

/// Insert a new release
pub async fn insert_release(pool: &PgPool, release: &NewRelease) -> Result<i64> {
    // Note: amount is stored as NUMERIC(78,0) in the database, so we cast the text value
    let row = sqlx::query(
        r#"
        INSERT INTO releases (src_chain_key, nonce, sender, recipient, token, amount, source_chain_id)
        VALUES ($1, $2, $3, $4, $5, $6::NUMERIC, $7)
        RETURNING id
        "#,
    )
    .bind(&release.src_chain_key)
    .bind(release.nonce)
    .bind(&release.sender)
    .bind(&release.recipient)
    .bind(&release.token)
    .bind(&release.amount)
    .bind(release.source_chain_id)
    .fetch_one(pool)
    .await
    .wrap_err("Failed to insert release")?;

    Ok(row.get("id"))
}

/// Get pending releases for submission
pub async fn get_pending_releases(pool: &PgPool) -> Result<Vec<Release>> {
    let query = format!(
        "SELECT {} FROM releases WHERE status = 'pending'",
        RELEASE_SELECT
    );
    let rows = sqlx::query_as::<_, Release>(&query)
        .fetch_all(pool)
        .await
        .wrap_err("Failed to get pending releases")?;

    Ok(rows)
}

/// Update release status with tx_hash
pub async fn update_release_submitted(pool: &PgPool, id: i64, tx_hash: &str) -> Result<()> {
    sqlx::query(
        r#"UPDATE releases SET status = 'submitted', tx_hash = $1, attempts = attempts + 1, last_attempt_at = NOW() WHERE id = $2"#,
    )
    .bind(tx_hash)
    .bind(id)
    .execute(pool)
    .await
    .wrap_err_with(|| format!("Failed to update release {} as submitted", id))?;

    Ok(())
}

/// Update release status to confirmed
pub async fn update_release_confirmed(pool: &PgPool, id: i64) -> Result<()> {
    sqlx::query(r#"UPDATE releases SET status = 'confirmed' WHERE id = $1"#)
        .bind(id)
        .execute(pool)
        .await
        .wrap_err_with(|| format!("Failed to update release {} as confirmed", id))?;

    Ok(())
}

/// Update release status to failed with error
pub async fn update_release_failed(pool: &PgPool, id: i64, error: &str) -> Result<()> {
    sqlx::query(
        r#"UPDATE releases SET status = 'failed', error_message = $1, attempts = attempts + 1, last_attempt_at = NOW() WHERE id = $2"#,
    )
    .bind(error)
    .bind(id)
    .execute(pool)
    .await
    .wrap_err_with(|| format!("Failed to update release {} as failed", id))?;

    Ok(())
}

/// Check if release exists for (src_chain_key, nonce)
pub async fn release_exists(pool: &PgPool, src_chain_key: &[u8], nonce: i64) -> Result<bool> {
    let row: (bool,) = sqlx::query_as(
        r#"SELECT EXISTS(SELECT 1 FROM releases WHERE src_chain_key = $1 AND nonce = $2)"#,
    )
    .bind(src_chain_key)
    .bind(nonce)
    .fetch_one(pool)
    .await
    .wrap_err("Failed to check release existence")?;

    Ok(row.0)
}

/// Get last processed EVM block
pub async fn get_last_evm_block(pool: &PgPool, chain_id: i64) -> Result<Option<i64>> {
    let row: Option<(i64,)> =
        sqlx::query_as(r#"SELECT last_processed_block FROM evm_blocks WHERE chain_id = $1"#)
            .bind(chain_id)
            .fetch_optional(pool)
            .await
            .wrap_err("Failed to get last EVM block")?;

    Ok(row.map(|r| r.0))
}

/// Update last processed EVM block
pub async fn update_last_evm_block(pool: &PgPool, chain_id: i64, block_number: i64) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO evm_blocks (chain_id, last_processed_block)
        VALUES ($1, $2)
        ON CONFLICT (chain_id) DO UPDATE SET last_processed_block = $2, updated_at = NOW()
        "#,
    )
    .bind(chain_id)
    .bind(block_number)
    .execute(pool)
    .await
    .wrap_err_with(|| format!("Failed to update last EVM block for chain {}", chain_id))?;

    Ok(())
}

/// Get last processed Terra block height
pub async fn get_last_terra_block(pool: &PgPool, chain_id: &str) -> Result<Option<i64>> {
    let row: Option<(i64,)> =
        sqlx::query_as(r#"SELECT last_processed_height FROM terra_blocks WHERE chain_id = $1"#)
            .bind(chain_id)
            .fetch_optional(pool)
            .await
            .wrap_err("Failed to get last Terra block")?;

    Ok(row.map(|r| r.0))
}

/// Update last processed Terra block height
pub async fn update_last_terra_block(
    pool: &PgPool,
    chain_id: &str,
    block_height: i64,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO terra_blocks (chain_id, last_processed_height)
        VALUES ($1, $2)
        ON CONFLICT (chain_id) DO UPDATE SET last_processed_height = $2, updated_at = NOW()
        "#,
    )
    .bind(chain_id)
    .bind(block_height)
    .execute(pool)
    .await
    .wrap_err_with(|| format!("Failed to update last Terra block for chain {}", chain_id))?;

    Ok(())
}

// ============ Sprint 4: Confirmation Tracking ============

/// Get submitted approvals for confirmation checking
pub async fn get_submitted_approvals(pool: &PgPool) -> Result<Vec<Approval>> {
    let rows = {
        let query = format!(
            "SELECT {} FROM approvals WHERE status = 'submitted'",
            APPROVAL_SELECT
        );
        sqlx::query_as::<_, Approval>(&query)
            .fetch_all(pool)
            .await
            .wrap_err("Failed to get submitted approvals")?
    };

    Ok(rows)
}

/// Get submitted releases for confirmation checking
pub async fn get_submitted_releases(pool: &PgPool) -> Result<Vec<Release>> {
    let query = format!(
        "SELECT {} FROM releases WHERE status = 'submitted'",
        RELEASE_SELECT
    );
    let rows = sqlx::query_as::<_, Release>(&query)
        .fetch_all(pool)
        .await
        .wrap_err("Failed to get submitted releases")?;

    Ok(rows)
}

/// Update approval to reorged status
pub async fn update_approval_reorged(pool: &PgPool, id: i64) -> Result<()> {
    sqlx::query(r#"UPDATE approvals SET status = 'reorged' WHERE id = $1"#)
        .bind(id)
        .execute(pool)
        .await
        .wrap_err_with(|| format!("Failed to update approval {} as reorged", id))?;

    Ok(())
}

/// Update release to reorged status
pub async fn update_release_reorged(pool: &PgPool, id: i64) -> Result<()> {
    sqlx::query(r#"UPDATE releases SET status = 'reorged' WHERE id = $1"#)
        .bind(id)
        .execute(pool)
        .await
        .wrap_err_with(|| format!("Failed to update release {} as reorged", id))?;

    Ok(())
}

// ============ Sprint 4: Retry System ============

/// Get failed approvals that are ready for retry
pub async fn get_failed_approvals_for_retry(
    pool: &PgPool,
    dest_chain_id: i64,
    max_attempts: i32,
) -> Result<Vec<Approval>> {
    let query = format!(
        "SELECT {} FROM approvals 
         WHERE status = 'failed' 
           AND dest_chain_id = $1
           AND attempts < $2
           AND (retry_after IS NULL OR retry_after <= NOW())
         ORDER BY created_at ASC
         LIMIT 10",
        APPROVAL_SELECT
    );
    let rows = sqlx::query_as::<_, Approval>(&query)
        .bind(dest_chain_id)
        .bind(max_attempts)
        .fetch_all(pool)
        .await
        .wrap_err("Failed to get failed approvals for retry")?;

    Ok(rows)
}

/// Get failed releases that are ready for retry
pub async fn get_failed_releases_for_retry(
    pool: &PgPool,
    max_attempts: i32,
) -> Result<Vec<Release>> {
    let query = format!(
        "SELECT {} FROM releases 
         WHERE status = 'failed' 
           AND attempts < $1
           AND (retry_after IS NULL OR retry_after <= NOW())
         ORDER BY created_at ASC
         LIMIT 10",
        RELEASE_SELECT
    );
    let rows = sqlx::query_as::<_, Release>(&query)
        .bind(max_attempts)
        .fetch_all(pool)
        .await
        .wrap_err("Failed to get failed releases for retry")?;

    Ok(rows)
}

/// Update approval to pending for retry with retry_after
pub async fn update_approval_for_retry(
    pool: &PgPool,
    id: i64,
    retry_after: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    sqlx::query(r#"UPDATE approvals SET status = 'pending', retry_after = $1 WHERE id = $2"#)
        .bind(retry_after)
        .bind(id)
        .execute(pool)
        .await
        .wrap_err_with(|| format!("Failed to update approval {} for retry", id))?;

    Ok(())
}

/// Update release to pending for retry with retry_after
pub async fn update_release_for_retry(
    pool: &PgPool,
    id: i64,
    retry_after: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    sqlx::query(r#"UPDATE releases SET status = 'pending', retry_after = $1 WHERE id = $2"#)
        .bind(retry_after)
        .bind(id)
        .execute(pool)
        .await
        .wrap_err_with(|| format!("Failed to update release {} for retry", id))?;

    Ok(())
}

// ============ Sprint 4: API/Status Queries ============

/// Count pending deposits across all chains (evm_deposits + terra_deposits).
/// Both writers use this shared DB; when Terra writer approves EVM→Terra it updates
/// evm_deposits, when EVM writer approves Terra→EVM it updates terra_deposits.
pub async fn count_pending_deposits(pool: &PgPool) -> Result<i64> {
    let evm: (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*) FROM evm_deposits WHERE status = 'pending'"#)
            .fetch_one(pool)
            .await
            .wrap_err("Failed to count pending EVM deposits")?;
    let terra: (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*) FROM terra_deposits WHERE status = 'pending'"#)
            .fetch_one(pool)
            .await
            .wrap_err("Failed to count pending Terra deposits")?;
    Ok(evm.0 + terra.0)
}

/// Count pending approvals
pub async fn count_pending_approvals(pool: &PgPool) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM approvals WHERE status = 'pending'"#)
        .fetch_one(pool)
        .await
        .wrap_err("Failed to count pending approvals")?;

    Ok(row.0)
}

/// Count submitted approvals
pub async fn count_submitted_approvals(pool: &PgPool) -> Result<i64> {
    let row: (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*) FROM approvals WHERE status = 'submitted'"#)
            .fetch_one(pool)
            .await
            .wrap_err("Failed to count submitted approvals")?;

    Ok(row.0)
}

/// Count pending releases
pub async fn count_pending_releases(pool: &PgPool) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM releases WHERE status = 'pending'"#)
        .fetch_one(pool)
        .await
        .wrap_err("Failed to count pending releases")?;

    Ok(row.0)
}

/// Count submitted releases
pub async fn count_submitted_releases(pool: &PgPool) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM releases WHERE status = 'submitted'"#)
        .fetch_one(pool)
        .await
        .wrap_err("Failed to count submitted releases")?;

    Ok(row.0)
}

// ============ Sprint 5: API Helper Functions ============

/// Get all pending and submitted approvals with pagination
pub async fn get_pending_and_submitted_approvals(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<Approval>> {
    let query = format!(
        "SELECT {} FROM approvals 
         WHERE status IN ('pending', 'submitted')
         ORDER BY created_at DESC
         LIMIT $1 OFFSET $2",
        APPROVAL_SELECT
    );
    let rows = sqlx::query_as::<_, Approval>(&query)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .wrap_err("Failed to get pending/submitted approvals")?;

    Ok(rows)
}

/// Get all pending and submitted releases with pagination
pub async fn get_pending_and_submitted_releases(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<Release>> {
    let query = format!(
        "SELECT {} FROM releases 
         WHERE status IN ('pending', 'submitted')
         ORDER BY created_at DESC
         LIMIT $1 OFFSET $2",
        RELEASE_SELECT
    );
    let rows = sqlx::query_as::<_, Release>(&query)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .wrap_err("Failed to get pending/submitted releases")?;

    Ok(rows)
}

/// Get approval by transaction hash
pub async fn get_approval_by_tx_hash(pool: &PgPool, tx_hash: &str) -> Result<Option<Approval>> {
    let query = format!(
        "SELECT {} FROM approvals WHERE tx_hash = $1",
        APPROVAL_SELECT
    );
    let row = sqlx::query_as::<_, Approval>(&query)
        .bind(tx_hash)
        .fetch_optional(pool)
        .await
        .wrap_err("Failed to get approval by tx_hash")?;

    Ok(row)
}

/// Get release by transaction hash
pub async fn get_release_by_tx_hash(pool: &PgPool, tx_hash: &str) -> Result<Option<Release>> {
    let query = format!("SELECT {} FROM releases WHERE tx_hash = $1", RELEASE_SELECT);
    let row = sqlx::query_as::<_, Release>(&query)
        .bind(tx_hash)
        .fetch_optional(pool)
        .await
        .wrap_err("Failed to get release by tx_hash")?;

    Ok(row)
}

/// Count total pending and submitted approvals
pub async fn count_pending_and_submitted_approvals(pool: &PgPool) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM approvals WHERE status IN ('pending', 'submitted')"#,
    )
    .fetch_one(pool)
    .await
    .wrap_err("Failed to count pending/submitted approvals")?;

    Ok(row.0)
}

/// Count total pending and submitted releases
pub async fn count_pending_and_submitted_releases(pool: &PgPool) -> Result<i64> {
    let row: (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*) FROM releases WHERE status IN ('pending', 'submitted')"#)
            .fetch_one(pool)
            .await
            .wrap_err("Failed to count pending/submitted releases")?;

    Ok(row.0)
}
