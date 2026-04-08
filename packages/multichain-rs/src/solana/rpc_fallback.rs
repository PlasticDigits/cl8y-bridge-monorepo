//! Comma-separated Solana RPC URL lists and JSON-RPC client failover.

use solana_client::client_error::{ClientError, ClientErrorKind, Result as ClientResult};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_request::{RpcError, RpcResponseErrorData};

/// Split a comma-separated RPC URL string into trimmed, non-empty URLs.
pub fn parse_solana_rpc_urls(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Whether an RPC failure is likely infrastructure/transient (try another endpoint).
pub fn is_transient_solana_client_error(err: &ClientError) -> bool {
    match &err.kind {
        ClientErrorKind::Reqwest(_) => true,
        ClientErrorKind::Io(_) => true,
        ClientErrorKind::SerdeJson(_) => true,
        ClientErrorKind::Middleware(_) => true,
        ClientErrorKind::RpcError(RpcError::RpcRequestError(_)) => true,
        ClientErrorKind::RpcError(RpcError::RpcResponseError {
            code,
            message,
            data,
        }) => {
            if matches!(data, RpcResponseErrorData::NodeUnhealthy { .. }) {
                return true;
            }
            matches!(code, -32603_i64 | -32005 | -32001 | -32002 | -32003)
                || message.to_ascii_lowercase().contains("timed out")
                || message.to_ascii_lowercase().contains("timeout")
                || message.to_ascii_lowercase().contains("rate limit")
                || message.to_ascii_lowercase().contains("429")
                || message.to_ascii_lowercase().contains("503")
                || message.to_ascii_lowercase().contains("502")
                || message.to_ascii_lowercase().contains("410")
        }
        ClientErrorKind::RpcError(RpcError::ForUser(_)) => false,
        ClientErrorKind::RpcError(RpcError::ParseError(_)) => false,
        ClientErrorKind::SigningError(_) => false,
        ClientErrorKind::TransactionError(_) => false,
        // Application-specific `Custom` messages must not trigger another endpoint.
        ClientErrorKind::Custom(_) => false,
    }
}

/// Run `op` against each client until one succeeds or a non-transient error occurs.
pub fn run_with_solana_rpc_fallback<T>(
    clients: &[RpcClient],
    op: impl Fn(&RpcClient) -> ClientResult<T>,
) -> ClientResult<T> {
    let mut last_err: Option<ClientError> = None;
    for c in clients {
        match op(c) {
            Ok(v) => return Ok(v),
            Err(e) => {
                if is_transient_solana_client_error(&e) {
                    last_err = Some(e);
                    continue;
                }
                return Err(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        ClientError::from(ClientErrorKind::Custom(
            "no Solana RPC endpoints configured".to_string(),
        ))
    }))
}

#[cfg(test)]
mod tests {
    use super::parse_solana_rpc_urls;

    #[test]
    fn parse_solana_rpc_urls_splits_and_trims() {
        let u = parse_solana_rpc_urls(" https://a.test ,https://b.test , ");
        assert_eq!(u, vec!["https://a.test", "https://b.test"]);
    }
}
