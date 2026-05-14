//! Body of the `publish_event` MCP tool.
//!
//! Mirrors `src/mcp/tools/handlers.ts::handlePublishEvent`. Validates
//! the payload locally against the bundled JSON Schema (via
//! `nt_schemas::validate`) before any network call, then POSTs the
//! envelope to `/v1/events` with Bearer auth.
//!
//! Source identity (`source.name`) is fixed at `"nt-mcp"` and cannot
//! be overridden by the agent — matches the TS reference, where the
//! MCP server fills source server-side. Source attributes carry the
//! `project` arg (NOT used for token routing — single-token per MCP
//! server instance per `config::EnvConfig`).
//!
//! Retry is intentionally OUT of scope for this slice. The Task 17
//! retry policy is unlikely to fit MCP's "tool call returns
//! immediately" lifecycle the same way it fits a CLI publish, and
//! adding it without a real use case would be speculative. Wrapper
//! callers can implement their own retry around the MCP tool call.
//! See Task 24 for the eventual shared-retry extraction.

use rmcp::{model::*, ErrorData as McpError};
use serde::Deserialize;
use serde_json::Value;

use crate::config::EnvConfig;

/// Exact TS-parity description from `src/mcp/tools/publish-event.ts`.
/// Pinned here as a constant so the integration test asserts on byte-
/// for-byte equality rather than a substring match. Referenced only
/// from the integration test (`tests/mcp.rs`) — the `#[tool]` macro
/// in `server.rs` requires a literal, so the constant can't be passed
/// to it directly.
#[allow(dead_code)] // Test-only reference; the literal lives in the #[tool] attribute.
pub const TS_PARITY_DESCRIPTION: &str = "Publish a single event. Call describe_event_type first to confirm the schema; the server will reject mismatches. Source metadata is filled server-side and cannot be overridden.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[allow(dead_code)] // Fields are consumed by Task 19 GREEN.
pub struct PublishEventArgs {
    /// Project name; appears in `source.attributes.project` on the
    /// wire. Does NOT route to a different token — the MCP server is
    /// single-token per invocation (see `config::EnvConfig`).
    pub project: String,
    /// Event type id (domain.entity.action.vN).
    #[serde(rename = "type")]
    pub type_id: String,
    /// Event payload matching the type schema.
    pub data: Value,
    /// Optional subject reference.
    #[serde(default)]
    pub subject: Option<SubjectRef>,
    /// Optional ISO-8601 timestamp; defaults to now server-side.
    #[serde(default, rename = "occurred_at")]
    pub occurred_at: Option<String>,
    /// Optional parent event id.
    #[serde(default, rename = "parent_event_id")]
    pub parent_event_id: Option<String>,
    /// Optional trace id.
    #[serde(default, rename = "trace_id")]
    pub trace_id: Option<String>,
    /// Optional idempotency key.
    #[serde(default, rename = "dedupe_key")]
    pub dedupe_key: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[allow(dead_code)] // Fields are consumed by Task 19 GREEN.
pub struct SubjectRef {
    #[serde(rename = "type")]
    pub subject_type: String,
    pub id: String,
}

/// Handle a `tools/call publish_event` invocation. Resolves auth from
/// `config`, validates `args.data` locally, posts the envelope, maps
/// the response to a `CallToolResult`.
///
/// Production callers pass the real `reqwest::Client`-backed transport
/// (lives inline in this module for the Task 19 slice; extracted to
/// `nt-core` in Task 24). Tests may inject a fake via the same trait
/// once the testable seam lands in GREEN.
pub async fn handle(
    _args: &PublishEventArgs,
    _config: &EnvConfig,
) -> Result<CallToolResult, McpError> {
    // RED-phase sentinel: returns an MCP error rather than `panic!`-ing
    // via `unimplemented!()` so the JSON-RPC server doesn't crash mid-
    // request. Behavior tests see a structured `tools/call` error
    // response instead of a hung pipe / dropped connection. GREEN
    // replaces this body with the real handler.
    Err(McpError::internal_error(
        "Task 19 GREEN — publish_event handler not yet implemented".to_string(),
        None,
    ))
}
