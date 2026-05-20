//! Task 19: `publish_event` tool. Discovery, happy path, failure modes
//! (handler short-circuits before HTTP), server-failure mappings,
//! wire-shape source/attributes pins, empty-env-var handling, malformed
//! server responses, dedupe-detection truth table, URL normalisation +
//! identity-spoof protection, and the stdout-purity invariant under
//! the publish_event code path.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use wiremock::matchers::{header, method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::{collect_error_text, extract_tool_result_payload, McpClient};

/// Byte-for-byte TS-parity description from `src/mcp/tools/publish-event.ts`.
/// Pinned here as a string literal (not a path import — the binary's
/// production constant lives in `crates/nt-mcp/src/tools/publish_event.rs`
/// but the binary isn't a library, so the test can't see it). Drift
/// here is the same kind of drift the list_event_types parity test
/// catches.
const PUBLISH_EVENT_TS_PARITY_DESCRIPTION: &str = "Publish a single event. Call describe_event_type first to confirm the schema; the server will reject mismatches. Source metadata is filled server-side and cannot be overridden.";

/// Valid `ai.task.completed.v1` data payload — matches the
/// `crates/nt-cli/tests/publish.rs::VALID_AI_TASK_DATA` shape and is
/// the canonical happy-path event payload used across the binary
/// tests. Includes all server-required fields so schema validation
/// passes.
fn valid_ai_task_data() -> Value {
    json!({
        "taskId": "task-1",
        "sessionId": "sess-1",
        "startedAt": "2026-05-01T00:00:00.000Z",
        "completedAt": "2026-05-01T00:00:01.000Z",
        "durationMs": 1000,
        "outcome": "success",
        "callCount": 1,
    })
}

/// Wire-body capturer for the publish_event tests. Mounts a wiremock
/// route on `POST /v1/events` that responds 200 with a canned body
/// and stores the request body for later inspection. The returned
/// Arc<Mutex<Option<String>>> holds the raw body bytes as a UTF-8
/// String — tests parse it into a Value to assert on shape.
async fn capture_publish_body(server: &MockServer) -> Arc<Mutex<Option<String>>> {
    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let cap_for_responder = captured.clone();
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .respond_with(move |req: &wiremock::Request| {
            let body = String::from_utf8(req.body.clone()).expect("request body utf8");
            *cap_for_responder.lock().unwrap() = Some(body);
            ResponseTemplate::new(200).set_body_json(json!({
                "ingested": 1, "deduped": 0, "ids": ["evt_captured"],
            }))
        })
        .expect(1)
        .mount(server)
        .await;
    captured
}

// ─── Discovery: tool is registered with the right shape ──────────────────

#[tokio::test]
async fn tools_list_includes_publish_event_with_ts_parity_description() {
    let mut c = McpClient::spawn().await;
    c.handshake().await;
    let resp = c.request("tools/list", json!({})).await;
    let tools = resp["result"]["tools"].as_array().expect("tools array");
    let entry = tools
        .iter()
        .find(|t| t["name"] == "publish_event")
        .expect("publish_event tool registered");
    assert_eq!(
        entry["description"].as_str(),
        Some(PUBLISH_EVENT_TS_PARITY_DESCRIPTION),
        "publish_event description must byte-match TS reference",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn publish_event_input_schema_declares_required_and_optional_fields() {
    let mut c = McpClient::spawn().await;
    c.handshake().await;
    let resp = c.request("tools/list", json!({})).await;
    let entry = resp["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .find(|t| t["name"] == "publish_event")
        .expect("publish_event tool registered")
        .clone();

    let schema = &entry["inputSchema"];
    let props = &schema["properties"];

    // Required: type, data — every event needs these for a wire-valid
    // envelope. Project tenancy is server-resolved from the push
    // token (see notickets-service/.../routes/events.ts); there is no
    // `project` arg on the tool, and a regression that re-adds one
    // would fail the "must NOT declare" assertion below.
    let required = schema["required"]
        .as_array()
        .expect("required array")
        .iter()
        .filter_map(|v| v.as_str())
        .map(str::to_string)
        .collect::<Vec<_>>();
    for needed in ["type", "data"] {
        assert!(
            required.iter().any(|r| r == needed),
            "schema must require `{needed}`; required={required:?}",
        );
    }

    // Optional fields surfaced via JSON Schema properties even though
    // they're not in `required`. A missing entry means the
    // schema-derive macro lost a field — caught here.
    for optional in ["occurred_at", "parent_event_id", "trace_id", "dedupe_key"] {
        assert!(
            props[optional].is_object(),
            "schema must declare optional property `{optional}`; got props={props}",
        );
    }

    // Source identity is fixed server-side — agents must NOT be able
    // to override `source` via tool args. A regression that adds a
    // `source` property to the schema would let an agent spoof its
    // identity.
    assert!(
        props.get("source").is_none(),
        "schema must NOT declare a `source` property — server fills it; got props={props}",
    );

    // `project` was removed 2026-05-15: project tenancy is server-
    // resolved from the push token, so a client-supplied label
    // would be advisory-only dead weight. A regression that re-
    // introduces the arg lands here.
    assert!(
        props.get("project").is_none(),
        "schema must NOT declare a `project` property — tenancy is token-derived; got props={props}",
    );

    // `subject` was removed in v0.1.1: subjects are not modelled
    // server-side, so the tool arg was misleading users into thinking
    // their input would be tracked. The wire envelope retains a
    // `subject` slot as forward-compat but neither client populates it.
    assert!(
        props.get("subject").is_none(),
        "schema must NOT declare a `subject` property — subjects not yet modelled; got props={props}",
    );

    c.shutdown().await;
}

// ─── Behavior: happy path posts to /v1/events ────────────────────────────

#[tokio::test]
async fn publish_event_happy_path_posts_and_returns_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .and(header("authorization", "Bearer nt_test_token"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ingested": 1, "deduped": 0, "ids": ["evt_happy"],
        })))
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    assert_eq!(
        payload["id"], "evt_happy",
        "tool result must surface the server-returned event id"
    );
    assert_eq!(
        payload["deduped"], false,
        "deduped=false when server reports ingested=1, deduped=0",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn publish_event_marks_deduped_when_server_reports_only_deduped() {
    // Pin the dedupe-detection branch: when server returns
    // `ingested: 0, deduped: 1`, the tool reports `deduped: true`.
    // Matches the TS handler's `ingested === 0 && deduped > 0`.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ingested": 0, "deduped": 1, "ids": ["evt_dup"],
        })))
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    assert_eq!(
        payload["deduped"], true,
        "deduped=true when ingested=0 + deduped>0"
    );
    c.shutdown().await;
}

// ─── Behavior: failure modes (handler short-circuits before HTTP) ────────

#[tokio::test]
async fn publish_event_missing_token_surfaces_auth_error_before_http() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // no token → must short-circuit before any HTTP
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
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
async fn publish_event_missing_api_url_surfaces_config_error_before_http() {
    // Symmetric to missing-token: NO_TICKETS_API_URL absent must
    // produce a config error rather than a hung HTTP call against
    // the default production URL (which a test machine probably
    // can't reach).
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
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

#[tokio::test]
async fn publish_event_unknown_event_type_short_circuits_before_http() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // unknown type → local validate fails → no HTTP
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
                "name": "publish_event",
                "arguments": {
                    "type": "not.a.real.type.v999",
                    "data": {},
                }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.to_lowercase().contains("unknown") || msg.contains("not.a.real.type.v999"),
        "unknown-type error must name the offending type or describe it as unknown; got {msg:?}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn publish_event_schema_validation_failure_short_circuits_before_http() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // schema fail → local validate → no HTTP
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    // `taskId` should be a string; pass a number to force a schema fail.
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": { "taskId": 42 },
                }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.to_lowercase().contains("validation") || msg.to_lowercase().contains("schema"),
        "schema-fail error must say so; got {msg:?}",
    );
    c.shutdown().await;
}

// ─── Behavior: server-side failure responses surface as MCP errors ───────

#[tokio::test]
async fn publish_event_5xx_response_surfaces_transport_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1) // single attempt — no retry in this slice
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.contains("503") || msg.to_lowercase().contains("server"),
        "5xx must surface as a transport/server error; got {msg:?}",
    );
    c.shutdown().await;
}

// ─── Wire shape: source identity + project attribute ─────────────────────

#[tokio::test]
async fn publish_event_wire_body_has_source_name_nt_mcp() {
    // Source identity is server-side per the TS reference. Agents
    // cannot override `source` via tool args (schema test above pins
    // the absence of a `source` property), AND the server fills
    // source.name with the fixed identity `"nt-mcp"`. A regression
    // that copied the CLI's default (`"no-tickets"`) would land here.
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let body = captured.lock().unwrap().clone().expect("body captured");
    let parsed: Value = serde_json::from_str(&body).expect("body is JSON");
    let envelope = &parsed[0];
    assert_eq!(
        envelope["source"]["name"], "nt-mcp",
        "MCP-side source.name must be `nt-mcp`; got body={body}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn publish_event_wire_body_omits_source_attributes_block_when_absent() {
    // Default-omit invariant: when the agent doesn't pass an
    // `attributes` arg, the source block carries only name +
    // sdkVersion. Pin the absence so a regression that always
    // emits an empty `attributes: {}` (or hardcodes some "default"
    // tag like the old advisory `project` label, removed 1ea230d)
    // fails this check. See the sibling
    // `…_carries_attributes_when_supplied` test for the present-
    // when-supplied path.
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let body = captured.lock().unwrap().clone().expect("body captured");
    let envelope: Value = serde_json::from_str(&body).expect("body JSON");
    let src = &envelope[0]["source"];
    assert!(
        src.get("attributes").is_none(),
        "source.attributes must be omitted from the wire envelope; got src={src}",
    );
    // Source still carries name + sdkVersion — only the attributes
    // block is gone.
    assert_eq!(src["name"], "nt-mcp", "source.name preserved");
    assert!(
        src["sdkVersion"].is_string(),
        "source.sdkVersion preserved; got src={src}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn publish_event_wire_body_carries_attributes_when_supplied() {
    // Re-add of the attributes slot 2026-05-15. The slot is the
    // deliberate client-passthrough surface: flat scalar labels land
    // on `source.attributes` verbatim, server stores in JSONB and
    // surfaces in UI. Pin that supplied attributes round-trip with
    // their value-types intact (string, integer, boolean).
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                    "attributes": {
                        "ticketUrl": "https://linear.app/foo/bar",
                        "submittedBy": "ada",
                        "trialDays": 14,
                        "costUSD": 1.99,
                        "isCanary": true
                    }
                }
            }),
        )
        .await;
    let body = captured.lock().unwrap().clone().expect("body captured");
    let envelope: Value = serde_json::from_str(&body).expect("body JSON");
    let attrs = &envelope[0]["source"]["attributes"];
    assert_eq!(
        attrs["ticketUrl"], "https://linear.app/foo/bar",
        "string attribute must round-trip; got attrs={attrs}",
    );
    assert_eq!(
        attrs["submittedBy"], "ada",
        "string attribute must round-trip; got attrs={attrs}",
    );
    // Integer round-trip — `serde_json::Number` preserves int vs
    // float distinction across deserialize→serialize. A regression
    // that coerced through f64 would surface `14.0` here.
    assert_eq!(
        attrs["trialDays"], 14,
        "integer attribute must round-trip as integer (not 14.0); got attrs={attrs}",
    );
    // Adversarial review #7: float round-trip — non-integer JSON
    // numbers must survive as floats with their precision intact.
    // Pinning a representative decimal (1.99) ensures the Num
    // variant handles both integral and fractional values, not just
    // the integer path the prior test covered.
    let cost = attrs["costUSD"].as_f64().expect("costUSD is a number");
    assert!(
        (cost - 1.99).abs() < f64::EPSILON,
        "float attribute must round-trip with precision; got costUSD={cost}",
    );
    assert_eq!(
        attrs["isCanary"], true,
        "boolean attribute must round-trip; got attrs={attrs}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn publish_event_wire_body_omits_attributes_when_supplied_empty() {
    // An empty `attributes: {}` is treated identically to "not
    // supplied" — no `attributes` block on the wire. Avoids bloating
    // every envelope with an empty object and means a regression
    // that always emits an empty block fails this assertion.
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                    "attributes": {}
                }
            }),
        )
        .await;
    let body = captured.lock().unwrap().clone().expect("body captured");
    let envelope: Value = serde_json::from_str(&body).expect("body JSON");
    let src = &envelope[0]["source"];
    assert!(
        src.get("attributes").is_none(),
        "empty attributes map must drop the block from the wire; got src={src}",
    );
    // Adversarial review #5: pin that the rest of the source block
    // is intact. A regression that destroyed the whole source object
    // when attributes was empty would also pass the absence check
    // above — sibling assertions stop that.
    assert_eq!(
        src["name"], "nt-mcp",
        "source.name must survive empty-attributes; got src={src}",
    );
    assert!(
        src["sdkVersion"].is_string(),
        "source.sdkVersion must survive empty-attributes; got src={src}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn publish_event_rejects_nested_object_as_attribute_value() {
    // Scalar-only is enforced at the deserialisation layer (the
    // `AttributeValue` enum has only Str / Num / Bool variants). An
    // agent that passes a nested object as a value fails to
    // deserialise into `PublishEventArgs`, surfaces as a structured
    // MCP error, and never reaches the wire. Pin the constraint
    // here so a regression that widens the union (e.g. adds a
    // `Map(Map)` variant) fails this check.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // deserialise failure → never reaches HTTP
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                    "attributes": { "nested": { "no": "good" } }
                }
            }),
        )
        .await;
    // Either the MCP framework rejects at the args-deserialise
    // layer (preferred) or the handler surfaces a structured error.
    // Both manifest as a non-success response.
    let has_error = !resp["error"].is_null();
    let has_is_error_true = resp["result"]["isError"] == json!(true);
    assert!(
        has_error || has_is_error_true,
        "nested attribute value must be rejected (deserialise layer or handler); got {resp}",
    );
    // Adversarial review #2: pin which layer rejected. An unrelated
    // error (e.g. token resolution failing) shouldn't satisfy this
    // test — the error text must name the offending field or the
    // AttributeValue type so the agent knows it was the attributes
    // arg that broke.
    let msg = collect_error_text(&resp);
    let m = msg.to_lowercase();
    assert!(
        m.contains("attributes") || m.contains("attributevalue"),
        "rejection must name the attributes arg / AttributeValue type so the agent can locate the bad input; got {msg:?}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn publish_event_input_schema_declares_attributes_as_optional_scalar_map() {
    // Tool-discovery pin: the schemars-derived JSON Schema for the
    // `attributes` arg must declare it as optional, of object shape,
    // with `additionalProperties` accepting only the string / number
    // / boolean union. Agents read this to know what shape to send.
    let mut c = McpClient::spawn().await;
    c.handshake().await;
    let resp = c.request("tools/list", json!({})).await;
    let entry = resp["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .find(|t| t["name"] == "publish_event")
        .expect("publish_event tool registered")
        .clone();
    let schema = &entry["inputSchema"];
    let props = &schema["properties"];

    // Optional — not in required.
    let required = schema["required"]
        .as_array()
        .expect("required array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>();
    assert!(
        !required.contains(&"attributes"),
        "attributes must be optional; required={required:?}",
    );

    // Object-typed at the property level. `additionalProperties`
    // points at a `$defs/AttributeValue` ref (schemars hoists enums
    // into `$defs`); the contract that matters is "value type is the
    // scalar union" — look up the def to verify.
    let attrs_schema = &props["attributes"];
    assert!(
        attrs_schema.is_object(),
        "attributes property must be declared; got props={props}",
    );
    let ref_path = attrs_schema["additionalProperties"]["$ref"]
        .as_str()
        .expect("additionalProperties must use a $ref to the AttributeValue def");
    assert!(
        ref_path.ends_with("AttributeValue"),
        "additionalProperties ref must target AttributeValue; got {ref_path}",
    );
    // Resolve the def from the schema's $defs. Adversarial review
    // #3: parse the union variants directly rather than substring-
    // matching, so a regression that renames a variant or smuggles
    // an "object" inside a description string can't pass.
    let attr_value_def = &schema["$defs"]["AttributeValue"];
    let variants = attr_value_def["oneOf"]
        .as_array()
        .or_else(|| attr_value_def["anyOf"].as_array())
        .unwrap_or_else(|| {
            panic!("AttributeValue def must be a oneOf/anyOf union; got {attr_value_def}")
        });
    let variant_types: std::collections::BTreeSet<String> = variants
        .iter()
        .filter_map(|v| v["type"].as_str().map(str::to_string))
        .collect();
    let expected: std::collections::BTreeSet<String> = ["boolean", "number", "string"]
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(
        variant_types, expected,
        "AttributeValue must be exactly the scalar union (string|number|boolean); got {attr_value_def}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn publish_event_wire_body_omits_optional_fields_when_args_absent() {
    // Regression pin: optional envelope-level fields (subject,
    // parentEventId, traceId, dedupeKey, occurredAt) MUST NOT appear
    // on the wire when not supplied. Matches the single-event publish
    // semantics from Task 15.
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let body = captured.lock().unwrap().clone().expect("body captured");
    let envelope: Value = serde_json::from_str(&body).expect("body JSON");
    let env = &envelope[0];
    for key in [
        "subject",
        "parentEventId",
        "traceId",
        "dedupeKey",
        "occurredAt",
    ] {
        assert!(
            env.get(key).is_none(),
            "{key} must be absent when arg not supplied; got envelope={env}",
        );
    }
    c.shutdown().await;
}

// ─── Empty-string env vars are treated identically to missing ────────────

#[tokio::test]
async fn publish_event_empty_token_treated_as_missing_var() {
    // An empty NO_TICKETS_TOKEN is never valid — Bearer "" wouldn't
    // authenticate anything. `EnvConfig::from_env` rejects it for the
    // same reason it rejects unset, with the same diagnostic. Pins the
    // `if !v.is_empty()` guard against a mutation that drops the
    // emptiness check.
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", ""),
        ("NO_TICKETS_API_URL", "http://unused.example"),
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.contains("NO_TICKETS_TOKEN"),
        "empty-token error must name the var; got {msg:?}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn publish_event_empty_api_url_treated_as_missing_var() {
    let mut c =
        McpClient::spawn_with_env(&[("NO_TICKETS_TOKEN", "nt_test"), ("NO_TICKETS_API_URL", "")])
            .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.contains("NO_TICKETS_API_URL"),
        "empty-api-url error must name the var; got {msg:?}",
    );
    c.shutdown().await;
}

// ─── Malformed server responses ──────────────────────────────────────────

#[tokio::test]
async fn publish_event_empty_ids_array_surfaces_error() {
    // Server-contract violation: a 2xx response with `ids: []` means
    // the server claims success but didn't return an event id. Surface
    // as an error rather than handing the agent a blank id it might
    // later use as `parent_event_id`. Mirrors the TS handler's
    // "missing id" defensive branch.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ingested": 1, "deduped": 0, "ids": [],
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.to_lowercase().contains("ids") || msg.to_lowercase().contains("missing"),
        "empty-ids error must mention the missing id; got {msg:?}",
    );
    c.shutdown().await;
}

// ─── Dedupe-detection truth table (ingested == 0 && deduped > 0) ─────────

#[tokio::test]
async fn publish_event_dedupe_false_when_both_zero() {
    // (ingested=0, deduped=0). Server returned an id but neither
    // ingested nor deduped. Per the TS handler's truth table, this
    // is NOT a dedupe — deduped flag must be false.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ingested": 0, "deduped": 0, "ids": ["evt_z"],
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    assert_eq!(
        payload["deduped"], false,
        "(0,0) must surface deduped=false; got {payload}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn publish_event_dedupe_false_when_both_positive() {
    // (ingested=1, deduped=1). The && in `ingested == 0 && deduped > 0`
    // requires ingested==0 for deduped:true to flip on. With ingested>0
    // the answer MUST be false even when deduped is also >0. Catches a
    // mutation from `&&` to `||`.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ingested": 1, "deduped": 1, "ids": ["evt_w"],
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    assert_eq!(
        payload["deduped"], false,
        "(1,1) must surface deduped=false (ingested>0 wins); got {payload}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn publish_event_missing_ingested_field_defaults_to_zero_for_dedupe_detection() {
    // Server response lacks `ingested`; our impl uses `unwrap_or(0)`.
    // Pin the lenient default by constructing a response where the
    // default matters: present `deduped: 5`, missing `ingested`.
    //   Default 0:   0 == 0 && 5 > 0 → deduped=true
    //   Mutation 1:  1 == 0 && 5 > 0 → deduped=false
    // Test pin asserts deduped=true. Kills `unwrap_or(1)` mutation on
    // the `ingested` line.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "deduped": 5, "ids": ["evt_m"],
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    assert_eq!(
        payload["deduped"], true,
        "missing ingested → defaults to 0 → satisfies the && → deduped=true; got {payload}",
    );
    c.shutdown().await;
}

#[tokio::test]
async fn publish_event_missing_deduped_field_defaults_to_zero_for_dedupe_detection() {
    // Symmetric pin for the `deduped` unwrap_or default. Server
    // response lacks `deduped`; present `ingested: 0`.
    //   Default 0:   0 == 0 && 0 > 0 → deduped=false
    //   Mutation 1:  0 == 0 && 1 > 0 → deduped=true
    // Test asserts deduped=false → kills `unwrap_or(1)` on `deduped`.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ingested": 0, "ids": ["evt_n"],
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    assert_eq!(
        payload["deduped"], false,
        "missing deduped → defaults to 0 → fails the > 0 check → deduped=false; got {payload}",
    );
    c.shutdown().await;
}

// ─── URL normalisation + identity-spoof protection ────────────────────────

#[tokio::test]
async fn publish_event_trailing_slash_api_url_routes_to_v1_events_unchanged() {
    // `NO_TICKETS_API_URL` with a trailing slash MUST land on
    // `<api_url>/v1/events` (single `/`, not double). Pins
    // `trim_end_matches('/')` against a regression that drops it or
    // mutates to `trim_start_matches`.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ingested": 1, "deduped": 0, "ids": ["evt_slash"],
        })))
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    assert_eq!(
        payload["id"], "evt_slash",
        "trailing-slash URL must still POST to /v1/events"
    );
    c.shutdown().await;
}

#[tokio::test]
async fn publish_event_extra_source_arg_does_not_spoof_identity() {
    // Defence in depth: the input schema doesn't declare a `source`
    // property (pinned by the discovery test), but serde's default
    // behaviour silently ignores unknown fields. If an agent (or a
    // future schema change with `#[serde(deny_unknown_fields)]`
    // dropped) passes `source: {...}` in args, it MUST NOT reach the
    // wire — `build_source` only reads the `attributes` arg (the
    // legitimate passthrough slot) and otherwise uses the fixed
    // `nt-mcp` identity.
    //
    // Adversarial review #1 (399d44f follow-up): this test used to
    // assert "no attributes block at all" — but `attributes` is now
    // a legitimate slot, so that pin couldn't distinguish "spoofed
    // attributes rejected" from "no attributes ever emitted". The
    // sharper pin: send BOTH a legit top-level `attributes` AND a
    // spoofed `source.attributes`. Assert legit keys survive, the
    // spoof keys do not, and `source.name` stays `nt-mcp`.
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;
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
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                    "attributes": { "legitTag": "ok" },
                    "source": {
                        "name": "evil-agent",
                        "sdkVersion": "0",
                        "attributes": { "spoofedTag": "no" }
                    }
                }
            }),
        )
        .await;
    let body = captured.lock().unwrap().clone().expect("body captured");
    let envelope: Value = serde_json::from_str(&body).expect("body JSON");
    let src = &envelope[0]["source"];
    assert_eq!(
        src["name"], "nt-mcp",
        "source.name must NOT be spoofable; got body={body}"
    );
    assert_eq!(
        src["attributes"]["legitTag"], "ok",
        "legitimate `attributes` arg must reach source.attributes; got src={src}",
    );
    assert!(
        src["attributes"].get("spoofedTag").is_none(),
        "spoofed `source.attributes.spoofedTag` must NOT leak through; got src={src}",
    );
}

// ─── Stdout-purity coverage for publish_event ────────────────────────────

#[tokio::test]
async fn publish_event_call_does_not_corrupt_stdout_jsonrpc_stream() {
    // The Task 2 stdout-purity test covered list_event_types. publish_
    // event has its own code path (HTTP I/O, JSON serialisation, error
    // mapping); a stray `println!` or a logger misconfigured to stdout
    // anywhere in that path would corrupt JSON-RPC framing and silently
    // disconnect Claude Code. Re-pin the invariant for publish_event.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/events"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ingested": 1, "deduped": 0, "ids": ["evt_pure"],
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
    let _ = c
        .request(
            "tools/call",
            json!({
                "name": "publish_event",
                "arguments": {
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
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
