//! Secret redaction for sensitive data in logs, serialization, and display.
//!
//! Use [`Redacted`] to wrap values that must never appear in logs, error messages,
//! or serialized output (e.g., JSON). The wrapped value is never exposed through
//! `Debug`, `Display`, or `Serialize` — all output as `"<redacted>"`.

use std::fmt::{self, Debug, Display};

/// Wrapper that redacts its inner value when formatted or serialized.
///
/// Use for tokens, API keys, passwords, private keys, mnemonics, or any
/// value that must not appear in logs, error messages, or structured output.
///
/// # Example
///
/// ```ignore
/// use multichain_rs::redact::Redacted;
///
/// let api_key = "sk-12345";
/// tracing::info!(key = %Redacted(api_key), "Making request");
/// // Logs: key = <redacted>
/// ```
#[derive(Clone, Copy)]
pub struct Redacted<T>(pub T);

impl<T> Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl<T> Display for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl<T> serde::Serialize for Redacted<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        "<redacted>".serialize(serializer)
    }
}
