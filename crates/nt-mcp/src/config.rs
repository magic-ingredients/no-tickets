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
pub struct EnvConfig {
    pub api_url: String,
    pub token: String,
}

impl EnvConfig {
    /// Read NO_TICKETS_TOKEN + NO_TICKETS_API_URL from the process env.
    /// Returns a user-facing error string naming the missing var when
    /// either is absent or empty.
    ///
    /// Order pinned by the test pair (`publish_event_missing_token_*`
    /// AND `publish_event_missing_api_url_*`): both messages must
    /// surface the var name verbatim so an MCP client can route the
    /// diagnostic to the user's mcp.json.
    pub fn from_env() -> Result<Self, String> {
        let token = read_required("NO_TICKETS_TOKEN")?;
        let api_url = read_required("NO_TICKETS_API_URL")?;
        Ok(Self { api_url, token })
    }
}

fn read_required(name: &str) -> Result<String, String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Ok(v),
        // Treat unset AND empty-string identically: an empty token /
        // empty URL is never a valid configuration, only confusing if
        // accepted then failing downstream with a different error.
        _ => Err(format!(
            "{name} is not set. The MCP server requires it (typically set in your client's mcp.json)."
        )),
    }
}
