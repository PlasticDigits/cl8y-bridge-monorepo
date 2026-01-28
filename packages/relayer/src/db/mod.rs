#![allow(dead_code)]

use eyre::{Result, WrapErr};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;

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
    let row = sqlx::query(
        r#"
        INSERT INTO evm_deposits (chain_id, tx_hash, log_index, nonce, dest_chain_key, 
            dest_token_address, dest_account, token, amount, block_number, block_hash)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
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
    .fetch_one(pool)
    .await
    .wrap_err("Failed to insert EVM deposit")?;

    Ok(row.get("id"))
}

/// Get pending EVM deposits (for creating releases on Terra)
pub async fn get_pending_evm_deposits(pool: &PgPool) -> Result<Vec<EvmDeposit>> {
    let rows = sqlx::query_as::<_, EvmDeposit>(
        r#"SELECT * FROM evm_deposits WHERE status = 'pending'"#,
    )
    .fetch_all(pool)
    .await
    .wrap_err("Failed to get pending EVM deposits")?;

    Ok(rows)
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
    let row = sqlx::query(
        r#"
        INSERT INTO terra_deposits (tx_hash, nonce, sender, recipient, token, amount, dest_chain_id, block_height)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
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
    .fetch_one(pool)
    .await
    .wrap_err("Failed to insert Terra deposit")?;

    Ok(row.get("id"))
}

/// Get pending Terra deposits (for creating approvals on EVM)
pub async fn get_pending_terra_deposits(pool: &PgPool) -> Result<Vec<TerraDeposit>> {
    let rows = sqlx::query_as::<_, TerraDeposit>(
        r#"SELECT * FROM terra_deposits WHERE status = 'pending'"#,
    )
    .fetch_all(pool)
    .await
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
    let row = sqlx::query(
        r#"
        INSERT INTO approvals (src_chain_key, nonce, dest_chain_id, withdraw_hash, token, recipient, 
            amount, fee, fee_recipient, deduct_from_amount)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id
        "#,
    )
    .bind(&approval.src_chain_key)
    .bind(approval.nonce)
    .bind(approval.dest_chain_id)
    .bind(&approval.withdraw_hash)
    .bind(&approval.token)
    .bind(&approval.recipient)
    .bind(&approval.amount)
    .bind(&approval.fee)
    .bind(&approval.fee_recipient)
    .bind(approval.deduct_from_amount)
    .fetch_one(pool)
    .await
    .wrap_err("Failed to insert approval")?;

    Ok(row.get("id"))
}

/// Get pending approvals for submission
pub async fn get_pending_approvals(pool: &PgPool, dest_chain_id: i64) -> Result<Vec<Approval>> {
    let rows = sqlx::query_as::<_, Approval>(
        r#"SELECT * FROM approvals WHERE status = 'pending' AND dest_chain_id = $1"#,
    )
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

/// Check if approval exists for (src_chain_key, nonce)
pub async fn approval_exists(
    pool: &PgPool,
    src_chain_key: &[u8],
    nonce: i64,
    dest_chain_id: i64,
) -> Result<bool> {
    let row: (bool,) = sqlx::query_as(
        r#"SELECT EXISTS(SELECT 1 FROM approvals WHERE src_chain_key = $1 AND nonce = $2 AND dest_chain_id = $3)"#,
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
    let row = sqlx::query(
        r#"
        INSERT INTO releases (src_chain_key, nonce, sender, recipient, token, amount, source_chain_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
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
    let rows =
        sqlx::query_as::<_, Release>(r#"SELECT * FROM releases WHERE status = 'pending'"#)
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
    let row: Option<(i64,)> = sqlx::query_as(
        r#"SELECT last_processed_block FROM evm_blocks WHERE chain_id = $1"#,
    )
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
    let row: Option<(i64,)> = sqlx::query_as(
        r#"SELECT last_processed_height FROM terra_blocks WHERE chain_id = $1"#,
    )
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
    let rows = sqlx::query_as::<_, Approval>(
        r#"SELECT * FROM approvals WHERE status = 'submitted'"#,
    )
    .fetch_all(pool)
    .await
    .wrap_err("Failed to get submitted approvals")?;

    Ok(rows)
}

/// Get submitted releases for confirmation checking
pub async fn get_submitted_releases(pool: &PgPool) -> Result<Vec<Release>> {
    let rows = sqlx::query_as::<_, Release>(
        r#"SELECT * FROM releases WHERE status = 'submitted'"#,
    )
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
    let rows = sqlx::query_as::<_, Approval>(
        r#"
        SELECT * FROM approvals 
        WHERE status = 'failed' 
          AND dest_chain_id = $1
          AND attempts < $2
          AND (retry_after IS NULL OR retry_after <= NOW())
        ORDER BY created_at ASC
        LIMIT 10
        "#,
    )
    .bind(dest_chain_id)
    .bind(max_attempts)
    .fetch_all(pool)
    .await
    .wrap_err("Failed to get failed approvals for retry")?;

    Ok(rows)
}

/// Get failed releases that are ready for retry
pub async fn get_failed_releases_for_retry(pool: &PgPool, max_attempts: i32) -> Result<Vec<Release>> {
    let rows = sqlx::query_as::<_, Release>(
        r#"
        SELECT * FROM releases 
        WHERE status = 'failed' 
          AND attempts < $1
          AND (retry_after IS NULL OR retry_after <= NOW())
        ORDER BY created_at ASC
        LIMIT 10
        "#,
    )
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
    sqlx::query(
        r#"UPDATE approvals SET status = 'pending', retry_after = $1 WHERE id = $2"#,
    )
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
    sqlx::query(
        r#"UPDATE releases SET status = 'pending', retry_after = $1 WHERE id = $2"#,
    )
    .bind(retry_after)
    .bind(id)
    .execute(pool)
    .await
    .wrap_err_with(|| format!("Failed to update release {} for retry", id))?;

    Ok(())
}

// ============ Sprint 4: API/Status Queries ============

/// Count pending deposits by chain
pub async fn count_pending_deposits(pool: &PgPool) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM evm_deposits WHERE status = 'pending'"#,
    )
    .fetch_one(pool)
    .await
    .wrap_err("Failed to count pending deposits")?;

    Ok(row.0)
}

/// Count pending approvals
pub async fn count_pending_approvals(pool: &PgPool) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM approvals WHERE status = 'pending'"#,
    )
    .fetch_one(pool)
    .await
    .wrap_err("Failed to count pending approvals")?;

    Ok(row.0)
}

/// Count submitted approvals
pub async fn count_submitted_approvals(pool: &PgPool) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM approvals WHERE status = 'submitted'"#,
    )
    .fetch_one(pool)
    .await
    .wrap_err("Failed to count submitted approvals")?;

    Ok(row.0)
}

/// Count pending releases
pub async fn count_pending_releases(pool: &PgPool) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM releases WHERE status = 'pending'"#,
    )
    .fetch_one(pool)
    .await
    .wrap_err("Failed to count pending releases")?;

    Ok(row.0)
}

/// Count submitted releases
pub async fn count_submitted_releases(pool: &PgPool) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM releases WHERE status = 'submitted'"#,
    )
    .fetch_one(pool)
    .await
    .wrap_err("Failed to count submitted releases")?;

    Ok(row.0)
}
