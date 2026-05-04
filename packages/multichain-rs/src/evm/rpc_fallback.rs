//! Comma-separated RPC URL lists and Alloy HTTP provider helpers.
//!
//! [`parse_comma_separated_rpc_urls`] is the same comma-splitting rule as `multi_evm` / operator `EVM_RPC_URL`.
//!
//! Read paths should use [`evm_consensus_latest_block`] instead of [`run_with_evm_rpc_url_fallback`]
//! so multiple endpoints must agree (within tolerance) before trusting a head block, unless
//! single-endpoint mode is explicitly configured.

use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::transports::http::{Client, Http};
use eyre::{eyre, Result, WrapErr};
use std::env;
use std::error::Error;
use std::future::Future;

/// Maximum `eth_chainId` attempts per RPC URL during startup verification (includes first try).
const VERIFY_CHAIN_ID_MAX_ATTEMPTS: u32 = 8;
/// Initial backoff after a transient RPC failure when verifying chain IDs at startup.
const VERIFY_CHAIN_ID_INITIAL_BACKOFF_MS: u64 = 300;

/// Whether an error chain looks like an HTTP / infra failure that may succeed after backoff.
fn evm_jsonrpc_error_is_transient(err: &(dyn Error + 'static)) -> bool {
    let mut collected = String::new();
    let mut current: Option<&(dyn Error + 'static)> = Some(err);
    while let Some(e) = current {
        if !collected.is_empty() {
            collected.push(' ');
        }
        collected.push_str(&e.to_string());
        current = e.source();
    }
    let m = collected.to_ascii_lowercase();
    m.contains("429")
        || m.contains("rate limit")
        || m.contains("too many request")
        || m.contains("\"code\":15")
        || m.contains("503")
        || m.contains("502")
        || m.contains("504")
        || m.contains("408")
        || m.contains("timed out")
        || m.contains("timeout")
        || m.contains("connection reset")
        || m.contains("temporarily unavailable")
        || m.contains("try again")
        || m.contains("service unavailable")
}

/// Split a comma-separated RPC URL string into trimmed, non-empty URLs.
pub fn parse_comma_separated_rpc_urls(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Build one Alloy HTTP [`RootProvider`] per URL (for polling with manual fallback).
pub fn create_alloy_http_providers(urls: &[String]) -> Result<Vec<RootProvider<Http<Client>>>> {
    if urls.is_empty() {
        return Err(eyre!("At least one RPC URL is required"));
    }
    urls.iter()
        .map(|url| {
            let parsed = url
                .parse()
                .wrap_err_with(|| format!("Invalid RPC URL: {}", url))?;
            Ok(ProviderBuilder::new().on_http(parsed))
        })
        .collect()
}

/// Policy for EVM JSON-RPC **reads** (block head, logs, etc.).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvmRpcReadPolicy {
    /// Minimum number of sampled endpoints whose `eth_blockNumber` must fall in the same tolerance cluster.
    pub min_agreeing: usize,
    /// When true, only `urls[0]` participates in read-path sampling (extra URLs are not consulted for reads).
    pub single_endpoint_reads: bool,
}

impl EvmRpcReadPolicy {
    /// Load policy from environment for a concrete URL list length.
    ///
    /// Environment:
    /// - `EVM_RPC_AGREEMENT_QUORUM` — optional; default `min(2, url_count)`.
    /// - `EVM_RPC_SINGLE_ENDPOINT_READS` — `1` / `true` to use only the first URL for reads when multiple URLs are listed.
    ///
    /// If `min_agreeing == 1` with `url_count > 1`, startup fails unless `EVM_RPC_SINGLE_ENDPOINT_READS` is set,
    /// so operators do not accidentally enable first-wins failover between independent RPC truths.
    pub fn from_env_for_url_count(url_count: usize) -> Result<Self> {
        if url_count == 0 {
            return Err(eyre!("EVM RPC URL list is empty"));
        }

        let single_endpoint_reads = env::var("EVM_RPC_SINGLE_ENDPOINT_READS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let default_quorum = std::cmp::min(2, url_count);
        let min_agreeing: usize = match env::var("EVM_RPC_AGREEMENT_QUORUM") {
            Ok(s) => s
                .parse()
                .map_err(|_| eyre!("EVM_RPC_AGREEMENT_QUORUM must be a positive integer"))?,
            Err(_) => default_quorum,
        };

        if min_agreeing < 1 {
            return Err(eyre!("EVM_RPC_AGREEMENT_QUORUM must be >= 1"));
        }
        if min_agreeing > url_count {
            return Err(eyre!(
                "EVM_RPC_AGREEMENT_QUORUM={} but only {} RPC URL(s) are configured",
                min_agreeing,
                url_count
            ));
        }

        if min_agreeing == 1 && url_count > 1 && !single_endpoint_reads {
            return Err(eyre!(
                "EVM_RPC_AGREEMENT_QUORUM=1 with multiple RPC URLs is unsafe: a failing primary could \
                 fail over to an unrelated endpoint with a different head. Use a single URL, set \
                 EVM_RPC_SINGLE_ENDPOINT_READS=1 to pin reads to the first URL only, or set \
                 EVM_RPC_AGREEMENT_QUORUM>=2 and list enough independent endpoints."
            ));
        }

        if single_endpoint_reads && min_agreeing > 1 {
            return Err(eyre!(
                "EVM_RPC_SINGLE_ENDPOINT_READS=1 is incompatible with EVM_RPC_AGREEMENT_QUORUM>1"
            ));
        }

        Ok(Self {
            min_agreeing,
            single_endpoint_reads,
        })
    }

    /// Indices into `urls` to sample for `eth_blockNumber` (at most three, for tie-breaking).
    pub fn sample_indices(&self, url_count: usize) -> Vec<usize> {
        if self.single_endpoint_reads {
            return vec![0];
        }
        let k = std::cmp::min(3, url_count);
        (0..k).collect()
    }
}

/// Latest block number agreed by RPC samples (before subtracting finality / confirmation depth).
#[derive(Clone, Debug)]
pub struct ConsensusHead {
    /// Conservative choice: **minimum** block height inside the winning tolerance cluster.
    pub latest_block: u64,
    /// Provider index to prefer for follow-up reads (`eth_getLogs`, etc.): lowest index whose
    /// reported head matches `latest_block` within tolerance.
    pub provider_index: usize,
}

/// Maximum absolute difference allowed between two `eth_blockNumber` values at the larger height.
/// Uses 0.01% of the larger height, with a minimum of 1 (integer block heights).
#[inline]
pub fn block_number_tolerance_delta(hi: u64) -> u64 {
    let hi = hi as u128;
    let pct = std::cmp::max(1u128, hi.saturating_mul(10) / 100_000); // ceil-ish 0.01%
    std::cmp::min(pct, u64::MAX as u128) as u64
}

/// Whether two block numbers are considered agreeing peers.
pub fn block_numbers_agree(a: u64, b: u64) -> bool {
    let hi = a.max(b);
    let lo = a.min(b);
    hi - lo <= block_number_tolerance_delta(hi)
}

fn cluster_min_and_indices(values: &[(usize, u64)], member: &[bool]) -> (u64, Vec<usize>) {
    let mut idxs: Vec<usize> = values
        .iter()
        .zip(member.iter())
        .filter_map(|(&(i, _), &m)| m.then_some(i))
        .collect();
    idxs.sort_unstable();
    let min_b = values
        .iter()
        .zip(member.iter())
        .filter_map(|(&(_, b), &m)| m.then_some(b))
        .min()
        .expect("non-empty cluster");
    (min_b, idxs)
}

/// Given successful `(provider_index, block_number)` samples, pick a cluster of size ≥ `min_agreeing`.
fn pick_consensus_cluster(
    samples: &[(usize, u64)],
    min_agreeing: usize,
) -> Option<(u64, Vec<usize>)> {
    if samples.is_empty() {
        return None;
    }
    let n = samples.len();
    let mut best_count = 0usize;
    let mut best_member: Vec<bool> = vec![false; n];

    for anchor in 0..n {
        let v0 = samples[anchor].1;
        let mut member = vec![false; n];
        for (j, &(_, bj)) in samples.iter().enumerate() {
            if block_numbers_agree(v0, bj) {
                member[j] = true;
            }
        }
        let count = member.iter().filter(|&&m| m).count();
        let cluster_min = samples
            .iter()
            .zip(&member)
            .filter_map(|(&(_, b), &m)| m.then_some(b))
            .min()
            .unwrap();
        let best_min = samples
            .iter()
            .zip(&best_member)
            .filter_map(|(&(_, b), &m)| m.then_some(b))
            .min();
        let better = count > best_count
            || (count == best_count && count > 0 && best_min.map_or(true, |bm| cluster_min < bm));
        if better {
            best_count = count;
            best_member = member;
        }
    }

    if best_count < min_agreeing {
        return None;
    }
    let (min_b, idxs) = cluster_min_and_indices(samples, &best_member);
    Some((min_b, idxs))
}

/// Query `eth_blockNumber` for each index in `sample_indices` in parallel.
pub async fn fetch_block_numbers_for_indices(
    urls: &[String],
    sample_indices: &[usize],
) -> Vec<(usize, Result<u64>)> {
    let mut handles = Vec::new();
    for &i in sample_indices {
        let url = urls[i].clone();
        handles.push(async move {
            let res: Result<u64> = async {
                let parsed = url
                    .parse()
                    .wrap_err_with(|| format!("Invalid RPC URL: {}", url))?;
                let provider = ProviderBuilder::new().on_http(parsed);
                provider
                    .get_block_number()
                    .await
                    .map_err(|e| eyre::Report::from(e))
                    .wrap_err_with(|| format!("get_block_number failed for {}", url))
            }
            .await;
            (i, res)
        });
    }
    futures_util::future::join_all(handles).await
}

/// Compute consensus head from parallel `eth_blockNumber` samples.
pub fn consensus_from_block_samples(
    samples: Vec<(usize, Result<u64>)>,
    min_agreeing: usize,
) -> Result<ConsensusHead> {
    let mut oks: Vec<(usize, u64)> = Vec::new();
    let mut errs: Vec<eyre::Report> = Vec::new();
    for (i, r) in samples {
        match r {
            Ok(b) => oks.push((i, b)),
            Err(e) => errs.push(e),
        }
    }

    if oks.is_empty() {
        let joined = errs
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(eyre!(
            "no successful eth_blockNumber responses for RPC consensus: {}",
            joined
        ));
    }

    let (latest_block, cluster_idxs) = pick_consensus_cluster(&oks, min_agreeing).ok_or_else(|| {
        let parts: Vec<String> = oks
            .iter()
            .map(|(i, b)| format!("idx{}=>{}", i, b))
            .collect();
        eyre!(
            "EVM RPC block head consensus failed: need {} agreeing endpoints within tolerance; got [{}]",
            min_agreeing,
            parts.join(", ")
        )
    })?;

    let provider_index = *cluster_idxs.iter().min().expect("non-empty cluster");

    Ok(ConsensusHead {
        latest_block,
        provider_index,
    })
}

/// Parallel multi-endpoint `eth_blockNumber` with tolerance / quorum rules.
pub async fn evm_consensus_latest_block(
    urls: &[String],
    policy: &EvmRpcReadPolicy,
) -> Result<ConsensusHead> {
    let idxs = policy.sample_indices(urls.len());
    let raw = fetch_block_numbers_for_indices(urls, &idxs).await;
    consensus_from_block_samples(raw, policy.min_agreeing)
}

/// Verify `eth_chainId` matches `expected_chain_id` for every URL in `urls`.
///
/// Each endpoint is queried with retries when failures look transient (HTTP 429, 5xx, timeouts),
/// so a rate-limited fallback URL does not prevent process startup if it recovers within the
/// backoff window.
pub async fn verify_evm_rpc_chain_ids(urls: &[String], expected_chain_id: u64) -> Result<()> {
    if urls.is_empty() {
        return Err(eyre!("no RPC URLs to verify"));
    }
    for (i, url) in urls.iter().enumerate() {
        let parsed: url::Url = url
            .parse()
            .wrap_err_with(|| format!("Invalid RPC URL: {}", url))?;

        let mut attempt = 0u32;
        loop {
            let provider = ProviderBuilder::new().on_http(parsed.clone());
            match provider.get_chain_id().await {
                Ok(id) => {
                    if id != expected_chain_id {
                        return Err(eyre!(
                            "RPC URL index {} ({}) reports chain id {} but expected {}",
                            i,
                            url,
                            id,
                            expected_chain_id
                        ));
                    }
                    break;
                }
                Err(e) => {
                    let retryable = evm_jsonrpc_error_is_transient(&e);
                    if retryable && attempt + 1 < VERIFY_CHAIN_ID_MAX_ATTEMPTS {
                        let backoff_ms = VERIFY_CHAIN_ID_INITIAL_BACKOFF_MS
                            .saturating_mul(2u64.saturating_pow(attempt));
                        let backoff_ms = backoff_ms.min(10_000);
                        tracing::warn!(
                            rpc_url_index = i,
                            rpc_url = %url,
                            attempt = attempt + 1,
                            max_attempts = VERIFY_CHAIN_ID_MAX_ATTEMPTS,
                            backoff_ms,
                            error = %e,
                            "eth_chainId failed with transient error; retrying"
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(
                        std::result::Result::<(), _>::Err(e)
                            .wrap_err_with(|| {
                                format!("eth_chainId failed for RPC URL index {}", i)
                            })
                            .unwrap_err(),
                    );
                }
            }
        }
    }
    Ok(())
}

/// Run `op` against each URL in order until one returns `Ok`.
///
/// `op` receives an owned `String` per attempt so async blocks can `move` without borrow issues.
///
/// On failure, logs and tries the next URL when more than one is configured.
pub async fn run_with_evm_rpc_url_fallback<T, F, Fut>(urls: &[String], mut op: F) -> Result<T>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    if urls.is_empty() {
        return Err(eyre!("no EVM RPC URLs configured"));
    }
    let mut last_err: Option<eyre::Report> = None;
    for (i, url) in urls.iter().enumerate() {
        match op(url.clone()).await {
            Ok(v) => {
                if i > 0 {
                    tracing::info!(
                        rpc_index = i,
                        rpc_url = %url,
                        "EVM RPC fallback endpoint succeeded"
                    );
                }
                return Ok(v);
            }
            Err(e) => {
                if urls.len() > 1 {
                    tracing::warn!(
                        rpc_index = i,
                        rpc_url = %url,
                        error = %e,
                        "EVM RPC attempt failed, trying next endpoint if any"
                    );
                }
                last_err = Some(e);
            }
        }
    }
    Err(last_err.expect("non-empty urls implies at least one attempt"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_url() {
        let urls = parse_comma_separated_rpc_urls("https://bsc.publicnode.com");
        assert_eq!(urls, vec!["https://bsc.publicnode.com"]);
    }

    #[test]
    fn test_parse_multiple_urls() {
        let urls = parse_comma_separated_rpc_urls(
            "https://bsc.publicnode.com,https://bsc-dataseed1.binance.org,https://bsc.drpc.org",
        );
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://bsc.publicnode.com");
        assert_eq!(urls[1], "https://bsc-dataseed1.binance.org");
        assert_eq!(urls[2], "https://bsc.drpc.org");
    }

    #[test]
    fn test_parse_trims_whitespace() {
        let urls =
            parse_comma_separated_rpc_urls(" https://a.com , https://b.com , https://c.com ");
        assert_eq!(
            urls,
            vec!["https://a.com", "https://b.com", "https://c.com"]
        );
    }

    #[test]
    fn test_parse_ignores_empty() {
        let urls = parse_comma_separated_rpc_urls("https://a.com,,https://b.com,");
        assert_eq!(urls, vec!["https://a.com", "https://b.com"]);
    }

    #[test]
    fn test_parse_empty_string() {
        let urls = parse_comma_separated_rpc_urls("");
        assert!(urls.is_empty());
    }

    #[test]
    fn test_create_providers_single() {
        let providers =
            create_alloy_http_providers(&["http://localhost:8545".to_string()]).unwrap();
        assert_eq!(providers.len(), 1);
    }

    #[test]
    fn test_create_providers_multiple() {
        let providers = create_alloy_http_providers(&[
            "http://localhost:8545".to_string(),
            "http://localhost:8546".to_string(),
        ])
        .unwrap();
        assert_eq!(providers.len(), 2);
    }

    #[test]
    fn test_create_providers_empty_fails() {
        assert!(create_alloy_http_providers(&[]).is_err());
    }

    #[test]
    fn test_block_numbers_agree_close() {
        assert!(block_numbers_agree(10_000_000, 10_000_000));
        let d = block_number_tolerance_delta(10_000_000);
        assert!(d >= 1000);
        assert!(block_numbers_agree(10_000_000, 10_000_000 + d));
        assert!(!block_numbers_agree(10_000_000, 10_000_000 + d + 1));
    }

    #[test]
    fn test_consensus_two_agree() {
        let samples = vec![(0usize, Ok(1000u64)), (1, Ok(1000))];
        let h = consensus_from_block_samples(samples, 2).unwrap();
        assert_eq!(h.latest_block, 1000);
        assert_eq!(h.provider_index, 0);
    }

    #[test]
    fn test_consensus_majority_three() {
        let samples = vec![(0usize, Ok(1000u64)), (1, Ok(10_000)), (2, Ok(1000))];
        let h = consensus_from_block_samples(samples, 2).unwrap();
        assert_eq!(h.latest_block, 1000);
        assert_eq!(h.provider_index, 0);
    }

    #[test]
    fn test_consensus_fails_split() {
        let samples = vec![(0usize, Ok(1000u64)), (1, Ok(5000u64))];
        assert!(consensus_from_block_samples(samples, 2).is_err());
    }

    #[test]
    fn test_policy_sample_indices() {
        let p = EvmRpcReadPolicy {
            min_agreeing: 2,
            single_endpoint_reads: false,
        };
        assert_eq!(p.sample_indices(5), vec![0, 1, 2]);
        let p1 = EvmRpcReadPolicy {
            min_agreeing: 1,
            single_endpoint_reads: true,
        };
        assert_eq!(p1.sample_indices(5), vec![0]);
    }

    #[test]
    fn jsonrpc_transient_detects_429_body() {
        let e = std::io::Error::new(
            std::io::ErrorKind::Other,
            r#"HTTP error 429 with body: {"code":15,"message":"Too many request"}"#,
        );
        assert!(super::evm_jsonrpc_error_is_transient(&e));
    }

    #[test]
    fn jsonrpc_transient_detects_503() {
        let e = std::io::Error::new(std::io::ErrorKind::Other, "HTTP status 503 Service Unavailable");
        assert!(super::evm_jsonrpc_error_is_transient(&e));
    }

    #[test]
    fn jsonrpc_transient_rejects_unrelated_message() {
        let e = std::io::Error::new(std::io::ErrorKind::Other, "invalid JSON-RPC params");
        assert!(!super::evm_jsonrpc_error_is_transient(&e));
    }
}
