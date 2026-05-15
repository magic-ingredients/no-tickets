//! Generic error type for nt-core primitives.
//!
//! Carries raw HTTP facts (status + body) and transport / parse
//! failure messages. Consumers (nt-cli's `TransportError`, nt-mcp's
//! per-tool `McpError` mapping) implement `From<nt_core::Error>` or
//! local match arms to convert into their own typed errors — there
//! is intentionally no semantic mapping (e.g. 404 → "not found") in
//! nt-core, because the right wording is consumer-specific.
//!
//! ## Why `Transport(String)` instead of `Transport(reqwest::Error)`
//!
//! Stringifying the reqwest error at the boundary is a deliberate
//! trade-off, not an oversight. Keeping it as a typed value would
//! let consumers introspect `is_timeout()` / `is_connect()` /
//! `is_decode()` for finer-grained retry / categorisation. The
//! costs that pushed it to a `String`:
//!
//! - Forces every consumer to depend on `reqwest` (defeats the
//!   "thin shared core" goal — nt-cli already does, but nt-mcp
//!   could swap its HTTP layer in future without dragging reqwest
//!   types through its public surface).
//! - Couples nt-core's API to a major version of reqwest. A reqwest
//!   bump becomes a breaking change for every consumer instead of
//!   a contained one inside nt-core.
//!
//! The trade-off binds nt-cli's existing `TransportError::Network
//! (reqwest::Error)` shape — that's why nt-cli is NOT migrated to
//! nt-core in this commit. Task 25 either adapts nt-cli's
//! `TransportError` to the string-based variant (losing per-error
//! introspection) or keeps its own thin reqwest layer for now.

use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// reqwest-layer failure: connect refused, DNS lookup failed,
    /// TLS handshake failed, request timeout, etc. Carries the
    /// reqwest error's display text — see module docs for why this
    /// isn't the typed `reqwest::Error`.
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
            // "invalid server JSON response" matches the wording the
            // nt-mcp integration tests pin verbatim
            // (publish_event_… and describe_event_type_non_json_body).
            // Drift between this Display and the adapter wording in
            // nt-mcp::error_map flagged by the adversarial review.
            Self::InvalidJson(msg) => write!(f, "invalid server JSON response: {msg}"),
            Self::HttpStatus { status, body } => write!(f, "server returned {status}: {body}"),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::Error;

    /// Pin each variant's `Display` output. Consumers (notably the
    /// `nt-mcp::error_map::transport_to_mcp` adapter) rely on
    /// `to_string()` as the single source of wording — an empty
    /// Display impl would silently produce empty `McpError` messages
    /// only caught at the cross-crate integration-test layer.
    /// cargo-mutants flagged a `fmt -> Ok(())` survivor here; these
    /// tests close that gap inside nt-core.
    #[test]
    fn display_transport_includes_payload() {
        let msg = Error::Transport("dns failed".into()).to_string();
        assert_eq!(msg, "transport error: dns failed");
    }

    #[test]
    fn display_body_includes_payload() {
        let msg = Error::Body("stream ended".into()).to_string();
        assert_eq!(msg, "transport error reading body: stream ended");
    }

    #[test]
    fn display_invalid_json_uses_canonical_wording() {
        // "invalid server JSON response" is the wording the nt-mcp
        // integration tests pin (`describe_event_type_non_json_body`,
        // `run_interaction_non_json_response_…`). Drift was flagged
        // by the adversarial review.
        let msg = Error::InvalidJson("expected ident".into()).to_string();
        assert_eq!(msg, "invalid server JSON response: expected ident");
    }

    #[test]
    fn display_http_status_includes_status_and_body() {
        let msg = Error::HttpStatus {
            status: 503,
            body: "down".into(),
        }
        .to_string();
        assert_eq!(msg, "server returned 503: down");
    }
}
