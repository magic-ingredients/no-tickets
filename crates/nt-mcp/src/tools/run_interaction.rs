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

use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use rmcp::{model::*, ErrorData as McpError};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::config::EnvConfig;

/// Path-segment encode set — RFC 3986 pchar minus the percent itself.
/// Duplicated from `describe_event_type.rs` per the Task 24 plan:
/// extraction into `nt-core` is the parent fix's mechanism for
/// deduping shared HTTP scaffolding; doing it locally now would be
/// premature when the same code is about to land in
/// `create_subject` too.
const PATH_SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'/')
    .add(b'%');

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunInteractionArgs {
    /// Interaction id. Server-defined; the tool is a thin passthrough
    /// so the id space is whatever the server registry knows.
    pub id: String,
    /// Input payload for the interaction. Forwarded verbatim to the
    /// server; interaction-specific schema validation is server-side
    /// (the per-interaction schema is not bundled in the binary).
    pub input: Map<String, Value>,
    /// Optional subject reference (`{ type, id }`). Mirrors the TS
    /// handler's conditional spread — present in the wire body when
    /// supplied, omitted when not.
    #[serde(default)]
    pub subject: Option<SubjectRef>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SubjectRef {
    /// Subject type. Renamed via serde because `type` is a Rust
    /// keyword; deserialises from the camelCase wire key `type`.
    #[serde(rename = "type")]
    pub subject_type: String,
    /// Subject id (free-form string assigned by the caller).
    pub id: String,
}

/// Handle a `tools/call run_interaction` invocation. Issues a POST
/// against the server's interactions endpoint, parses the
/// `{ events }` response, and surfaces the event list as the tool
/// result.
pub async fn handle(
    args: &RunInteractionArgs,
    config: &EnvConfig,
    http_client: &reqwest::Client,
) -> Result<CallToolResult, McpError> {
    // 1. Build the wire body — `input` is forwarded verbatim; the
    //    optional `subject` is OMITTED when absent (no JSON null) so
    //    the server can distinguish "no subject provided" from
    //    "subject explicitly cleared".
    let mut body = Map::new();
    body.insert("input".to_string(), Value::Object(args.input.clone()));
    if let Some(s) = &args.subject {
        body.insert(
            "subject".to_string(),
            serde_json::json!({ "type": s.subject_type, "id": s.id }),
        );
    }
    let body = Value::Object(body);

    // 2. POST `/v1/interactions/{id}` with Bearer auth. The id is
    //    percent-encoded via the RFC 3986 pchar-minus-percent set so
    //    canonical ids pass through unchanged but pathological inputs
    //    (`/`, `?`, `#`) can't break URL structure.
    let encoded_id = utf8_percent_encode(&args.id, PATH_SEGMENT);
    let url = format!(
        "{}/v1/interactions/{}",
        config.api_url.trim_end_matches('/'),
        encoded_id,
    );
    let response = http_client
        .post(&url)
        .bearer_auth(&config.token)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| McpError::internal_error(format!("transport error: {e}"), None))?;

    // 3. Status-code mapping. 404 → named not-found (so the agent
    //    can correct the id). 401/403 → auth-specific (so MCP clients
    //    know to refresh NO_TICKETS_TOKEN rather than retry blindly).
    //    Other non-2xx → transport error carrying status + upstream
    //    body for diagnostics.
    let status = response.status();
    match status.as_u16() {
        404 => {
            return Err(McpError::invalid_params(
                format!("interaction \"{}\" not found", args.id),
                None,
            ));
        }
        401 | 403 => {
            let code = status.as_u16();
            return Err(McpError::invalid_params(
                format!(
                    "authentication failed ({code}) — check NO_TICKETS_TOKEN; the server rejected the bearer credential"
                ),
                None,
            ));
        }
        _ => {}
    }
    let body_text = response.text().await.map_err(|e| {
        McpError::internal_error(format!("transport error reading body: {e}"), None)
    })?;
    if !status.is_success() {
        return Err(McpError::internal_error(
            format!("server returned {}: {}", status.as_u16(), body_text),
            None,
        ));
    }

    // 4. Parse `{ events: [...] }`. A 2xx without the `events` field
    //    is a server-contract violation — surface loudly rather than
    //    rendering an empty list. Mirrors the describe_event_type
    //    missing-schema guard.
    let parsed: Value = serde_json::from_str(&body_text).map_err(|e| {
        McpError::internal_error(format!("invalid server JSON response: {e}"), None)
    })?;
    if parsed.get("events").and_then(Value::as_array).is_none() {
        return Err(McpError::internal_error(
            format!(
                "server-contract violation: interaction \"{}\" response is missing the events array",
                args.id,
            ),
            None,
        ));
    }

    // 5. Passthrough: surface `{ events }` exactly as the server
    //    returned. Matches the TS handler's `{ events: response.events }`.
    let result = serde_json::json!({ "events": parsed["events"].clone() });
    let text = serde_json::to_string(&result).expect("events array is always serialisable");
    Ok(CallToolResult::success(vec![Content::text(text)]))
}
