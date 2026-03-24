//! One-off backfill: set `evm_deposits.transfer_hash` for rows where it is NULL but V2
//! fields are present (same `compute_xchain_hash_id` as the EVM watcher).
//!
//! ## Run on QA / production (any machine with Rust toolchain and DB access)
//!
//! ```text
//! cd /path/to/cl8y-bridge-monorepo/packages/operator
//! export DATABASE_URL="postgres://operator:operator@localhost:5433/operator"
//!   # or: set -a && source .env && set +a
//! cargo run --release --bin backfill-evm-transfer-hashes
//! ```
//!
//! Safe to run multiple times: only updates rows that still have `transfer_hash IS NULL`.
//! Idempotent per row: `UPDATE ... WHERE id = $1 AND transfer_hash IS NULL`.

use eyre::{Result, WrapErr};
use multichain_rs::hash::compute_xchain_hash_id;
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").wrap_err(
        "DATABASE_URL must be set (e.g. export DATABASE_URL=postgres://... on the QA VPS)",
    )?;

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .wrap_err("Failed to connect to Postgres")?;

    let rows = sqlx::query(
        r#"
        SELECT id, nonce, dest_chain_key, dest_token_address, dest_account,
               amount::TEXT AS amount, src_account, src_v2_chain_id
        FROM evm_deposits
        WHERE transfer_hash IS NULL
        ORDER BY id
        "#,
    )
    .fetch_all(&pool)
    .await
    .wrap_err("Failed to list evm_deposits with NULL transfer_hash")?;

    let mut updated = 0u64;
    let mut skipped = 0u64;

    for row in rows {
        let id: i64 = row.try_get("id")?;
        let nonce: i64 = row.try_get("nonce")?;
        let dest_chain_key: Vec<u8> = row.try_get("dest_chain_key")?;
        let dest_token_address: Vec<u8> = row.try_get("dest_token_address")?;
        let dest_account: Vec<u8> = row.try_get("dest_account")?;
        let amount_str: String = row.try_get("amount")?;
        let src_account: Option<Vec<u8>> = row.try_get("src_account")?;
        let src_v2_chain_id: Option<Vec<u8>> = row.try_get("src_v2_chain_id")?;

        let hash = match try_compute_hash(
            nonce,
            &dest_chain_key,
            &dest_token_address,
            &dest_account,
            &amount_str,
            src_account.as_deref(),
            src_v2_chain_id.as_deref(),
        ) {
            Ok(h) => h,
            Err(reason) => {
                tracing::debug!(id, %reason, "skip row");
                skipped += 1;
                continue;
            }
        };

        let res = sqlx::query(
            r#"UPDATE evm_deposits SET transfer_hash = $1 WHERE id = $2 AND transfer_hash IS NULL"#,
        )
        .bind(&hash[..])
        .bind(id)
        .execute(&pool)
        .await
        .wrap_err_with(|| format!("failed to update evm_deposits id={}", id))?;

        if res.rows_affected() > 0 {
            updated += 1;
            tracing::info!(id, hash = %hex::encode(hash), "backfilled transfer_hash");
        }
    }

    tracing::info!(
        updated,
        skipped,
        "backfill-evm-transfer-hashes done (skipped = missing or invalid V2 fields)"
    );
    Ok(())
}

fn try_compute_hash(
    nonce: i64,
    dest_chain_key: &[u8],
    dest_token_address: &[u8],
    dest_account: &[u8],
    amount_str: &str,
    src_account: Option<&[u8]>,
    src_v2_chain_id: Option<&[u8]>,
) -> std::result::Result<[u8; 32], String> {
    let src_account = src_account.filter(|s| s.len() == 32).ok_or_else(|| {
        "src_account missing or not 32 bytes (legacy V1 deposit — cannot derive hash here)"
            .to_string()
    })?;
    let src_chain = src_v2_chain_id
        .filter(|s| s.len() == 4)
        .ok_or_else(|| "src_v2_chain_id missing or not 4 bytes".to_string())?;
    if dest_chain_key.len() < 4 {
        return Err("dest_chain_key too short".into());
    }
    if dest_token_address.len() != 32 {
        return Err("dest_token_address not 32 bytes".into());
    }
    if dest_account.len() != 32 {
        return Err("dest_account not 32 bytes".into());
    }
    let nonce_u64 = u64::try_from(nonce).map_err(|_| "nonce negative or too large".to_string())?;
    let amount_u128: u128 = amount_str
        .parse()
        .map_err(|_| "amount parse failed".to_string())?;

    let mut src_account_arr = [0u8; 32];
    src_account_arr.copy_from_slice(src_account);
    let mut dest_account_arr = [0u8; 32];
    dest_account_arr.copy_from_slice(dest_account);
    let mut token_arr = [0u8; 32];
    token_arr.copy_from_slice(dest_token_address);
    let mut src_chain_arr = [0u8; 4];
    src_chain_arr.copy_from_slice(&src_chain[..4]);
    let mut dest_chain_arr = [0u8; 4];
    dest_chain_arr.copy_from_slice(&dest_chain_key[..4]);

    Ok(compute_xchain_hash_id(
        &src_chain_arr,
        &dest_chain_arr,
        &src_account_arr,
        &dest_account_arr,
        &token_arr,
        amount_u128,
        nonce_u64,
    ))
}
