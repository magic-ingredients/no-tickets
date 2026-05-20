//! Body of the `publish_event` MCP tool.
//!
//! Validates the payload locally against the bundled JSON Schema (via
//! `nt_schemas::validate`) before any network call, then POSTs the
//! envelope to `/v1/events` with Bearer auth.
//!
//! Source identity (`source.name`) is fixed at `"no-tickets-mcp"` and
//! cannot be overridden by the agent. Project tenancy is server-
//! resolved from the push token; there is no `project` arg.
//!
//! Subjects are intentionally absent: the wire envelope retains a
//! `subject` slot for forward-compat, but neither this tool nor the
//! CLI populates it today. Re-introduce when subjects ship server-side.
//!
//! `source.attributes` is exposed as the **client passthrough slot**:
//! flat `Record<string, string | number | boolean>` of user-supplied
//! labels (`ticketUrl`, `submittedBy`, free-form tags). The server
//! stores them verbatim in `events.source_metadata` JSONB and
//! surfaces them in the UI; it does NOT interpret them for routing,
//! authz, or counting. Scalar-only is deliberate — flat keys keep
//! JSONB queries cheap, aggregate cleanly, and stop callers from
//! smuggling typed payloads into an untyped slot. If you have
//! nested structure, that data belongs in `data` under an event-type
//! schema, not in attributes.
//!
//! Retry is intentionally OUT of scope for this slice. The Task 17
//! retry policy is unlikely to fit MCP's "tool call returns
//! immediately" lifecycle the same way it fits a CLI publish, and
//! adding it without a real use case would be speculative. Wrapper
//! callers can implement their own retry around the MCP tool call.
//! See Task 24 for the eventual shared-retry extraction.

use std::collections::BTreeMap;

use nt_core::http::post_json;
use nt_core::url::api_url;
use nt_schemas::validate;
use rmcp::{model::*, ErrorData as McpError};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

use crate::config::EnvConfig;
use crate::error_map::transport_to_mcp;

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
    /// Optional client passthrough labels — flat
    /// `{ string: string | number | boolean }` map. Land on
    /// `source.attributes` on the wire. Server stores verbatim and
    /// surfaces in the UI; scalar-only constraint is deliberate
    /// (see module docs). `BTreeMap` for deterministic key ordering
    /// across serialisations.
    #[serde(default)]
    pub attributes: Option<BTreeMap<String, AttributeValue>>,
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

/// Allowed value types for `source.attributes`. Mirrors the server
/// Zod union `z.union([z.string(), z.number(), z.boolean()])`. The
/// `Num` variant uses `serde_json::Number` (not `f64`) so an integer
/// label round-trips as the integer it was sent — at runtime
/// `Number` preserves the int/float discriminant across
/// deserialise → serialise, where `f64` would coerce `14` to `14.0`.
///
/// The `#[schemars(with = "f64")]` annotation is JSON-Schema-side
/// metadata only — it tells schemars to advertise this variant as
/// JSON Schema `{"type": "number"}` (which accepts both `14` and
/// `14.0`). It does NOT cause runtime float coercion; the actual
/// deserialise target is still `serde_json::Number`. Schemars
/// doesn't ship a default `JsonSchema` impl for `Number` itself, so
/// the `with` annotation is how we get a sensible schema emitted
/// without defining a manual impl.
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum AttributeValue {
    Str(String),
    Num(#[schemars(with = "f64")] Number),
    Bool(bool),
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
    //    "nt-mcp"`. The agent cannot override `source` — the input
    //    schema doesn't expose it (pinned by the discovery test).
    //    Client-supplied `attributes` (the deliberate passthrough
    //    slot) lands on `source.attributes` via `build_source`.
    let envelope = build_envelope(args);

    // 3. POST /v1/events with Bearer auth. Single attempt — retry is
    //    deferred per the Task 19 scope note. A non-2xx after one
    //    try surfaces as a transport error; the agent / MCP client
    //    can retry the tool call itself if it wants.
    //
    //    URL composition (trailing-slash trim) and bearer auth live
    //    in nt-core. publish_event has no path-segment to encode
    //    (the endpoint is the fixed `/v1/events`).
    let url = api_url(&config.api_url, "/v1/events");
    let response = post_json(
        http_client,
        &url,
        &config.token,
        &serde_json::json!([envelope]),
    )
    .await
    .map_err(transport_to_mcp)?;

    if !response.is_success() {
        return Err(McpError::internal_error(
            format!("server returned {}: {}", response.status, response.body),
            None,
        ));
    }

    // 4. Parse server response and build the tool result. Mirrors the
    //    TS handler's `{ id, deduped }` shape. `deduped: true` only
    //    when the server reports the event went through dedupe rather
    //    than fresh ingestion.
    let parsed: Value = serde_json::from_str(&response.body).map_err(|e| {
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
    envelope.insert("source".to_string(), build_source(args));
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
/// tool args. `attributes` is the client passthrough slot (flat
/// scalar map): when supplied non-empty, it lands on
/// `source.attributes` verbatim. When omitted OR empty, the
/// `attributes` block is dropped from the wire so an empty `{}`
/// doesn't bloat every envelope.
fn build_source(args: &PublishEventArgs) -> Value {
    let mut src = Map::new();
    src.insert("name".to_string(), Value::String("nt-mcp".to_string()));
    src.insert(
        "sdkVersion".to_string(),
        Value::String(env!("CARGO_PKG_VERSION").to_string()),
    );
    if let Some(attrs) = &args.attributes {
        if !attrs.is_empty() {
            // Adversarial review #8: direct match per variant rather
            // than `serde_json::to_value(...).expect(...)` — the
            // variant set is closed (Str/Num/Bool), every arm
            // returns a `Value` infallibly, no panic path.
            let mut wire = Map::new();
            for (key, value) in attrs {
                let v = match value {
                    AttributeValue::Str(s) => Value::String(s.clone()),
                    AttributeValue::Num(n) => Value::Number(n.clone()),
                    AttributeValue::Bool(b) => Value::Bool(*b),
                };
                wire.insert(key.clone(), v);
            }
            src.insert("attributes".to_string(), Value::Object(wire));
        }
    }
    Value::Object(src)
}
