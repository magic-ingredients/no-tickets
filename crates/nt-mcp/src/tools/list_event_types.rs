//! Body of the `list_event_types` MCP tool.
//!
//! Lives in its own module per the fix doc's planned layout
//! (crates/nt-mcp/src/tools/list_event_types.rs). server.rs registers
//! the tool with rmcp's `#[tool_router]` macro and delegates the body
//! here, so Task 5 (full MCP surface) can add tools alongside without
//! the impl block becoming a god struct.

use rmcp::{ErrorData as McpError, model::*};
use serde::{Deserialize, Serialize};

use crate::fixtures::EventTypeRow;

/// Exact TS-parity description from src/mcp/tools/list-event-types.ts.
/// Pinned here as a constant so the integration test asserts on
/// byte-for-byte equality rather than a substring match.
pub const TS_PARITY_DESCRIPTION: &str = "List event types this caller can publish, optionally filtered by domain. Type ids follow domain.entity.action.vN grammar. Reads from the local cache; refresh fires async.";

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
struct Payload<'a> {
    types: Vec<&'a EventTypeRow>,
}

pub fn handle(
    args: &ListEventTypesArgs,
    fixtures: &'static [EventTypeRow],
) -> Result<CallToolResult, McpError> {
    tracing::info!(
        domain = args.domain.as_deref(),
        deprecated = args.deprecated,
        "list_event_types called",
    );

    let filtered: Vec<&EventTypeRow> = fixtures
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

    let payload = Payload { types: filtered };
    let json = serde_json::to_string(&payload)
        .expect("Payload always serialises");
    Ok(CallToolResult::success(vec![Content::text(json)]))
}
