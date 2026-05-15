//! Body of the `publish_event` MCP tool.
//!
//! Mirrors `src/mcp/tools/handlers.ts::handlePublishEvent`. Validates
//! the payload locally against the bundled JSON Schema (via
//! `nt_schemas::validate`) before any network call, then POSTs the
//! envelope to `/v1/events` with Bearer auth.
//!
//! Source identity (`source.name`) is fixed at `"nt-mcp"` and cannot
//! be overridden by the agent — matches the TS reference, where the
//! MCP server fills source server-side. No `source.attributes` block:
//! project tenancy is server-resolved from the push token
//! (`pushToken.projectId` in
//! `notickets-service/src/server/routes/events.ts`), so a client-
//! supplied `project` label would be advisory-only and never
//! consulted for routing, authz, or counting.
//!
//! Retry is intentionally OUT of scope for this slice. The Task 17
//! retry policy is unlikely to fit MCP's "tool call returns
//! immediately" lifecycle the same way it fits a CLI publish, and
//! adding it without a real use case would be speculative. Wrapper
//! callers can implement their own retry around the MCP tool call.
//! See Task 24 for the eventual shared-retry extraction.

use nt_schemas::validate;
use rmcp::{model::*, ErrorData as McpError};
use serde::Deserialize;
use serde_json::{Map, Value};

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
pub struct PublishEventArgs {
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
pub struct SubjectRef {
    #[serde(rename = "type")]
    pub subject_type: String,
    pub id: String,
}

/// Handle a `tools/call publish_event` invocation. Resolves auth from
/// `config`, validates `args.data` locally, posts the envelope, maps
/// the response to a `CallToolResult`.
///
/// Takes a shared `&reqwest::Client` rather than constructing one per
/// call — `NtServer` owns the client (with the per-request timeout
/// configured) and threads it through every tool that does HTTP. A
/// per-call `Client::new()` would re-init TLS state and rebuild the
/// connection pool every invocation, costing real wall-clock on every
/// publish.
pub async fn handle(
    args: &PublishEventArgs,
    config: &EnvConfig,
    http_client: &reqwest::Client,
) -> Result<CallToolResult, McpError> {
    // 1. Local schema validation — gates before any HTTP. Mirrors the
    //    TS handler's `validateAgainstBundledSchema` step. Unknown type
    //    short-circuits with a typed McpError; schema-fail surfaces
    //    per-issue paths so the agent knows what to fix.
    match validate(&args.type_id, &args.data) {
        None => {
            return Err(McpError::invalid_params(
                format!("unknown event type \"{}\"", args.type_id),
                None,
            ));
        }
        Some(issues) if !issues.is_empty() => {
            let mut msg = format!(
                "{}: {} schema validation error(s):",
                args.type_id,
                issues.len()
            );
            for issue in &issues {
                msg.push_str(&format!("\n  {}: {}", issue.path, issue.message));
            }
            return Err(McpError::invalid_params(msg, None));
        }
        Some(_) => {}
    }

    // 2. Build the envelope. Source identity is fixed: `source.name =
    //    "nt-mcp"`, no `attributes`. The agent cannot override
    //    `source` — the input schema doesn't expose it (pinned by the
    //    discovery test).
    let envelope = build_envelope(args);

    // 3. POST /v1/events with Bearer auth. Single attempt — retry is
    //    deferred per the Task 19 scope note. A 5xx after one try
    //    surfaces as a transport error; the agent / MCP client can
    //    retry the tool call itself if it wants.
    let url = format!("{}/v1/events", config.api_url.trim_end_matches('/'));
    let response = http_client
        .post(&url)
        .bearer_auth(&config.token)
        .header("content-type", "application/json")
        .json(&vec![envelope])
        .send()
        .await
        .map_err(|e| McpError::internal_error(format!("transport error: {e}"), None))?;

    let status = response.status();
    let body_text = response.text().await.map_err(|e| {
        McpError::internal_error(format!("transport error reading body: {e}"), None)
    })?;
    if !status.is_success() {
        return Err(McpError::internal_error(
            format!("server returned {}: {}", status.as_u16(), body_text),
            None,
        ));
    }

    // 4. Parse server response and build the tool result. Mirrors the
    //    TS handler's `{ id, deduped }` shape. `deduped: true` only
    //    when the server reports the event went through dedupe rather
    //    than fresh ingestion.
    let parsed: Value = serde_json::from_str(&body_text).map_err(|e| {
        McpError::internal_error(format!("invalid server JSON response: {e}"), None)
    })?;
    let id = parsed["ids"][0].as_str().ok_or_else(|| {
        McpError::internal_error("server response missing ids[0]".to_string(), None)
    })?;
    let ingested = parsed["ingested"].as_u64().unwrap_or(0);
    let deduped = parsed["deduped"].as_u64().unwrap_or(0);
    let result_payload = serde_json::json!({
        "id": id,
        "deduped": ingested == 0 && deduped > 0,
    });
    let text =
        serde_json::to_string(&result_payload).expect("simple JSON object always serialises");
    Ok(CallToolResult::success(vec![Content::text(text)]))
}

/// Build the wire envelope from the tool args. Pure given inputs.
/// Pinned shape: `type`, `data`, `subject?`, `source`,
/// `parentEventId?`, `traceId?`, `dedupeKey?`, `occurredAt?` — the
/// optional fields are OMITTED when absent (no JSON `null`, no empty
/// string), matching the TS `eventSchema` emission order and the
/// `nt publish` envelope from Task 15.
fn build_envelope(args: &PublishEventArgs) -> Value {
    let mut envelope = Map::new();
    envelope.insert("type".to_string(), Value::String(args.type_id.clone()));
    envelope.insert("data".to_string(), args.data.clone());
    if let Some(s) = &args.subject {
        envelope.insert(
            "subject".to_string(),
            serde_json::json!({ "type": s.subject_type, "id": s.id }),
        );
    }
    envelope.insert("source".to_string(), build_source());
    if let Some(p) = &args.parent_event_id {
        envelope.insert("parentEventId".to_string(), Value::String(p.clone()));
    }
    if let Some(t) = &args.trace_id {
        envelope.insert("traceId".to_string(), Value::String(t.clone()));
    }
    if let Some(d) = &args.dedupe_key {
        envelope.insert("dedupeKey".to_string(), Value::String(d.clone()));
    }
    if let Some(o) = &args.occurred_at {
        envelope.insert("occurredAt".to_string(), Value::String(o.clone()));
    }
    Value::Object(envelope)
}

/// Build the source identity attached to every MCP-published event.
/// `name = "nt-mcp"` is fixed — the agent cannot spoof its source via
/// tool args. No `attributes` block: the project is server-resolved
/// from the push token (`pushToken.projectId` in
/// `notickets-service/src/server/routes/events.ts`), so a client-
/// supplied label would be advisory-only and the wire-stored value
/// would never be consulted for routing, authz, or counting. Keeping
/// the slot empty avoids the lying-shaped field problem.
fn build_source() -> Value {
    serde_json::json!({
        "name": "nt-mcp",
        "sdkVersion": env!("CARGO_PKG_VERSION"),
    })
}
