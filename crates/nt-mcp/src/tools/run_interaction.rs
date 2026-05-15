//! Body of the `run_interaction` MCP tool.
//!
//! Mirrors `src/mcp/tools/handlers.ts::handleRunInteraction`. POSTs
//! `{ input, subject? }` to `/v1/interactions/{id}` and returns the
//! server-emitted event list verbatim. The tool is a thin passthrough:
//! the server orchestrates the compound action and decides which
//! events to emit. No client-side validation of `input` — the
//! interaction schema is server-defined and not bundled.
//!
//! Naturally pairs with Task 24 (`nt-core` extraction); the URL
//! builder, bearer-auth scaffolding, and status-code mapping all
//! mirror `describe_event_type.rs` and would dedupe into the shared
//! crate once a second cross-tool consumer pins the shape.
//!
//! RED-phase stub: returns an internal error so the JSON-RPC pipe
//! stays alive and failing tests see a structured error response
//! rather than a hung pipe.

use rmcp::{model::*, ErrorData as McpError};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::config::EnvConfig;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunInteractionArgs {
    /// Interaction id (unused at RED — GREEN handler builds the POST
    /// path from it).
    #[allow(dead_code)]
    pub id: String,
    /// Input payload for the interaction. Forwarded verbatim to the
    /// server; interaction-specific schema validation is server-side.
    #[allow(dead_code)]
    pub input: Map<String, Value>,
    /// Optional subject reference (`{ type, id }`).
    #[serde(default)]
    #[allow(dead_code)]
    pub subject: Option<SubjectRef>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SubjectRef {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub subject_type: String,
    #[allow(dead_code)]
    pub id: String,
}

/// Handle a `tools/call run_interaction` invocation. Issues a POST
/// against the server's interactions endpoint, parses the
/// `{ events }` response, and surfaces the event list as the tool
/// result.
pub async fn handle(
    _args: &RunInteractionArgs,
    _config: &EnvConfig,
    _http_client: &reqwest::Client,
) -> Result<CallToolResult, McpError> {
    // RED stub — replaced at GREEN with the POST + parse impl. An
    // internal error keeps the JSON-RPC pipe alive while every
    // behaviour test fails predictably.
    Err(McpError::internal_error(
        "run_interaction not yet implemented (RED phase)".to_string(),
        None,
    ))
}
