//! Body of the `describe_event_type` MCP tool.
//!
//! Mirrors `src/mcp/tools/handlers.ts::handleDescribeEventType`. GETs
//! `/v1/registry/event-types/{id}` with Bearer auth and unwraps the
//! `{ eventType }` response wrapper. Adds a synthesised example payload
//! produced from the embedded JSON Schema via
//! `crate::example_synth::synthesise_example`.
//!
//! Naturally pairs with Task 23 (in-memory registry cache); for now,
//! every invocation is a fresh GET. Cache lives at the
//! list/describe-registry layer once Task 23 lands.
//!
//! RED-phase stub: returns an internal error so the JSON-RPC server
//! stays alive and the failing tests see a structured error response
//! rather than a hung pipe.

use rmcp::{model::*, ErrorData as McpError};
use serde::Deserialize;

use crate::config::EnvConfig;

/// Exact TS-parity description from
/// `src/mcp/tools/describe-event-type.ts`. Pinned here as a constant so
/// the integration test asserts on byte-for-byte equality rather than
/// a substring match. The literal lives in the `#[tool]` attribute over
/// in `server.rs`; rmcp's macro requires a string literal there.
#[allow(dead_code)] // Test-only anchor; the literal lives in the #[tool] attribute.
pub const TS_PARITY_DESCRIPTION: &str = "Return schema, dedupe strategy, retention, and a synthesised example payload for a single event type. Call this before publish_event when you do not already know the schema; the example field is a starting point you can adapt.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DescribeEventTypeArgs {
    /// Event type id (domain.entity.action.vN). Unused at RED — the
    /// GREEN handler consumes it when building the GET path.
    #[allow(dead_code)]
    pub id: String,
}

/// Handle a `tools/call describe_event_type` invocation. Issues a GET
/// against the server's registry detail endpoint, unwraps the
/// `eventType` envelope, validates the embedded schema is present
/// (server-contract violation if not), and surfaces the spec plus a
/// synthesised example payload.
///
/// Takes a shared `&reqwest::Client` for the same reason as
/// `publish_event::handle` — `NtServer` owns the client (with timeout)
/// and threads it through every tool that does HTTP, so the connection
/// pool / TLS state isn't rebuilt per invocation.
pub async fn handle(
    _args: &DescribeEventTypeArgs,
    _config: &EnvConfig,
    _http_client: &reqwest::Client,
) -> Result<CallToolResult, McpError> {
    // RED stub — replaced at GREEN with the GET + unwrap + synthesise
    // implementation. Returning an internal error here keeps the
    // JSON-RPC pipe alive and lets every behavioural test fail with a
    // predictable diagnostic rather than panicking the server.
    Err(McpError::internal_error(
        "describe_event_type not yet implemented (RED phase)".to_string(),
        None,
    ))
}
