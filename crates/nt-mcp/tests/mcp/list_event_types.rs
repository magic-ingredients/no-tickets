//! `list_event_types` tool: discovery, real-server GET against
//! `/v1/registry/event-types` with bearer auth, in-memory caching with
//! async refresh, domain/deprecated filters applied client-side on the
//! cached rows. Also covers initialize-handshake parity, unknown-tool
//! error mapping, and the stdout-purity + stderr-logging cross-cutting
//! invariants (those tests use list_event_types as their driver tool).

use std::time::Duration;

use serde_json::{json, Value};
use tokio::time::sleep;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::{collect_error_text, extract_tool_result_payload, McpClient};

// ─── Acceptance criterion: list_event_types is registered and discoverable ──

/// Exact byte-for-byte parity with the TS reference at
/// src/mcp/tools/list-event-types.ts. Pinning the literal string here
/// catches any drift, including whitespace, that a `contains` check
/// would miss.
const TS_PARITY_DESCRIPTION: &str = "List event types this caller can publish, optionally filtered by domain. Type ids follow domain.entity.action.vN grammar. Reads from the local cache; refresh fires async.";

/// Canonical list-endpoint body shape — matches the TS
/// `listResponseSchema` in src/registry/client.ts: a top-level
/// `eventTypes` envelope wrapping an array of specs.
/// `deprecatedAt: null` means active; a string datetime means
/// deprecated. Only the fields tests assert on are populated.
fn list_body(rows: &[(&str, &str, &str, &str, &str, Option<&str>)]) -> Value {
    let arr: Vec<Value> = rows
        .iter()
        .map(|(id, domain, entity, action, version, deprecated_at)| {
            let mut row = serde_json::Map::new();
            row.insert("id".to_string(), Value::String(id.to_string()));
            row.insert("domain".to_string(), Value::String(domain.to_string()));
            row.insert("entity".to_string(), Value::String(entity.to_string()));
            row.insert("action".to_string(), Value::String(action.to_string()));
            row.insert("version".to_string(), Value::String(version.to_string()));
            row.insert(
                "deprecatedAt".to_string(),
                match deprecated_at {
                    Some(s) => Value::String((*s).to_string()),
                    None => Value::Null,
                },
            );
            Value::Object(row)
        })
        .collect();
    json!({ "eventTypes": arr })
}

#[tokio::test]
async fn tools_list_includes_list_event_types_with_exact_ts_parity_description() {
    let mut c = McpClient::spawn().await;
    c.handshake().await;

    let resp = c.request("tools/list", json!({})).await;
    let tools = resp["result"]["tools"].as_array().expect("tools array");
    let entry = tools
        .iter()
        .find(|t| t["name"] == "list_event_types")
        .expect("list_event_types tool registered");

    assert_eq!(
        entry["description"].as_str(),
        Some(TS_PARITY_DESCRIPTION),
        "description must byte-match the TS reference",
    );

    // Input schema must declare optional `domain` and `deprecated` parameters.
    let schema = &entry["inputSchema"];
    let props = &schema["properties"];
    assert!(
        props["domain"].is_object(),
        "schema must declare a `domain` property",
    );
    assert!(
        props["deprecated"].is_object(),
        "schema must declare a `deprecated` property",
    );
    // Neither parameter is required.
    let required = schema["required"].as_array();
    assert!(
        required.is_none_or(|r| r.is_empty()),
        "domain and deprecated must both be optional",
    );

    c.shutdown().await;
}

/// `serverInfo.name` in the initialize response must match the TS server
/// (src/mcp/create-server.ts reports `no-tickets`). Without this pin a
/// regression on `Implementation::from_build_env()` would silently
/// switch the reported name to whatever the Rust crate is called.
#[tokio::test]
async fn initialize_reports_ts_parity_server_name() {
    let mut c = McpClient::spawn().await;
    let init = c.handshake().await;
    assert_eq!(
        init["result"]["serverInfo"]["name"].as_str(),
        Some("no-tickets"),
        "serverInfo.name must match the TS server identity",
    );
    c.shutdown().await;
}

// ─── Real-server: GET against /v1/registry/event-types ─────────────────────

/// First-call behaviour: GETs the registry endpoint with the Bearer
/// token, parses the `{eventTypes: [...]}` envelope, and returns each
/// row's id/domain/entity/action/version on the wire. Replaces the
/// Task 2 spike fixture-based path.
#[tokio::test]
async fn list_event_types_issues_get_against_registry_with_bearer_and_returns_rows() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/registry/event-types"))
        .and(header("authorization", "Bearer nt_test_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list_body(&[
            ("ai.task.completed.v1", "ai", "task", "completed", "v1", None),
            (
                "billing.invoice.issued.v2",
                "billing",
                "invoice",
                "issued",
                "v2",
                None,
            ),
        ])))
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
            json!({ "name": "list_event_types", "arguments": {} }),
        )
        .await;
    let payload = extract_tool_result_payload(&resp);
    let types = payload["types"].as_array().expect("types array");
    assert_eq!(types.len(), 2, "two rows expected; got {payload}");
    let ids: Vec<&str> = types.iter().filter_map(|t| t["id"].as_str()).collect();
    assert!(ids.contains(&"ai.task.completed.v1"));
    assert!(ids.contains(&"billing.invoice.issued.v2"));
    // Wire-shape: each row carries id/domain/entity/action/version,
    // NOT deprecatedAt (that's an internal filter dimension stripped
    // by the handler — matches the spike's contract).
    for t in types {
        for field in ["id", "domain", "entity", "action", "version"] {
            assert!(
                t[field].is_string(),
                "row must have string field {field}; got {t:?}",
            );
        }
        assert!(
            t.get("deprecatedAt").is_none() && t.get("deprecated_at").is_none(),
            "row must NOT carry deprecation timestamps on the wire; got {t:?}",
        );
    }
    c.shutdown().await;
}

/// Caching contract: the second invocation within an MCP session MUST
/// return data from the in-memory cache rather than re-fetching the
/// full body. Async refresh fires after a cached read, but the read
/// itself is synchronous + cache-served.
///
/// To prove the cache is doing the work: mock the server to return
/// DIFFERENT data on the first vs second hit (via a wiremock priority
/// trick — fall-through `.up_to_n_times(1)` then a different responder).
/// The second tool call's PAYLOAD must reflect the FIRST response, not
/// the second — proving the cache served the read. The async refresh
/// after the second call may hit the server again; we don't assert
/// hit counts here because async refresh timing is not deterministic.
#[tokio::test]
async fn list_event_types_second_call_returns_cached_rows_not_refetched_body() {
    let server = MockServer::start().await;
    // First response: one row. Second (and beyond): different row.
    Mock::given(method("GET"))
        .and(path("/v1/registry/event-types"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list_body(&[(
            "first.call.value.v1",
            "first",
            "call",
            "value",
            "v1",
            None,
        )])))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/registry/event-types"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list_body(&[(
            "second.call.value.v1",
            "second",
            "call",
            "value",
            "v1",
            None,
        )])))
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test_token"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    // First call populates the cache from the FIRST responder.
    let first = c
        .request(
            "tools/call",
            json!({ "name": "list_event_types", "arguments": {} }),
        )
        .await;
    let first_payload = extract_tool_result_payload(&first);
    assert_eq!(first_payload["types"][0]["id"], "first.call.value.v1");
    // Second call MUST serve the cached body, not the new server data.
    let second = c
        .request(
            "tools/call",
            json!({ "name": "list_event_types", "arguments": {} }),
        )
        .await;
    let second_payload = extract_tool_result_payload(&second);
    assert_eq!(
        second_payload["types"][0]["id"], "first.call.value.v1",
        "second call must read from the cache, not re-fetch; got {second_payload}",
    );
    c.shutdown().await;
}

// ─── Behavior: filters apply client-side over the cached set ───────────────

#[tokio::test]
async fn list_event_types_filters_by_domain() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/registry/event-types"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list_body(&[
            ("ai.task.completed.v1", "ai", "task", "completed", "v1", None),
            (
                "billing.invoice.issued.v2",
                "billing",
                "invoice",
                "issued",
                "v2",
                None,
            ),
            ("auth.session.created.v1", "auth", "session", "created", "v1", None),
        ])))
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;

    let filtered = c
        .request(
            "tools/call",
            json!({
                "name": "list_event_types",
                "arguments": { "domain": "billing" }
            }),
        )
        .await;
    let payload = extract_tool_result_payload(&filtered);
    let types = payload["types"].as_array().unwrap();
    assert!(
        !types.is_empty(),
        "filter should retain rows whose domain matches"
    );
    for t in types {
        assert_eq!(
            t["domain"].as_str().unwrap(),
            "billing",
            "domain filter must exclude other domains",
        );
    }

    let none = c
        .request(
            "tools/call",
            json!({
                "name": "list_event_types",
                "arguments": { "domain": "this-domain-does-not-exist-x9z" }
            }),
        )
        .await;
    let none_payload = extract_tool_result_payload(&none);
    assert_eq!(
        none_payload["types"].as_array().unwrap().len(),
        0,
        "no matches → empty array, not error"
    );

    c.shutdown().await;
}

/// Deprecation semantics: a row is "deprecated" when `deprecatedAt` is
/// a non-null datetime; null/missing means active. Filter pins the
/// direction with known fixtures so a backwards predicate (mutation
/// `==` → `!=`) is caught.
#[tokio::test]
async fn list_event_types_filters_by_deprecated_flag() {
    const KNOWN_ACTIVE: &str = "billing.invoice.issued.v2";
    const KNOWN_DEPRECATED: &str = "billing.invoice.issued.v1";

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/registry/event-types"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list_body(&[
            (
                KNOWN_ACTIVE,
                "billing",
                "invoice",
                "issued",
                "v2",
                None,
            ),
            (
                KNOWN_DEPRECATED,
                "billing",
                "invoice",
                "issued",
                "v1",
                Some("2026-01-01T00:00:00.000Z"),
            ),
        ])))
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;

    let active = c
        .request(
            "tools/call",
            json!({
                "name": "list_event_types",
                "arguments": { "deprecated": false }
            }),
        )
        .await;
    let deprecated = c
        .request(
            "tools/call",
            json!({
                "name": "list_event_types",
                "arguments": { "deprecated": true }
            }),
        )
        .await;

    let active_payload = extract_tool_result_payload(&active);
    let deprecated_payload = extract_tool_result_payload(&deprecated);
    let collect_ids = |payload: &Value| {
        payload["types"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap().to_string())
            .collect::<std::collections::HashSet<_>>()
    };
    let active_ids = collect_ids(&active_payload);
    let deprecated_ids = collect_ids(&deprecated_payload);

    assert!(
        active_ids.contains(KNOWN_ACTIVE),
        "deprecated:false must include known-active id {KNOWN_ACTIVE}; got {active_ids:?}",
    );
    assert!(
        deprecated_ids.contains(KNOWN_DEPRECATED),
        "deprecated:true must include known-deprecated id {KNOWN_DEPRECATED}; got {deprecated_ids:?}",
    );
    assert!(
        !active_ids.contains(KNOWN_DEPRECATED),
        "deprecated:false must NOT include deprecated row {KNOWN_DEPRECATED}",
    );
    assert!(
        !deprecated_ids.contains(KNOWN_ACTIVE),
        "deprecated:true must NOT include active row {KNOWN_ACTIVE}",
    );
    assert!(
        active_ids.is_disjoint(&deprecated_ids),
        "active and deprecated id sets must be disjoint; active={active_ids:?} deprecated={deprecated_ids:?}",
    );

    c.shutdown().await;
}

// ─── Behavior: failure modes ───────────────────────────────────────────────

#[tokio::test]
async fn list_event_types_missing_token_surfaces_auth_error_before_http() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/registry/event-types"))
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
            json!({ "name": "list_event_types", "arguments": {} }),
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
async fn list_event_types_missing_api_url_surfaces_config_error_before_http() {
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        // NO_TICKETS_API_URL deliberately absent
    ])
    .await;
    c.handshake().await;
    let resp = c
        .request(
            "tools/call",
            json!({ "name": "list_event_types", "arguments": {} }),
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
async fn list_event_types_5xx_on_cold_cache_surfaces_transport_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/registry/event-types"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream registry overloaded"))
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
            json!({ "name": "list_event_types", "arguments": {} }),
        )
        .await;
    let msg = collect_error_text(&resp);
    assert!(
        msg.contains("503"),
        "cold-cache 5xx must surface upstream status code; got {msg:?}",
    );
    c.shutdown().await;
}

/// Refresh-failure tolerance: once the cache is populated, subsequent
/// requests must keep serving cached data even if the server starts
/// failing. The PRD framing: "If refresh fails, log a debug-level note;
/// never error the user-facing command." Pinned end-to-end with a
/// server that succeeds once then 503s.
#[tokio::test]
async fn list_event_types_refresh_failure_after_population_keeps_serving_cache() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/registry/event-types"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list_body(&[(
            "cached.event.type.v1",
            "cached",
            "event",
            "type",
            "v1",
            None,
        )])))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/registry/event-types"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    // First call populates cache; second call hits cache + spawns
    // refresh that 503s. Give the spawned refresh a moment to land
    // (and fail) before the third call so the failure mode actually
    // exercises.
    let first = c
        .request(
            "tools/call",
            json!({ "name": "list_event_types", "arguments": {} }),
        )
        .await;
    assert_eq!(
        extract_tool_result_payload(&first)["types"][0]["id"],
        "cached.event.type.v1",
    );
    let _ = c
        .request(
            "tools/call",
            json!({ "name": "list_event_types", "arguments": {} }),
        )
        .await;
    sleep(Duration::from_millis(100)).await;
    let third = c
        .request(
            "tools/call",
            json!({ "name": "list_event_types", "arguments": {} }),
        )
        .await;
    let payload = extract_tool_result_payload(&third);
    assert_eq!(
        payload["types"][0]["id"], "cached.event.type.v1",
        "refresh failure must NOT poison the cache; got {payload}",
    );
    c.shutdown().await;
}

/// Calling an unknown tool name must produce a JSON-RPC error response,
/// not a panic. rmcp's router should handle this; pinned so it doesn't
/// regress.
#[tokio::test]
async fn unknown_tool_returns_error_not_panic() {
    let mut c = McpClient::spawn().await;
    c.handshake().await;

    let resp = c
        .request(
            "tools/call",
            json!({ "name": "does_not_exist", "arguments": {} }),
        )
        .await;

    let has_error = !resp["error"].is_null();
    let has_is_error_true = resp["result"]["isError"] == json!(true);
    assert!(
        has_error || has_is_error_true,
        "unknown tool must produce error response; got {resp}",
    );

    c.shutdown().await;
}

// ─── Acceptance criterion: stdout purity ────────────────────────────────────

/// Under repeated tool invocation, every stdout byte must be part of a
/// valid JSON-RPC frame. Log lines on stdout corrupt the protocol and
/// cause Claude Code to silently disconnect — this is the explicit
/// critical note in the fix doc (Task 2).
#[tokio::test]
async fn stdout_contains_only_jsonrpc_frames() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/registry/event-types"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list_body(&[(
            "ai.task.completed.v1",
            "ai",
            "task",
            "completed",
            "v1",
            None,
        )])))
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;

    // Drive multiple tool calls to maximise the chance of any stray log
    // line slipping in.
    for _ in 0..5 {
        c.request(
            "tools/call",
            json!({ "name": "list_event_types", "arguments": {} }),
        )
        .await;
    }

    let (captured, _stderr) = c.shutdown().await;

    // Every non-empty line must parse as a JSON-RPC response.
    let mut frame_count = 0_usize;
    for (i, raw_line) in captured.iter().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line).unwrap_or_else(|e| {
            panic!(
                "stdout line {i} is not valid JSON: {e}\nline: {line:?}\nfull capture: {captured:?}"
            )
        });
        assert_eq!(
            value["jsonrpc"].as_str(),
            Some("2.0"),
            "stdout line {i} is JSON but not a JSON-RPC frame (missing or wrong jsonrpc field): {line:?}",
        );
        let has_result = !value["result"].is_null();
        let has_error = !value["error"].is_null();
        assert!(
            has_result || has_error,
            "stdout line {i} is JSON-RPC but neither result nor error: {line:?}",
        );
        frame_count += 1;
    }
    // Initialize + 5 tool calls = exactly 6 responses. The
    // `notifications/initialized` notification expects no response.
    assert_eq!(
        frame_count, 6,
        "expected exactly 6 JSON-RPC frames on stdout; saw {frame_count} (captured: {captured:?})",
    );
}

// ─── Stderr is allowed to carry logs ────────────────────────────────────────

/// Counterpart to the stdout-purity test: confirms that BOTH startup
/// AND per-tool-call logging is wired to stderr, and that stderr being
/// noisy doesn't corrupt stdout. The previous version only proved the
/// startup line landed on stderr — silent regression on per-call
/// tracing would have passed.
#[tokio::test]
async fn stderr_receives_per_call_logs_without_polluting_stdout() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/registry/event-types"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list_body(&[(
            "ai.task.completed.v1",
            "ai",
            "task",
            "completed",
            "v1",
            None,
        )])))
        .mount(&server)
        .await;
    let uri = server.uri();
    let mut c = McpClient::spawn_with_env(&[
        ("NO_TICKETS_TOKEN", "nt_test"),
        ("NO_TICKETS_API_URL", uri.as_str()),
    ])
    .await;
    c.handshake().await;
    c.request(
        "tools/call",
        json!({ "name": "list_event_types", "arguments": {} }),
    )
    .await;

    let (captured, stderr) = c.shutdown().await;
    assert!(
        !stderr.is_empty(),
        "tracing-subscriber must be routing to stderr; got empty stderr",
    );
    assert!(
        stderr.contains("list_event_types called"),
        "per-call tracing event missing from stderr; got: {stderr:?}",
    );

    for line in &captured {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        serde_json::from_str::<Value>(trimmed).expect("stdout still pure JSON");
    }
}
