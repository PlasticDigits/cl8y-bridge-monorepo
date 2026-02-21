use alloy::providers::{ProviderBuilder, RootProvider};
use alloy::transports::http::{Client, Http};
use eyre::{Result, WrapErr};

/// Parse a comma-separated RPC URL string into individual trimmed URLs.
pub fn parse_rpc_urls(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Create alloy HTTP providers for each RPC URL.
pub fn create_providers(urls: &[String]) -> Result<Vec<RootProvider<Http<Client>>>> {
    if urls.is_empty() {
        return Err(eyre::eyre!("At least one RPC URL is required"));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_url() {
        let urls = parse_rpc_urls("https://bsc.publicnode.com");
        assert_eq!(urls, vec!["https://bsc.publicnode.com"]);
    }

    #[test]
    fn test_parse_multiple_urls() {
        let urls = parse_rpc_urls(
            "https://bsc.publicnode.com,https://bsc-dataseed1.binance.org,https://binance.llamarpc.com",
        );
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://bsc.publicnode.com");
        assert_eq!(urls[1], "https://bsc-dataseed1.binance.org");
        assert_eq!(urls[2], "https://binance.llamarpc.com");
    }

    #[test]
    fn test_parse_trims_whitespace() {
        let urls = parse_rpc_urls(" https://a.com , https://b.com , https://c.com ");
        assert_eq!(
            urls,
            vec!["https://a.com", "https://b.com", "https://c.com"]
        );
    }

    #[test]
    fn test_parse_ignores_empty() {
        let urls = parse_rpc_urls("https://a.com,,https://b.com,");
        assert_eq!(urls, vec!["https://a.com", "https://b.com"]);
    }

    #[test]
    fn test_parse_empty_string() {
        let urls = parse_rpc_urls("");
        assert!(urls.is_empty());
    }

    #[test]
    fn test_create_providers_single() {
        let providers = create_providers(&["http://localhost:8545".to_string()]).unwrap();
        assert_eq!(providers.len(), 1);
    }

    #[test]
    fn test_create_providers_multiple() {
        let providers = create_providers(&[
            "http://localhost:8545".to_string(),
            "http://localhost:8546".to_string(),
        ])
        .unwrap();
        assert_eq!(providers.len(), 2);
    }

    #[test]
    fn test_create_providers_empty_fails() {
        assert!(create_providers(&[]).is_err());
    }
}
