//! `nt-mcp` server: rmcp tool routing + ServerHandler impl.
//!
//! Tool bodies live in `crates/nt-mcp/src/tools/<name>.rs` so the
//! impl block stays a thin dispatch layer as Task 5 adds the rest of
//! the surface (describe_event_type, publish_event, status, validate,
//! create_subject, run_interaction).

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router,
};

use crate::fixtures::{EventTypeRow, all_event_types};
use crate::tools::list_event_types::{self, ListEventTypesArgs};

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
    fixtures: &'static [EventTypeRow],
}

impl NtServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            fixtures: all_event_types(),
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
    #[tool(description = "List event types this caller can publish, optionally filtered by domain. Type ids follow domain.entity.action.vN grammar. Reads from the local cache; refresh fires async.")]
    fn list_event_types(
        &self,
        Parameters(args): Parameters<ListEventTypesArgs>,
    ) -> Result<CallToolResult, McpError> {
        list_event_types::handle(&args, self.fixtures)
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
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(info)
    }
}
