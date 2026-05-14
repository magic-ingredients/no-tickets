//! Runtime configuration for nt-mcp, read from environment variables.
//!
//! The MCP server is spawned by its client (Claude Code etc.) with env
//! vars configured in the client's `mcp.json`. Unlike `nt-cli`, there's
//! no credentials-file fallback, no interactive browser flow, no
//! project registry lookup. Auth resolution is single-token / single-
//! project per server invocation:
//!
//! - `NO_TICKETS_TOKEN` (required) — Bearer token sent on every publish.
//! - `NO_TICKETS_API_URL` (required) — base URL for `/v1/events`.
//!
//! Future tasks may broaden this (NO_TICKETS_ENV preset support,
//! NO_TICKETS_AUTH_URL pairing) when shared with `nt-cli` via the
//! Task 24 `nt-core` extraction.

#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields are read by Task 19 GREEN.
pub struct EnvConfig {
    pub api_url: String,
    pub token: String,
}

impl EnvConfig {
    /// Read NO_TICKETS_TOKEN + NO_TICKETS_API_URL from the process env.
    /// Returns a user-facing error string naming the missing var when
    /// either is absent or empty.
    pub fn from_env() -> Result<Self, String> {
        unimplemented!("Task 19 GREEN — read NO_TICKETS_TOKEN + NO_TICKETS_API_URL")
    }
}
