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

use nt_core::encoding::encode_path_segment;
use nt_core::http::get_raw;
use nt_core::url::api_url;
use rmcp::{model::*, ErrorData as McpError};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::config::EnvConfig;
use crate::error_map::transport_to_mcp;
use crate::example_synth::synthesise_example;

// Note on TS parity: unlike `list_event_types` and `publish_event`,
// this module does NOT carry a `TS_PARITY_DESCRIPTION` constant.
// rmcp's `#[tool]` macro requires a string literal so the constant
// can't be referenced from `server.rs` anyway; the test in `mcp.rs`
// keeps its own byte-for-byte copy. A module-side constant would be
// a third unverified copy of the same string. Drift between the
// `#[tool]` attribute and the TS reference is caught by the
// integration test (adversarial review #6).

/// camelCase → snake_case rename table for the optional fields on the
/// describe result envelope. Sourced verbatim from the TS handler's
/// spread (`...(spec.dedupeStrategy !== undefined && { dedupe_strategy:
/// ... })`). Drift in either column would surface in
/// `describe_event_type_passes_through_optional_fields_when_present`.
const OPTIONAL_FIELD_RENAMES: &[(&str, &str)] = &[
    ("dedupeStrategy", "dedupe_strategy"),
    ("retentionDays", "retention_days"),
    ("uiHints", "ui_hints"),
    ("deprecatedAt", "deprecated_at"),
];

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
    // 1. GET the detail endpoint via the nt-core primitive. URL
    //    composition (trailing-slash trim) and path-segment encoding
    //    (RFC 3986 pchar-minus-percent) live in nt-core; the
    //    canonical `domain.entity.action.vN` ids pass through
    //    unchanged, pathological inputs get percent-escaped before
    //    they hit the URL.
    let url = api_url(
        &config.api_url,
        &format!("/v1/registry/event-types/{}", encode_path_segment(&args.id)),
    );
    let response = get_raw(http_client, &url, &config.token)
        .await
        .map_err(transport_to_mcp)?;

    // 2. Map status codes to typed errors. 404 → named not-found
    //    (the agent can correct the id). 401/403 → auth-specific so
    //    the MCP client knows to re-resolve NO_TICKETS_TOKEN.
    //    Other non-2xx → transport error with the upstream body.
    //    No retry — matches the PRD's async-non-blocking refresh
    //    framing. Mapping stays inline because the wording is
    //    tool-specific ("event type X not found" vs "interaction X
    //    not found" in the broader nt-mcp surface).
    match response.status {
        404 => {
            return Err(McpError::invalid_params(
                format!("event type \"{}\" not found", args.id),
                None,
            ));
        }
        401 | 403 => {
            // Auth failures are not param errors — map to
            // `internal_error` (-32603) so JSON-RPC codes distinguish
            // auth/infra failures from bad-input (404).
            return Err(McpError::internal_error(
                format!(
                    "authentication failed ({}) — check NO_TICKETS_TOKEN; the server rejected the bearer credential",
                    response.status,
                ),
                None,
            ));
        }
        s if !(200..300).contains(&s) => {
            return Err(McpError::internal_error(
                format!("server returned {}: {}", response.status, response.body),
                None,
            ));
        }
        _ => {}
    }

    // 3. Parse `{ eventType: {...} }`. A 2xx without the envelope,
    //    with `eventType: null`, or without an embedded `schema`
    //    field is a server-contract violation — surface loudly rather
    //    than rendering an empty example, matching the TS handler's
    //    explicit guard.
    let parsed: Value = serde_json::from_str(&response.body).map_err(|e| {
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

    // 4. Build the result envelope. Pull `id` from the server-echoed
    //    spec (matches the TS handler's `id: spec.id`); a missing
    //    `id` is itself a server-contract violation, surfaced before
    //    the camelCase rename loop. Optional fields rename from
    //    camelCase to snake_case and are OMITTED when absent (no JSON
    //    null on the wire) so an agent can distinguish "server didn't
    //    say" from "server said null".
    let Some(spec_id) = spec.get("id").and_then(Value::as_str) else {
        return Err(McpError::internal_error(
            format!(
                "server-contract violation: detail response for \"{}\" is missing the eventType.id field",
                args.id,
            ),
            None,
        ));
    };
    let mut result = Map::new();
    result.insert("id".to_string(), Value::String(spec_id.to_string()));
    result.insert("schema".to_string(), schema.clone());
    result.insert("example".to_string(), synthesise_example(schema));
    for (wire_key, snake_key) in OPTIONAL_FIELD_RENAMES {
        if let Some(value) = spec.get(*wire_key) {
            result.insert((*snake_key).to_string(), value.clone());
        }
    }

    let text = serde_json::to_string(&Value::Object(result))
        .expect("describe result is always serialisable");
    Ok(CallToolResult::success(vec![Content::text(text)]))
}
