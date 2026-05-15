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

impl NtServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            registry: RegistryCache::new(),
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
        // + version to the TS parity values.
        let mut info = Implementation::from_build_env();
        info.name = SERVER_NAME.to_string();
        info.version = env!("CARGO_PKG_VERSION").to_string();
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(info)
    }
}
