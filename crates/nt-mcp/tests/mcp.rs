//! Integration tests for the nt-mcp server.
//!
//! Tests spawn the binary as a subprocess, drive the JSON-RPC handshake over
//! stdio, and assert on response shapes + stdout purity (no log lines mixed
//! with protocol frames — see fix doc Task 2 critical note).
//!
//! Hand-rolled minimal MCP handshake rather than using rmcp's client side,
//! so the raw stdout-purity property is directly inspectable.

use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;
use wiremock::matchers::{header, method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const READ_TIMEOUT: Duration = Duration::from_secs(5);

struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    /// Every line read from stdout, in order. Used by the stdout-purity
    /// test to inspect raw protocol bytes without racing the child for
    /// post-EOF reads.
    captured_stdout: Vec<String>,
}

impl McpClient {
    async fn spawn() -> Self {
        Self::spawn_with_env(&[]).await
    }

    /// Spawn nt-mcp with additional env vars (e.g. NO_TICKETS_TOKEN +
    /// NO_TICKETS_API_URL for publish_event tests pointing at a
    /// wiremock instance). Caller-supplied env layers on top of the
    /// inherited process env; callers should also `env_remove` any
    /// host-shell vars they want guaranteed-absent (the helper itself
    /// doesn't strip — different tests need different defaults).
    async fn spawn_with_env(extra_env: &[(&str, &str)]) -> Self {
        let bin = env!("CARGO_BIN_EXE_nt-mcp");
        let mut cmd = Command::new(bin);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Default-strip publish-time env vars so a host shell with
        // NO_TICKETS_TOKEN set can't leak into tests that don't opt in.
        // Callers that want them set pass via `extra_env` AFTER these
        // removals.
        cmd.env_remove("NO_TICKETS_TOKEN")
            .env_remove("NO_TICKETS_API_URL")
            .env_remove("NO_TICKETS_AUTH_URL")
            .env_remove("NO_TICKETS_ENV")
            .env_remove("NO_TICKETS_INCLUDE_MACHINE");
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().expect("spawn nt-mcp binary");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Self {
            child,
            stdin,
            stdout,
            next_id: 1,
            captured_stdout: Vec::new(),
        }
    }

    async fn send(&mut self, body: Value) {
        let line = format!("{body}\n");
        self.stdin
            .write_all(line.as_bytes())
            .await
            .expect("write request");
        self.stdin.flush().await.expect("flush");
    }

    async fn read_line(&mut self) -> String {
        let mut buf = String::new();
        timeout(READ_TIMEOUT, self.stdout.read_line(&mut buf))
            .await
            .expect("response within timeout")
            .expect("read response");
        self.captured_stdout.push(buf.clone());
        buf
    }

    async fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.send(req).await;
        let line = self.read_line().await;
        serde_json::from_str(line.trim()).expect("parse JSON-RPC response")
    }

    async fn notify(&mut self, method: &str, params: Value) {
        let note = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.send(note).await;
    }

    /// MCP handshake: initialize → initialized notification.
    async fn handshake(&mut self) -> Value {
        let init = self
            .request(
                "initialize",
                json!({
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": { "name": "nt-mcp-test", "version": "0.0.0" }
                }),
            )
            .await;
        self.notify("notifications/initialized", json!({})).await;
        init
    }

    /// Shut the server down and return EVERY stdout line the child
    /// emitted — including ones the test never explicitly read for
    /// request/response correlation. After dropping stdin, the server
    /// exits and stdout closes; we drain to EOF before reaping. This
    /// closes the regression introduced by the GREEN harness fix, where
    /// captured_stdout only held lines the test consumed and stray
    /// stdout writes would have been invisible.
    async fn shutdown(mut self) -> (Vec<String>, String) {
        drop(self.stdin);
        // Drain any remaining lines after stdin-close, until EOF.
        loop {
            let mut buf = String::new();
            let n = timeout(READ_TIMEOUT, self.stdout.read_line(&mut buf))
                .await
                .expect("stdout drains within timeout")
                .expect("read remaining stdout");
            if n == 0 {
                break; // EOF — stdout closed by child exit
            }
            self.captured_stdout.push(buf);
        }
        let captured = std::mem::take(&mut self.captured_stdout);
        drop(self.stdout);
        let mut stderr_buf = Vec::new();
        if let Some(mut stderr) = self.child.stderr.take() {
            use tokio::io::AsyncReadExt;
            timeout(READ_TIMEOUT, stderr.read_to_end(&mut stderr_buf))
                .await
                .expect("stderr drains within timeout")
                .expect("read stderr");
        }
        let _ = timeout(READ_TIMEOUT, self.child.wait())
            .await
            .expect("child exits within timeout")
            .expect("child exit status");
        let stderr = String::from_utf8(stderr_buf).expect("stderr utf8");
        (captured, stderr)
    }
}

// ─── Acceptance criterion: list_event_types is registered and discoverable ──

/// Exact byte-for-byte parity with the TS reference at
/// src/mcp/tools/list-event-types.ts. Pinning the literal string here
/// catches any drift, including whitespace, that a `contains` check
/// would miss.
const TS_PARITY_DESCRIPTION: &str = "List event types this caller can publish, optionally filtered by domain. Type ids follow domain.entity.action.vN grammar. Reads from the local cache; refresh fires async.";

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

// ─── Acceptance criterion: list_event_types call returns the expected shape ─

#[tokio::test]
async fn list_event_types_returns_typed_rows() {
    let mut c = McpClient::spawn().await;
    c.handshake().await;

    let resp = c
        .request(
            "tools/call",
            json!({ "name": "list_event_types", "arguments": {} }),
        )
        .await;

    // CallToolResult contract: content array with one text item containing
    // a JSON-encoded { types: [...] } payload.
    let content = resp["result"]["content"].as_array().expect("content array");
    assert_eq!(content.len(), 1, "single content item expected");
    let text = content[0]["text"].as_str().expect("text content");
    let payload: Value = serde_json::from_str(text).expect("payload parses");
    let types = payload["types"].as_array().expect("types array");
    assert!(!types.is_empty(), "spike should return at least one type");
    for t in types {
        for field in ["id", "domain", "entity", "action", "version"] {
            assert!(
                t[field].is_string(),
                "row must have string field {field}; got {t:?}",
            );
        }
        // `deprecated` is an internal filter dimension, not part of the
        // wire payload per TS parity (handlers.ts maps to id/domain/
        // entity/action/version only). A regression would expose it.
        assert!(
            t.get("deprecated").is_none(),
            "row must NOT carry the deprecated field on the wire; got {t:?}",
        );
    }

    // Pin field declaration order on the RAW wire text — the value
    // serde_json parsed into a Map alphabetises on its own re-emit,
    // so we must inspect the original string the server sent. Same
    // monotonic-byte-position approach as the nt status spike.
    let p = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("missing {needle:?} in {text:?}"))
    };
    let id = p(r#""id":"#);
    let domain = p(r#""domain":"#);
    let entity = p(r#""entity":"#);
    let action = p(r#""action":"#);
    let version = p(r#""version":"#);
    assert!(
        id < domain && domain < entity && entity < action && action < version,
        "wire field order must be id, domain, entity, action, version — got {text}",
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

// ─── domain filter narrows the result set ──────────────────────────────────

#[tokio::test]
async fn list_event_types_filters_by_domain() {
    let mut c = McpClient::spawn().await;
    c.handshake().await;

    let all = c
        .request(
            "tools/call",
            json!({ "name": "list_event_types", "arguments": {} }),
        )
        .await;
    let all_payload: Value =
        serde_json::from_str(all["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    let all_types = all_payload["types"].as_array().unwrap();

    // Pick a domain that appears in the unfiltered set.
    let target_domain = all_types[0]["domain"].as_str().unwrap().to_string();

    let filtered = c
        .request(
            "tools/call",
            json!({
                "name": "list_event_types",
                "arguments": { "domain": target_domain }
            }),
        )
        .await;
    let filtered_payload: Value =
        serde_json::from_str(filtered["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    let filtered_types = filtered_payload["types"].as_array().unwrap();
    assert!(
        !filtered_types.is_empty(),
        "filter should retain at least the row whose domain we picked"
    );
    for t in filtered_types {
        assert_eq!(
            t["domain"].as_str().unwrap(),
            target_domain,
            "domain filter must exclude other domains",
        );
    }

    // A bogus domain returns an empty array, not an error.
    let none = c
        .request(
            "tools/call",
            json!({
                "name": "list_event_types",
                "arguments": { "domain": "this-domain-does-not-exist-x9z" }
            }),
        )
        .await;
    let none_payload: Value =
        serde_json::from_str(none["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        none_payload["types"].as_array().unwrap().len(),
        0,
        "no matches → empty array, not error"
    );

    c.shutdown().await;
}

// ─── deprecated filter inverts active vs deprecated ────────────────────────

/// Pins the *direction* of the deprecated filter against known fixture
/// rows — not just "both sets non-empty + disjoint" (a backwards filter
/// passes that). Mutation testing on `t.deprecated == want` flipped to
/// `!=` survives the disjoint-non-empty contract but is caught here by
/// asserting that a known-ACTIVE id appears in `deprecated:false` and a
/// known-DEPRECATED id appears in `deprecated:true`.
///
/// Fixtures pinned here MUST stay in sync with `src/fixtures.rs`.
#[tokio::test]
async fn list_event_types_filters_by_deprecated_flag() {
    const KNOWN_ACTIVE: &str = "billing.invoice.issued.v2";
    const KNOWN_DEPRECATED: &str = "billing.invoice.issued.v1";

    let mut c = McpClient::spawn().await;
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

    let active_text = active["result"]["content"][0]["text"].as_str().unwrap();
    let deprecated_text = deprecated["result"]["content"][0]["text"].as_str().unwrap();
    let active_payload: Value = serde_json::from_str(active_text).unwrap();
    let deprecated_payload: Value = serde_json::from_str(deprecated_text).unwrap();

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

    // Direction check (the mutation-survivor kill). A backwards filter
    // fails BOTH of these asserts simultaneously — KNOWN_ACTIVE would
    // show up in the deprecated set and vice versa.
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

    // Cross-check: active and deprecated sets are disjoint by id.
    assert!(
        active_ids.is_disjoint(&deprecated_ids),
        "active and deprecated id sets must be disjoint; active={active_ids:?} deprecated={deprecated_ids:?}",
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
    let mut c = McpClient::spawn().await;
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
        // Each response must carry either a `result` or an `error` (it's
        // a response to one of our requests).
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
    // Pinning exact (not >=) means a spurious extra frame on stdout
    // (e.g., a misplaced server log line that happens to be JSON)
    // would fail.
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
    let mut c = McpClient::spawn().await;
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
    // The list_event_types tool body emits a `tracing::info!` per call.
    // After one call we should see the per-call event on stderr, not
    // just the startup line. This rules out the "only startup logged"
    // regression.
    assert!(
        stderr.contains("list_event_types called"),
        "per-call tracing event missing from stderr; got: {stderr:?}",
    );

    // Cross-check: stdout must remain pure JSON-RPC regardless of how
    // chatty stderr is.
    for line in &captured {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        serde_json::from_str::<Value>(trimmed).expect("stdout still pure JSON");
    }
}

// ─── publish_event tool (Task 19) ──────────────────────────────────────────

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

/// Extract the `tools/call publish_event` text-content JSON payload
/// from a JSON-RPC response. The MCP `CallToolResult` carries content
/// as `[{ type: "text", text: "<json string>" }]`; this helper parses
/// the inner JSON for direct field assertions.
fn extract_tool_result_payload(resp: &Value) -> Value {
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("tools/call response missing text content; got {resp:?}"));
    serde_json::from_str(text)
        .unwrap_or_else(|e| panic!("tool result text is not JSON: {e}; raw={text:?}"))
}

/// Pretty error-message accessor for assertion messages. Looks at
/// both `result.content[0].text` (which carries the structured error
/// when rmcp wraps a McpError into a CallToolResult error) AND
/// `error.message` (which carries protocol-level errors).
fn collect_error_text(resp: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(s) = resp["error"]["message"].as_str() {
        parts.push(s.to_string());
    }
    if let Some(s) = resp["result"]["content"][0]["text"].as_str() {
        parts.push(s.to_string());
    }
    parts.join(" | ")
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

    // Required: project, type, data — every event needs these for a
    // wire-valid envelope.
    let required = schema["required"]
        .as_array()
        .expect("required array")
        .iter()
        .filter_map(|v| v.as_str())
        .map(str::to_string)
        .collect::<Vec<_>>();
    for needed in ["project", "type", "data"] {
        assert!(
            required.iter().any(|r| r == needed),
            "schema must require `{needed}`; required={required:?}",
        );
    }

    // Optional fields surfaced via JSON Schema properties even though
    // they're not in `required`. Mirrors the TS reference's input
    // schema. A missing entry means the schema-derive macro lost a
    // field — caught here.
    for optional in [
        "subject",
        "occurred_at",
        "parent_event_id",
        "trace_id",
        "dedupe_key",
    ] {
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
                    "project": "demo",
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
                    "project": "demo",
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
                    "project": "demo",
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
                    "project": "demo",
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
                    "project": "demo",
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
                    "project": "demo",
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
                    "project": "demo",
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
    // that copied `nt-cli`'s default (`"nt-cli"`) would land here.
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
                    "project": "demo",
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
async fn publish_event_wire_body_carries_project_in_source_attributes() {
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
                    "project": "demo-project",
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                }
            }),
        )
        .await;
    let body = captured.lock().unwrap().clone().expect("body captured");
    let envelope: Value = serde_json::from_str(&body).expect("body JSON");
    assert_eq!(
        envelope[0]["source"]["attributes"]["project"], "demo-project",
        "project arg must land on source.attributes.project; got body={body}",
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
                    "project": "demo",
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
                    "project": "demo",
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
                    "project": "demo",
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
                    "project": "demo",
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
                    "project": "demo",
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
                    "project": "demo",
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
                    "project": "demo",
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
                    "project": "demo",
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
                    "project": "demo",
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
    // wire — `build_source` ignores tool args entirely and uses the
    // fixed `nt-mcp` identity.
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
                    "project": "demo",
                    "type": "ai.task.completed.v1",
                    "data": valid_ai_task_data(),
                    "source": { "name": "evil-agent", "sdkVersion": "0", "attributes": { "spoofed": "yes" } }
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
    assert!(
        src["attributes"].get("spoofed").is_none(),
        "spoofed attribute must not leak; got src={src}",
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
                    "project": "demo",
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
