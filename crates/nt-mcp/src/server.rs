//! `nt-mcp` server: rmcp tool routing + ServerHandler impl.
//!
//! Tool bodies live in `crates/nt-mcp/src/tools/<name>.rs` so the
//! impl block stays a thin dispatch layer for the three-tool surface:
//! `list_event_types`, `describe_event_type`, `publish_event`.
//! `run_interaction` and `create_subject` were dropped (Tasks 21 + 22
//! superseded on 2026-05-15) — workflows are event sequences with a
//! shared run_id, and no production subject types are registered
//! server-side.

use std::time::Duration;

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};

use crate::config::EnvConfig;
use crate::registry_cache::RegistryCache;
use crate::tools::describe_event_type::{self, DescribeEventTypeArgs};
use crate::tools::list_event_types::{self, ListEventTypesArgs};
use crate::tools::publish_event::{self, PublishEventArgs};

/// Per-request timeout for outbound HTTP calls. Matches `nt-cli`'s
/// `DEFAULT_TIMEOUT`. A hung upstream must not block the JSON-RPC
/// stdio pipe indefinitely — without this, the MCP client would have
/// to enforce its own timeout from the outside.
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// Reported `serverInfo.name` in the initialize response. Matches the TS
/// server (src/mcp/create-server.ts), which reports `no-tickets` —
/// preserving wire parity for any client that pins on this string.
const SERVER_NAME: &str = "no-tickets";

#[derive(Clone)]
pub struct NtServer {
    // The macro-generated tool_handler reads this field reflectively;
    // the dead-code analyser doesn't see that path. Narrow allow.
    #[allow(dead_code)]
    tool_router: ToolRouter<NtServer>,
    /// Process-lifetime in-memory cache for the event-type registry.
    /// Populated on first list_event_types call, refreshed
    /// opportunistically on subsequent ones. Shared (clone is cheap —
    /// Arc inside) across cloned NtServer instances so rmcp's
    /// per-request handler cloning doesn't reset the cache.
    registry: RegistryCache,
    /// Shared HTTP client for outbound calls (list_event_types,
    /// publish_event, describe_event_type). `reqwest::Client` is
    /// `Clone`-cheap — it's `Arc`-internal — so handing it to each
    /// tool handler doesn't duplicate connection pools / TLS state.
    http_client: reqwest::Client,
}

/// Default throttle for the registry cache's opportunistic async
/// refresh. A busy MCP session calling `list_event_types` rapidly must
/// NOT translate into one outbound GET per call against the registry
/// — that's wasteful and risks self-DoS-pressure. 5 seconds is short
/// enough to feel fresh, long enough to coalesce bursts. Override via
/// `NT_REGISTRY_REFRESH_INTERVAL_MS` (integration tests use `0` to
/// observe refresh behaviour deterministically).
const DEFAULT_REGISTRY_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

fn registry_refresh_interval() -> Duration {
    parse_registry_refresh_interval(std::env::var("NT_REGISTRY_REFRESH_INTERVAL_MS").ok().as_deref())
}

/// Pure parser for `NT_REGISTRY_REFRESH_INTERVAL_MS`. Extracted so the
/// env-var contract (default, parse failure, valid override) is unit-
/// testable without touching the global env.
fn parse_registry_refresh_interval(raw: Option<&str>) -> Duration {
    raw.and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_REGISTRY_REFRESH_INTERVAL)
}

impl NtServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            registry: RegistryCache::new(registry_refresh_interval()),
            http_client: reqwest::Client::builder()
                .timeout(HTTP_TIMEOUT)
                .build()
                .expect("reqwest client build (rustls-tls features always present)"),
        }
    }
}

impl Default for NtServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl NtServer {
    // Description literal MUST stay byte-for-byte in sync with
    // `tools::list_event_types::TS_PARITY_DESCRIPTION` — the rmcp
    // `#[tool]` attribute requires a string literal, so the constant
    // can't be referenced here directly. The integration test asserts
    // byte-equality against the constant, so any drift fails CI.
    #[tool(
        description = "List event types this caller can publish, optionally filtered by domain. Type ids follow domain.entity.action.vN grammar. Reads from the local cache; refresh fires async."
    )]
    async fn list_event_types(
        &self,
        Parameters(args): Parameters<ListEventTypesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let config = EnvConfig::from_env().map_err(|msg| McpError::invalid_params(msg, None))?;
        list_event_types::handle(&args, &config, &self.http_client, &self.registry).await
    }

    // Description MUST stay byte-for-byte in sync with
    // `tools::publish_event::TS_PARITY_DESCRIPTION`. Same constraint
    // as above — rmcp's `#[tool]` requires a string literal.
    //
    // Env config is resolved lazily on each call: `EnvConfig::from_
    // env()` reads NO_TICKETS_TOKEN + NO_TICKETS_API_URL. A missing
    // var surfaces as a not-authenticated MCP error rather than
    // failing the server at boot — keeps the server alive so other
    // (auth-not-required) tools remain callable in the same session.
    #[tool(
        description = "Publish a single event. Call describe_event_type first to confirm the schema; the server will reject mismatches. Source metadata is filled server-side and cannot be overridden."
    )]
    async fn publish_event(
        &self,
        Parameters(args): Parameters<PublishEventArgs>,
    ) -> Result<CallToolResult, McpError> {
        let config = EnvConfig::from_env().map_err(|msg| McpError::invalid_params(msg, None))?;
        publish_event::handle(&args, &config, &self.http_client).await
    }

    // Description MUST stay byte-for-byte in sync with
    // `tools::describe_event_type::TS_PARITY_DESCRIPTION`. rmcp's
    // `#[tool]` macro requires a string literal so the constant can't
    // be referenced directly here.
    //
    // Env resolution is lazy, matching `publish_event` — a missing
    // NO_TICKETS_TOKEN / NO_TICKETS_API_URL surfaces per-call rather
    // than failing the server at boot, so the auth-not-required tools
    // remain callable in the same session.
    #[tool(
        description = "Return schema, dedupe strategy, retention, and a synthesised example payload for a single event type. Call this before publish_event when you do not already know the schema; the example field is a starting point you can adapt."
    )]
    async fn describe_event_type(
        &self,
        Parameters(args): Parameters<DescribeEventTypeArgs>,
    ) -> Result<CallToolResult, McpError> {
        let config = EnvConfig::from_env().map_err(|msg| McpError::invalid_params(msg, None))?;
        describe_event_type::handle(&args, &config, &self.http_client).await
    }
}

#[tool_handler]
impl ServerHandler for NtServer {
    fn get_info(&self) -> ServerInfo {
        // `Implementation` is `#[non_exhaustive]`, so direct struct
        // construction is disallowed. Start from the build-env default
        // (carries crate version, sensible defaults) and override name
        // + version to the pinned values.
        let mut info = Implementation::from_build_env();
        info.name = SERVER_NAME.to_string();
        info.version = env!("CARGO_PKG_VERSION").to_string();
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_registry_refresh_interval_uses_default_when_env_absent() {
        assert_eq!(
            parse_registry_refresh_interval(None),
            DEFAULT_REGISTRY_REFRESH_INTERVAL,
        );
    }

    #[test]
    fn parse_registry_refresh_interval_uses_default_when_env_unparseable() {
        // Garbage values fall back to the default — a malformed env
        // var must NOT silently turn into a zero throttle and re-
        // enable per-call refresh.
        assert_eq!(
            parse_registry_refresh_interval(Some("not-a-number")),
            DEFAULT_REGISTRY_REFRESH_INTERVAL,
        );
    }

    #[test]
    fn parse_registry_refresh_interval_parses_zero_for_test_override() {
        // Integration tests rely on this exact contract: setting the
        // env to "0" must produce Duration::ZERO so refresh-observing
        // tests can fire the spawn on every call.
        assert_eq!(parse_registry_refresh_interval(Some("0")), Duration::ZERO);
    }

    #[test]
    fn parse_registry_refresh_interval_parses_valid_milliseconds() {
        assert_eq!(
            parse_registry_refresh_interval(Some("1234")),
            Duration::from_millis(1234),
        );
    }

    /// Integration of the env-reader + parser: with the env unset
    /// in this process (cargo test inherits a clean env unless a
    /// caller exports the var), the function must return the
    /// non-zero default. Pins that the wrapper actually delegates
    /// to the parser rather than collapsing to `Duration::default()`.
    #[test]
    fn registry_refresh_interval_returns_default_when_env_unset() {
        // No in-process std::env::set_var anywhere in this crate's
        // tests — the integration tests pass NT_REGISTRY_REFRESH_
        // INTERVAL_MS through the spawned child's env, not via the
        // parent test process. So this read is deterministic.
        let got = registry_refresh_interval();
        assert!(
            got > Duration::ZERO,
            "registry_refresh_interval must return the non-zero default when env is unset; got {got:?}",
        );
        assert_eq!(got, DEFAULT_REGISTRY_REFRESH_INTERVAL);
    }
}
