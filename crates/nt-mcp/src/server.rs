//! `nt-mcp` server: a single tool (`list_event_types`) over rmcp's stdio
//! transport. Spike scope per the cross-platform-cli-binary fix Task 2:
//! validate the rmcp toolchain, prove stdout-purity discipline, confirm
//! TS parity on tool descriptor + payload shape.

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};

use crate::fixtures::{EventTypeRow, all_event_types};

/// Arguments to the `list_event_types` tool. Both optional — matches the
/// TS reference at src/mcp/tools/list-event-types.ts.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct ListEventTypesArgs {
    /// Filter to a single domain prefix.
    #[serde(default)]
    pub domain: Option<String>,
    /// When true, return ONLY deprecated types; when false, only active.
    #[serde(default)]
    pub deprecated: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ListEventTypesPayload<'a> {
    types: Vec<&'a EventTypeRow>,
}

#[derive(Clone)]
pub struct NtServer {
    // The macro-generated tool_handler reads this field reflectively;
    // the dead-code analyser doesn't see that path. Suppress the warning
    // narrowly rather than allowing it crate-wide.
    #[allow(dead_code)]
    tool_router: ToolRouter<NtServer>,
    fixtures: Vec<EventTypeRow>,
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
    /// Description text deliberately mirrors the TS implementation so a
    /// shared client sees identical tool metadata across runtimes (see
    /// src/mcp/tools/list-event-types.ts).
    #[tool(
        description = "List event types this caller can publish, optionally filtered by domain. Type ids follow domain.entity.action.vN grammar. Reads from the local cache; refresh fires async."
    )]
    fn list_event_types(
        &self,
        Parameters(args): Parameters<ListEventTypesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let filtered: Vec<&EventTypeRow> = self
            .fixtures
            .iter()
            .filter(|t| match &args.domain {
                Some(d) => t.domain == *d,
                None => true,
            })
            .filter(|t| match args.deprecated {
                Some(want) => t.deprecated == want,
                None => true,
            })
            .collect();

        let payload = ListEventTypesPayload { types: filtered };
        let json = serde_json::to_string(&payload)
            .expect("ListEventTypesPayload always serialises");
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[tool_handler]
impl ServerHandler for NtServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder().enable_tools().build(),
        )
        .with_server_info(Implementation::from_build_env())
    }
}
