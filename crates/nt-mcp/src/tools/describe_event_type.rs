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

use rmcp::{model::*, ErrorData as McpError};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::config::EnvConfig;
use crate::example_synth::synthesise_example;

/// Exact TS-parity description from
/// `src/mcp/tools/describe-event-type.ts`. Pinned here as a constant so
/// the integration test asserts on byte-for-byte equality rather than
/// a substring match. The literal lives in the `#[tool]` attribute over
/// in `server.rs`; rmcp's macro requires a string literal there.
#[allow(dead_code)] // Test-only anchor; the literal lives in the #[tool] attribute.
pub const TS_PARITY_DESCRIPTION: &str = "Return schema, dedupe strategy, retention, and a synthesised example payload for a single event type. Call this before publish_event when you do not already know the schema; the example field is a starting point you can adapt.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DescribeEventTypeArgs {
    /// Event type id (domain.entity.action.vN).
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
    args: &DescribeEventTypeArgs,
    config: &EnvConfig,
    http_client: &reqwest::Client,
) -> Result<CallToolResult, McpError> {
    // 1. GET the detail endpoint. The id segment is dropped in
    //    verbatim — canonical type ids are domain.entity.action.vN
    //    with only `.` (URL-safe), matching the TS reference's use of
    //    `encodeURIComponent` which is a no-op for that grammar.
    let url = format!(
        "{}/v1/registry/event-types/{}",
        config.api_url.trim_end_matches('/'),
        args.id,
    );
    let response = http_client
        .get(&url)
        .bearer_auth(&config.token)
        .send()
        .await
        .map_err(|e| McpError::internal_error(format!("transport error: {e}"), None))?;

    // 2. Map status codes to typed errors. 404 → not-found (named
    //    after the id so the agent can correct the request). Other
    //    non-2xx → transport error with the upstream body for
    //    diagnostics.
    let status = response.status();
    if status.as_u16() == 404 {
        return Err(McpError::invalid_params(
            format!("event type \"{}\" not found", args.id),
            None,
        ));
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

    // 3. Parse `{ eventType: {...} }`. A 2xx without the envelope or
    //    without an embedded `schema` field is a server-contract
    //    violation — surface loudly rather than rendering an empty
    //    example, matching the TS handler's explicit guard.
    let parsed: Value = serde_json::from_str(&body_text).map_err(|e| {
        McpError::internal_error(format!("invalid server JSON response: {e}"), None)
    })?;
    let Some(spec) = parsed.get("eventType").and_then(Value::as_object) else {
        return Err(McpError::internal_error(
            format!(
                "server-contract violation: detail response for \"{}\" is missing the eventType wrapper",
                args.id,
            ),
            None,
        ));
    };
    let Some(schema) = spec.get("schema") else {
        return Err(McpError::internal_error(
            format!(
                "server-contract violation: detail response for \"{}\" is missing the schema field",
                args.id,
            ),
            None,
        ));
    };

    // 4. Build the result envelope. camelCase → snake_case rename on
    //    the optional fields, mirroring the TS handler's spread. Each
    //    optional field is OMITTED when absent (no JSON null on the
    //    wire) so an agent can distinguish "server didn't say" from
    //    "server said null".
    let mut result = Map::new();
    result.insert("id".to_string(), Value::String(args.id.clone()));
    result.insert("schema".to_string(), schema.clone());
    result.insert("example".to_string(), synthesise_example(schema));
    rename_into(spec, "dedupeStrategy", "dedupe_strategy", &mut result);
    rename_into(spec, "retentionDays", "retention_days", &mut result);
    rename_into(spec, "uiHints", "ui_hints", &mut result);
    rename_into(spec, "deprecatedAt", "deprecated_at", &mut result);

    let text = serde_json::to_string(&Value::Object(result))
        .expect("describe result is always serialisable");
    Ok(CallToolResult::success(vec![Content::text(text)]))
}

/// Copy `spec[wire_key]` into `result[snake_key]` when present.
/// Absence is silent — the field stays omitted from the result so the
/// downstream `omits_optional_fields_when_absent_from_spec` pin holds.
fn rename_into(
    spec: &Map<String, Value>,
    wire_key: &str,
    snake_key: &str,
    result: &mut Map<String, Value>,
) {
    if let Some(value) = spec.get(wire_key) {
        result.insert(snake_key.to_string(), value.clone());
    }
}
