//! Generic error type for nt-core primitives.
//!
//! Carries raw HTTP facts (status + body) and transport / parse
//! failure messages. Consumers (nt-cli's `TransportError`, nt-mcp's
//! per-tool `McpError` mapping) implement `From<nt_core::Error>` or
//! local match arms to convert into their own typed errors — there
//! is intentionally no semantic mapping (e.g. 404 → "not found") in
//! nt-core, because the right wording is consumer-specific.

use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// reqwest-layer failure: connect refused, DNS lookup failed,
    /// TLS handshake failed, request timeout, etc. Carries the
    /// reqwest error's display text for diagnostics.
    Transport(String),

    /// Couldn't read the response body to bytes / UTF-8 string.
    /// Distinct from `Transport` so callers can tell "got a status
    /// line but then the body stream died" from "never reached the
    /// server".
    Body(String),

    /// Response was 2xx but the body wasn't valid JSON (when the
    /// caller asked for a JSON response). Carries the serde_json
    /// error message.
    InvalidJson(String),

    /// Non-2xx response. Status code + upstream body verbatim so
    /// callers can surface the server's own error message to the
    /// user / agent. Callers do their own status-code switching
    /// (401/403/404 mapping varies per consumer).
    HttpStatus { status: u16, body: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(msg) => write!(f, "transport error: {msg}"),
            Self::Body(msg) => write!(f, "transport error reading body: {msg}"),
            Self::InvalidJson(msg) => write!(f, "invalid JSON response: {msg}"),
            Self::HttpStatus { status, body } => write!(f, "server returned {status}: {body}"),
        }
    }
}

impl std::error::Error for Error {}
