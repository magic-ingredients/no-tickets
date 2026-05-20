//! Task 20: `describe_event_type` tool. Discovery, happy path with
//! schema/example synthesis, header propagation, failure modes
//! (404 / 5xx / 401 / 403 / missing-schema / pre-HTTP env guards),
//! optional-field passthrough rename, result.id-comes-from-spec
//! invariant, malformed server responses, URL path-segment encoding,
//! and stdout-purity under the describe code path.

use serde_json::{json, Value};
use wiremock::matchers::{header, method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::{collect_error_text, extract_tool_result_payload, McpClient};

/// Byte-for-byte expected description for the `describe_event_type`
/// MCP tool. Pinned so a drift in the production description fails
/// this test loudly.
const DESCRIBE_EVENT_TYPE_DESCRIPTION: &str = "Return schema, dedupe strategy, retention, and a synthesised example payload for a single event type. Call this before publish_event when you do not already know the schema; the example field is a starting point you can adapt.";

/// Canonical event-type detail body — the wire shape the server
/// returns for `GET /v1/registry/event-types/{id}`. Matches the TS
/// `detailResponseSchema` in `src/registry/client.ts`: a top-level
/// `eventType` envelope wrapping the spec. Only the fields the test
/// is asserting on are populated; the per-test helpers below extend
/// with optional fields (`dedupeStrategy`, `retentionDays`, etc).
fn detail_body_minimal(id: &str) -> Value {
    json!({
        "eventType": {
            "id": id,
            "domain": "ai",
            "entity": "task",
            "action": "completed",
            "version": "v1",
            "schema": {
                "type": "object",
                "properties": {
                    "taskId": { "type": "string" },
                    "outcome": { "type": "string", "enum": ["success", "failure"] }
                },
                "required": ["taskId", "outcome"]
            }
        }
    })
}

// ─── Discovery: tool is registered with the right shape ──────────────────

#[tokio::test]
async fn tools_list_includes_describe_event_type_with_ts_parity_description() {
    let mut c = McpClient::spawn().await;
    c.handshake().await;
    let resp = c.request("tools/list", json!({})).await;
    let tools = resp["result"]["tools"].as_array().expect("tools array");
    let entry = tools
        .iter()
        .find(|t| t["name"] == "describe_event_type")
        .expect("describe_event_type tool registered");
    assert_eq!(
        entry["description"].as_str(),
        Some(DESCRIBE_EVENT_TYPE_DESCRIPTION),
        "describe_event_type description must match the pinned literal",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn describe_event_type_input_schema_requires_id() {
    let mut c = McpClient::spawn().await;
    c.handshake().await;
    let resp = c.request("tools/list", json!({})).await;
    let entry = resp["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .find(|t| t["name"] == "describe_event_type")
        .expect("describe_event_type tool registered")
        .clone();
    let schema = &entry["inputSchema"];
    let props = &schema["properties"];
    assert!(
        props["id"].is_object(),
        "schema must declare an `id` property; got props={props}",
    );
    let required = schema["required"]
        .as_array()
        .expect("required array")
        .iter()
        .filter_map(|v| v.as_str())
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert!(
        required.iter().any(|r| r == "id"),
        "schema must require `id`; required={required:?}",
    );
    c.shutdown().await;
}

// ─── Behavior: happy path GETs the detail endpoint ───────────────────────

#[tokio::test]
async fn describe_event_type_happy_path_returns_id_schema_and_example() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(detail_body_minimal("ai.task.completed.v1")),
        )
        .expect(1)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    assert_eq!(
        payload["id"], "ai.task.completed.v1",
        "result.id must echo the requested type id; got {payload}",
    );
    assert!(
        payload["schema"]["properties"].is_object(),
        "result.schema must be the full JSON Schema; got {payload}",
    );
    // The example field comes from synthesise_example on the schema.
    // For the minimal body, taskId is a string (→ "") and outcome is
    // an enum-of-strings (→ "success", the first value).
    let example = &payload["example"];
    assert_eq!(
        example["taskId"], "",
        "example.taskId must be the type-placeholder for string; got example={example}",
    );
    assert_eq!(
        example["outcome"], "success",
        "example.outcome must be the first enum value; got example={example}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn describe_event_type_sends_bearer_token_on_get() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .and(header("authorization", "Bearer nt_test_token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(detail_body_minimal("ai.task.completed.v1")),
        )
        .expect(1)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test_token"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    assert_eq!(
        payload["id"], "ai.task.completed.v1",
        "happy path must complete when bearer matches; got {resp}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn describe_event_type_trailing_slash_api_url_routes_to_endpoint_unchanged() {
    // `NO_TICKETS_API_URL` with a trailing slash MUST land on
    // `<api_url>/v1/registry/event-types/{id}` (single `/`, not double).
    // Mirrors the publish_event trailing-slash pin.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(detail_body_minimal("ai.task.completed.v1")),
        )
        .expect(1)
        .mount(&server)
        .await;
    let uri_with_slash = format!("{}/", server.uri());
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri_with_slash.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    assert_eq!(
        payload["id"], "ai.task.completed.v1",
        "trailing-slash URL must still GET the detail endpoint",
    );
    c.shutdown().await;
}

// ─── Behavior: failure modes ─────────────────────────────────────────────

#[tokio::test]
async fn describe_event_type_404_surfaces_not_found_error_naming_the_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ghost.entity.action.v1"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ghost.entity.action.v1" }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.contains("ghost.entity.action.v1"),
        "404 error must name the missing id verbatim; got {msg:?}",
    );
    // Tightened (adversarial review #9): the impl only emits the
    // literal "not found"; an "unknown" fallback was unreachable.
    assert!(
        msg.to_lowercase().contains("not found"),
        "404 error must read as not-found; got {msg:?}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn describe_event_type_5xx_response_surfaces_transport_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream registry overloaded"))
        .expect(1) // single attempt — no retry per PRD
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    // Tightened (adversarial review #2): both the upstream status code
    // AND the upstream body must surface in the error text so the
    // agent has something concrete to act on. The previous OR-clause
    // accepted almost any error message containing the word "server".
    assert!(
        msg.contains("503"),
        "5xx error must include the upstream status code; got {msg:?}",
    );
    assert!(
        msg.contains("upstream registry overloaded"),
        "5xx error must include the upstream body for diagnostics; got {msg:?}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn describe_event_type_401_surfaces_auth_error() {
    // Adversarial review #3: 401 / 403 used to collapse into the
    // generic 5xx transport-error branch, giving the agent no signal
    // that the credential needs replacing. Pin the auth-specific
    // diagnostic so a regression can't bury it.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.to_lowercase().contains("auth")
            || msg.to_lowercase().contains("token")
            || msg.to_lowercase().contains("credential"),
        "401 must surface as an auth-specific diagnostic; got {msg:?}",
    );
    assert!(
        msg.contains("NO_TICKETS_TOKEN"),
        "401 diagnostic must name the env var the MCP client should refresh; got {msg:?}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn describe_event_type_403_surfaces_auth_error() {
    // Same auth-error treatment as 401 — neither flavour should
    // collapse into the generic transport-error branch.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .respond_with(ResponseTemplate::new(403))
        .expect(1)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.to_lowercase().contains("auth"),
        "403 must surface as an auth-specific diagnostic; got {msg:?}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn describe_event_type_missing_schema_field_surfaces_contract_violation() {
    // The detail endpoint MUST include `schema`; the list endpoint
    // omits it but describe always hits detail. Absence is a server-
    // contract violation worth surfacing loudly rather than rendering
    // an empty example. Mirrors the TS handler's explicit guard.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "eventType": {
                "id": "ai.task.completed.v1",
                "domain": "ai",
                "entity": "task",
                "action": "completed",
                "version": "v1"
                // schema deliberately absent
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.to_lowercase().contains("schema"),
        "missing-schema error must mention the field; got {msg:?}",
    );
    assert!(
        msg.to_lowercase().contains("contract") || msg.to_lowercase().contains("missing"),
        "missing-schema error must call out the server-contract issue; got {msg:?}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn describe_event_type_missing_token_surfaces_auth_error_before_http() {
    // Symmetric to publish_event's pre-HTTP env guard. Without
    // NO_TICKETS_TOKEN we must short-circuit before issuing the GET
    // — otherwise the mock would record the request.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        // NO_TICKETS_TOKEN deliberately absent
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.contains("NO_TICKETS_TOKEN"),
        "missing-token error must name the env var; got {msg:?}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn describe_event_type_missing_api_url_surfaces_config_error_before_http() {
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        // NO_TICKETS_API_URL deliberately absent
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.contains("NO_TICKETS_API_URL"),
        "missing-api-url error must name the env var; got {msg:?}",
    );
    c.shutdown().await;
}

// ─── Optional-field passthrough on the result envelope ───────────────────

#[tokio::test]
async fn describe_event_type_passes_through_optional_fields_when_present() {
    // The TS handler camelCase → snake_case maps the optional fields:
    //   dedupeStrategy  → dedupe_strategy
    //   retentionDays   → retention_days
    //   uiHints         → ui_hints
    //   deprecatedAt    → deprecated_at
    // Pin the renames + presence end-to-end so a regression that drops
    // one of them silently doesn't pass.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "eventType": {
                "id": "ai.task.completed.v1",
                "domain": "ai",
                "entity": "task",
                "action": "completed",
                "version": "v1",
                "schema": {
                    "type": "object",
                    "properties": { "taskId": { "type": "string" } }
                },
                "dedupeStrategy": "by-dedupe-key",
                "retentionDays": 30,
                "uiHints": { "color": "blue" },
                "deprecatedAt": "2026-01-01T00:00:00.000Z"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    assert_eq!(
        payload["dedupe_strategy"], "by-dedupe-key",
        "dedupeStrategy must rename to dedupe_strategy; got {payload}",
    );
    assert_eq!(
        payload["retention_days"], 30,
        "retentionDays must rename to retention_days; got {payload}",
    );
    assert_eq!(
        payload["ui_hints"]["color"], "blue",
        "uiHints must rename to ui_hints; got {payload}",
    );
    assert_eq!(
        payload["deprecated_at"], "2026-01-01T00:00:00.000Z",
        "deprecatedAt must rename to deprecated_at; got {payload}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn describe_event_type_omits_optional_fields_when_absent_from_spec() {
    // Symmetric to passthrough — when the server omits the optional
    // fields, the tool result MUST also omit them (not emit JSON null
    // / empty string / 0). Mirrors the TS handler's spread pattern.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(detail_body_minimal("ai.task.completed.v1")),
        )
        .expect(1)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    for absent in [
        "dedupe_strategy",
        "retention_days",
        "ui_hints",
        "deprecated_at",
    ] {
        assert!(
            payload.get(absent).is_none(),
            "{absent} must be omitted when the spec doesn't carry it; got payload={payload}",
        );
    }
    // Adversarial review #5: a regression that spread the full spec
    // into the result would leak server-internal field names (domain,
    // entity, action, version, etc.) and silently pass the per-key
    // assertions above. Pin the exact key set so unintended leakage
    // fails the test.
    let actual_keys: std::collections::BTreeSet<&str> = payload
        .as_object()
        .expect("payload is an object")
        .keys()
        .map(String::as_str)
        .collect();
    let expected_keys: std::collections::BTreeSet<&str> =
        ["id", "schema", "example"].into_iter().collect();
    assert_eq!(
        actual_keys, expected_keys,
        "result envelope must carry only id/schema/example when optional fields absent; got {payload}",
    );
    c.shutdown().await;
}

// ─── Result.id is server-echoed, not args-echoed ─────────────────────────

#[tokio::test]
async fn describe_event_type_result_id_comes_from_spec_not_args() {
    // Adversarial review #1: the TS handler uses `id: spec.id`; pin
    // that the Rust port mirrors this rather than echoing args.id.
    // A regression that did `result.id = args.id` would happily pass
    // every other test (in normal traffic args and spec agree). Set
    // them divergent here and assert spec wins.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "eventType": {
                // Server canonicalised the id (e.g. trimmed, normalised).
                "id": "ai.task.completed.v2",
                "domain": "ai",
                "entity": "task",
                "action": "completed",
                "version": "v2",
                "schema": { "type": "object", "properties": {} }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    assert_eq!(
        payload["id"], "ai.task.completed.v2",
        "result.id must echo spec.id (server-canonicalised), not args.id; got {payload}",
    );
    c.shutdown().await;
}

// ─── Malformed server response handling ──────────────────────────────────

#[tokio::test]
async fn describe_event_type_non_json_body_surfaces_parse_error() {
    // Adversarial review #7: the impl distinguishes "body isn't
    // JSON" from "JSON missing eventType wrapper" — pin the former
    // so a regression that swallowed the parse error and crashed in
    // .get("eventType") on a string Value doesn't pass.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not-json-at-all"))
        .expect(1)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.to_lowercase().contains("invalid") && msg.to_lowercase().contains("json"),
        "non-JSON body must surface as a JSON parse error; got {msg:?}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn describe_event_type_null_event_type_wrapper_surfaces_contract_violation() {
    // Adversarial review #7: `{ "eventType": null }` is JSON-valid
    // but contract-invalid — the unwrap must fail, not panic / not
    // proceed with a null spec. Pin alongside the missing-wrapper
    // case so both `parsed.get("eventType")` failure modes are
    // covered.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "eventType": null })))
        .expect(1)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.to_lowercase().contains("eventtype")
            && (msg.to_lowercase().contains("missing") || msg.to_lowercase().contains("contract")),
        "eventType=null must surface as a server-contract violation; got {msg:?}",
    );
    c.shutdown().await;
}

// ─── URL path-segment encoding ───────────────────────────────────────────

#[tokio::test]
async fn describe_event_type_canonical_id_passes_through_url_unchanged() {
    // Adversarial review #4: pin the canonical-grammar assumption.
    // The percent-encoding configuration must leave
    // `domain.entity.action.vN` ids untouched on the wire so the
    // common path doesn't degrade. The mock matcher uses an exact
    // unencoded path; a regression that encoded `.` (e.g.
    // `%2E`) would fail this matcher.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(detail_body_minimal("ai.task.completed.v1")),
        )
        .expect(1)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    assert_eq!(payload["id"], "ai.task.completed.v1");
    c.shutdown().await;
}

#[tokio::test]
async fn describe_event_type_unsafe_chars_in_id_are_percent_encoded() {
    // Adversarial review #4: pathological characters (`/`, `?`, `#`)
    // in the id MUST be percent-encoded before they hit the URL, or
    // they'd break the URL structure / smuggle a path traversal.
    // `weird/id?with#chars` → segments encoded to
    // `weird%2Fid%3Fwith%23chars`.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path(
            "/v1/registry/event-types/weird%2Fid%3Fwith%23chars",
        ))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "weird/id?with#chars" }
            }),
        )
        .await;
    // We don't care about the 404 outcome — only that the mock
    // matched, which proves the URL was built with the encoded
    // segment. `.expect(1)` above is the actual assertion; the
    // `let _ = resp` keeps the borrow tidy.
    let _ = resp;
    c.shutdown().await;
}

// ─── Stdout-purity coverage for describe_event_type ──────────────────────

#[tokio::test]
async fn describe_event_type_call_does_not_corrupt_stdout_jsonrpc_stream() {
    // Same invariant as the publish_event / list_event_types stdout-
    // purity tests. The describe path has its own code (HTTP GET, JSON
    // unwrap, synthesise_example); a stray `println!` anywhere in
    // that path would silently disconnect Claude Code.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v1/registry/event-types/ai.task.completed.v1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(detail_body_minimal("ai.task.completed.v1")),
        )
        .expect(1)
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    let _ = c
        .request(
            "tools/call",
            json!({
                "name": "describe_event_type",
                "arguments": { "id": "ai.task.completed.v1" }
            }),
        )
        .await;
    let (captured, _stderr) = c.shutdown().await;
    for line in &captured {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        serde_json::from_str::<Value>(trimmed)
            .unwrap_or_else(|e| panic!("stdout polluted by non-JSON line {trimmed:?}: {e}"));
    }
}
