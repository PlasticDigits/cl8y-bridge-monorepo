//! Comma-separated RPC URL lists and Alloy HTTP provider helpers.
//!
//! Shared by the operator (EVM event polling, config parsing) and the canceler (reads / cancel txs).
//! [`parse_comma_separated_rpc_urls`] is the same comma-splitting rule as `multi_evm` / operator `EVM_RPC_URL`.

use alloy::providers::{ProviderBuilder, RootProvider};
use alloy::transports::http::{Client, Http};
use eyre::{eyre, Result, WrapErr};
use std::future::Future;

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
}
