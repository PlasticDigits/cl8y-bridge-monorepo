//! Method-level EVM JSON-RPC fallback shared by the watcher and writer (GL-138 / GL-170).
//!
//! Selecting an endpoint with `eth_blockNumber` is not proof that `eth_getLogs`
//! or `eth_sendRawTransaction` will succeed. Each method is retried against the
//! remaining validated URLs on retryable transport / HTTP / rate-limit errors.
//! Execute send uses [`with_retryable_rpc_fallback`] so contract reverts are not
//! re-broadcast on every URL. Receipt waits after a successful send must look up
//! the hash on fallbacks — they must not send again (INV-OP-W12).
//!
//! Logs use [`log_rpc`] / [`log_rpc_error`] so credentials, query tokens, and
//! path API keys (Alchemy `/v2/<key>`, Infura `/v3/<id>`) never appear (INV-OP-W9).
//! Successful log queries (including empty results) are confirmed with
//! `eth_chainId` before the caller may treat the range as observed.

use alloy::providers::{Provider, RootProvider};
use alloy::rpc::types::{Filter, Log};
use alloy::transports::http::{Client, Http};
use eyre::{eyre, Result};
use std::future::Future;
use tracing::{debug, info, warn};

use crate::metrics;

/// Log-safe RPC URL (`scheme://host[:port]`, INV-OP-W9).
pub fn log_rpc(url: &str) -> String {
    multichain_rs::sanitize_rpc_endpoint(url)
}

/// Log-safe error Display (strips embedded RPC URLs / path keys).
pub fn log_rpc_error(err: &impl std::fmt::Display) -> String {
    multichain_rs::sanitize_rpc_error(&err.to_string())
}

/// Runtime `eth_chainId` check so empty fallback logs cannot advance a cursor
/// on a wrong-chain or unauthenticated endpoint (INV-OP-W1 / INV-OP-W3).
pub async fn confirm_rpc_chain_id(
    provider: &RootProvider<Http<Client>>,
    expected_chain_id: u64,
) -> Result<()> {
    let got = provider
        .get_chain_id()
        .await
        .map_err(|e| eyre!("eth_chainId failed: {}", log_rpc_error(&e)))?;
    if got != expected_chain_id {
        return Err(eyre!(
            "RPC endpoint chain id {got} != expected {expected_chain_id}"
        ));
    }
    Ok(())
}

/// Run `op(provider_index)` against endpoints in try-order until one returns `Ok`.
///
/// Used for method-level fallback (`eth_getLogs`, typed event queries). Cursor
/// advancement is the caller's responsibility (INV-OP-W2). Non-retryable errors
/// still try remaining URLs (a wrong-chain or auth failure on one host must
/// not hide a working fallback for reads).
pub async fn with_endpoint_fallback<T, F, Fut>(
    urls: &[String],
    prefer_index: Option<usize>,
    chain_label: &str,
    method: &str,
    op: F,
) -> Result<T>
where
    F: FnMut(usize) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    with_endpoint_fallback_inner(urls, prefer_index, chain_label, method, op, false).await
}

/// Like [`with_endpoint_fallback`], but stops on the first non-retryable error.
///
/// Use for `withdrawExecute*` **send** so `CancelWindowActive` / `BelowMin` /
/// period-limit reverts are not submitted on every URL. After a successful
/// send, wait for the receipt by hash — do not wrap send+receipt together
/// (that would re-broadcast; INV-OP-W12).
pub async fn with_retryable_rpc_fallback<T, F, Fut>(
    urls: &[String],
    prefer_index: Option<usize>,
    chain_label: &str,
    method: &str,
    op: F,
) -> Result<T>
where
    F: FnMut(usize) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    with_endpoint_fallback_inner(urls, prefer_index, chain_label, method, op, true).await
}

async fn with_endpoint_fallback_inner<T, F, Fut>(
    urls: &[String],
    prefer_index: Option<usize>,
    chain_label: &str,
    method: &str,
    mut op: F,
    stop_on_non_retryable: bool,
) -> Result<T>
where
    F: FnMut(usize) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    if urls.is_empty() {
        return Err(eyre!("no EVM RPC endpoints configured for {method}"));
    }
    let order = endpoint_try_order(urls.len(), prefer_index);
    let mut last_err: Option<eyre::Report> = None;

    for (attempt, idx) in order.iter().copied().enumerate() {
        match op(idx).await {
            Ok(v) => {
                if attempt > 0 {
                    metrics::record_rpc_fallback(chain_label, method);
                    info!(
                        chain = chain_label,
                        method,
                        rpc = %log_rpc(&urls[idx]),
                        fallback_attempt = attempt,
                        "{method} succeeded on fallback endpoint"
                    );
                }
                return Ok(v);
            }
            Err(e) => {
                metrics::record_rpc_failure(chain_label, method);
                let retryable = multichain_rs::is_retryable_evm_rpc_error_message(&e.to_string());
                warn!(
                    chain = chain_label,
                    method,
                    rpc = %log_rpc(&urls[idx]),
                    retryable,
                    remaining = order.len().saturating_sub(attempt + 1),
                    error = %log_rpc_error(&e),
                    "{method} failed on endpoint"
                );
                if stop_on_non_retryable && !retryable {
                    return Err(e);
                }
                last_err = Some(e);
                if !retryable && attempt + 1 < order.len() {
                    debug!(
                        chain = chain_label,
                        method, "error not classified retryable; still trying remaining endpoints"
                    );
                }
            }
        }
    }

    let last = last_err.unwrap_or_else(|| eyre!("{method} failed"));
    Err(eyre!(
        "{method} failed on all configured RPC endpoints: {}",
        log_rpc_error(&last)
    ))
}

/// `eth_getLogs` against `providers`, trying `prefer_index` first then the rest.
///
/// Advances through endpoints only; never skips the caller's block range.
/// The caller owns contiguous cursor semantics.
///
/// A successful response (including **empty** logs) is not returned until
/// `eth_chainId` matches `expected_chain_id`. Empty logs from a wrong-chain
/// fallback must not be treated as an observed range.
pub async fn get_logs_with_endpoint_fallback(
    urls: &[String],
    providers: &[RootProvider<Http<Client>>],
    filter: &Filter,
    prefer_index: Option<usize>,
    chain_label: &str,
    expected_chain_id: u64,
) -> Result<Vec<Log>> {
    if providers.len() != urls.len() {
        return Err(eyre!(
            "RPC URL/provider length mismatch ({} urls, {} providers)",
            urls.len(),
            providers.len()
        ));
    }
    with_endpoint_fallback(urls, prefer_index, chain_label, "eth_getLogs", |idx| {
        let provider = providers[idx].clone();
        let filter = filter.clone();
        async move {
            let logs = provider
                .get_logs(&filter)
                .await
                .map_err(|e| eyre::eyre!(e))?;
            confirm_rpc_chain_id(&provider, expected_chain_id).await?;
            Ok(logs)
        }
    })
    .await
}

/// Prefer `prefer_index` (consensus head provider) then remaining indices in order.
pub fn endpoint_try_order(count: usize, prefer_index: Option<usize>) -> Vec<usize> {
    if count == 0 {
        return Vec::new();
    }
    let mut order: Vec<usize> = (0..count).collect();
    if let Some(pref) = prefer_index {
        if pref < count {
            order.remove(pref);
            order.insert(0, pref);
        }
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::providers::ProviderBuilder;
    use axum::{
        extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router,
    };
    use serde_json::{json, Value};
    use std::sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    };

    #[test]
    fn try_order_prefers_consensus_index() {
        assert_eq!(endpoint_try_order(3, Some(1)), vec![1, 0, 2]);
        assert_eq!(endpoint_try_order(3, Some(0)), vec![0, 1, 2]);
        assert_eq!(endpoint_try_order(2, None), vec![0, 1]);
        assert!(endpoint_try_order(0, Some(0)).is_empty());
    }

    const EXPECTED_CHAIN: u64 = 31337;

    #[derive(Clone)]
    struct MockCfg {
        fail_logs: bool,
        chain_id: u64,
        log_calls: Arc<AtomicU64>,
        block_calls: Arc<AtomicU64>,
        chain_calls: Arc<AtomicU64>,
        log_status: StatusCode,
    }

    async fn jsonrpc_handler(
        State(cfg): State<MockCfg>,
        Json(req): Json<Value>,
    ) -> impl IntoResponse {
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = req.get("id").cloned().unwrap_or(json!(1));
        match method {
            "eth_blockNumber" => {
                cfg.block_calls.fetch_add(1, Ordering::SeqCst);
                Json(json!({"jsonrpc":"2.0","id": id, "result": "0x64"})).into_response()
            }
            "eth_chainId" => {
                cfg.chain_calls.fetch_add(1, Ordering::SeqCst);
                Json(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": format!("0x{:x}", cfg.chain_id)
                }))
                .into_response()
            }
            "eth_getLogs" => {
                cfg.log_calls.fetch_add(1, Ordering::SeqCst);
                if cfg.fail_logs {
                    (
                        cfg.log_status,
                        Json(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {"code": -32005, "message": "Too many requests"}
                        })),
                    )
                        .into_response()
                } else {
                    Json(json!({"jsonrpc":"2.0","id": id, "result": []})).into_response()
                }
            }
            _ => Json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32601, "message": "method not found"}
            }))
            .into_response(),
        }
    }

    async fn spawn_mock(
        fail_logs: bool,
        log_status: StatusCode,
        chain_id: u64,
    ) -> (String, MockCfg) {
        let cfg = MockCfg {
            fail_logs,
            chain_id,
            log_calls: Arc::new(AtomicU64::new(0)),
            block_calls: Arc::new(AtomicU64::new(0)),
            chain_calls: Arc::new(AtomicU64::new(0)),
            log_status,
        };
        let app = Router::new()
            .route("/", post(jsonrpc_handler))
            .with_state(cfg.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        (format!("http://{addr}"), cfg)
    }

    fn providers_for(urls: &[String]) -> Vec<RootProvider<Http<Client>>> {
        urls.iter()
            .map(|u| ProviderBuilder::new().on_http(u.parse().unwrap()))
            .collect()
    }

    #[tokio::test]
    async fn get_logs_falls_back_when_primary_rate_limits() {
        let (primary, pcfg) = spawn_mock(true, StatusCode::TOO_MANY_REQUESTS, EXPECTED_CHAIN).await;
        let (fallback, fcfg) = spawn_mock(false, StatusCode::OK, EXPECTED_CHAIN).await;
        let urls = vec![primary, fallback];
        let providers = providers_for(&urls);
        let logs = get_logs_with_endpoint_fallback(
            &urls,
            &providers,
            &Filter::new(),
            None,
            "evm-test",
            EXPECTED_CHAIN,
        )
        .await
        .expect("fallback should succeed");
        assert!(logs.is_empty());
        assert!(pcfg.log_calls.load(Ordering::SeqCst) >= 1);
        assert!(fcfg.log_calls.load(Ordering::SeqCst) >= 1);
        assert!(fcfg.chain_calls.load(Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn get_logs_errors_when_all_endpoints_fail() {
        let (a, _) = spawn_mock(true, StatusCode::TOO_MANY_REQUESTS, EXPECTED_CHAIN).await;
        let (b, _) = spawn_mock(true, StatusCode::SERVICE_UNAVAILABLE, EXPECTED_CHAIN).await;
        let urls = vec![a, b];
        let providers = providers_for(&urls);
        let err = get_logs_with_endpoint_fallback(
            &urls,
            &providers,
            &Filter::new(),
            Some(0),
            "evm-test",
            EXPECTED_CHAIN,
        )
        .await
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("all configured RPC endpoints") || msg.contains("eth_getLogs"),
            "{msg}"
        );
        assert!(
            !msg.contains("apiKey") && !msg.contains("/v2/"),
            "error Display leaked RPC path: {msg}"
        );
    }

    #[tokio::test]
    async fn get_logs_does_not_call_block_number() {
        // Method-level: log query must not depend on a prior eth_blockNumber success
        // on the same endpoint (the livelock root cause).
        let (url, cfg) = spawn_mock(false, StatusCode::OK, EXPECTED_CHAIN).await;
        let urls = vec![url.clone()];
        let providers = providers_for(&urls);
        let _ = get_logs_with_endpoint_fallback(
            &urls,
            &providers,
            &Filter::new(),
            None,
            "evm-test",
            EXPECTED_CHAIN,
        )
        .await
        .unwrap();
        assert_eq!(cfg.block_calls.load(Ordering::SeqCst), 0);
        assert!(cfg.log_calls.load(Ordering::SeqCst) >= 1);
        assert!(cfg.chain_calls.load(Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn writer_livelock_primary_blocknumber_ok_logs_429_fallback_advances_cursor_once() {
        use crate::writers::poll_cursor::EventPollCursor;
        use std::time::Instant;

        let (primary, pcfg) = spawn_mock(true, StatusCode::TOO_MANY_REQUESTS, EXPECTED_CHAIN).await;
        let (fallback, fcfg) = spawn_mock(false, StatusCode::OK, EXPECTED_CHAIN).await;
        let urls = vec![primary, fallback];
        let providers = providers_for(&urls);

        let mut cursor = EventPollCursor::new();
        let head = 0x64u64;
        let lookback = 50;
        let range = cursor
            .plan_range(head, lookback, Instant::now())
            .expect("first poll range");
        assert!(range.is_first_poll);
        assert!(cursor.take_first_poll_log());
        assert!(!cursor.take_first_poll_log(), "first-poll info only once");

        let logs = get_logs_with_endpoint_fallback(
            &urls,
            &providers,
            &Filter::new(),
            Some(0),
            "evm-test",
            EXPECTED_CHAIN,
        )
        .await
        .expect("fallback logs after primary 429");
        assert!(logs.is_empty());
        cursor.on_chunk_success(range.to_block);
        assert_eq!(cursor.last_polled_block, head);
        assert!(
            cursor.plan_range(head, lookback, Instant::now()).is_none(),
            "same head must not re-issue first-poll lookback"
        );
        assert!(pcfg.log_calls.load(Ordering::SeqCst) >= 1);
        assert!(pcfg.block_calls.load(Ordering::SeqCst) == 0);
        assert!(fcfg.log_calls.load(Ordering::SeqCst) >= 1);
        assert!(fcfg.chain_calls.load(Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn empty_wrong_chain_fallback_logs_do_not_succeed() {
        let (primary, _) = spawn_mock(true, StatusCode::TOO_MANY_REQUESTS, EXPECTED_CHAIN).await;
        let (fallback, fcfg) = spawn_mock(false, StatusCode::OK, 999).await;
        let urls = vec![primary, fallback];
        let providers = providers_for(&urls);
        let err = get_logs_with_endpoint_fallback(
            &urls,
            &providers,
            &Filter::new(),
            Some(0),
            "evm-test",
            EXPECTED_CHAIN,
        )
        .await
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("chain id") || msg.contains("all configured"),
            "{msg}"
        );
        assert!(fcfg.log_calls.load(Ordering::SeqCst) >= 1);
        assert!(fcfg.chain_calls.load(Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn retryable_rpc_fallback_skips_second_url_on_contract_revert() {
        let calls = Arc::new(AtomicU64::new(0));
        let calls2 = calls.clone();
        let urls = vec!["http://127.0.0.1:1".into(), "http://127.0.0.1:2".into()];
        let err = with_retryable_rpc_fallback(&urls, None, "evm-test", "withdrawExecute", |_idx| {
            let calls2 = calls2.clone();
            async move {
                calls2.fetch_add(1, Ordering::SeqCst);
                Err::<String, _>(eyre!("execution reverted: CancelWindowActive"))
            }
        })
        .await
        .unwrap_err();
        assert!(err.to_string().contains("CancelWindowActive"), "{err}");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retryable_rpc_fallback_uses_second_url_after_429() {
        let calls = Arc::new(AtomicU64::new(0));
        let calls2 = calls.clone();
        let urls = vec!["http://127.0.0.1:1".into(), "http://127.0.0.1:2".into()];
        let got = with_retryable_rpc_fallback(&urls, None, "evm-test", "withdrawExecute", |idx| {
            let calls2 = calls2.clone();
            async move {
                calls2.fetch_add(1, Ordering::SeqCst);
                if idx == 0 {
                    Err(eyre!("HTTP error 429"))
                } else {
                    Ok("0xabc".to_string())
                }
            }
        })
        .await
        .expect("fallback send");
        assert_eq!(got, "0xabc");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
