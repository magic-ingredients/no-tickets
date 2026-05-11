//! Integration tests for the nt-mcp server.
//!
//! Tests spawn the binary as a subprocess, drive the JSON-RPC handshake over
//! stdio, and assert on response shapes + stdout purity (no log lines mixed
//! with protocol frames — see fix doc Task 2 critical note).
//!
//! Hand-rolled minimal MCP handshake rather than using rmcp's client side,
//! so the raw stdout-purity property is directly inspectable.

use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

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
        let bin = env!("CARGO_BIN_EXE_nt-mcp");
        let mut child = Command::new(bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn nt-mcp binary");
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
    let tools = resp["result"]["tools"]
        .as_array()
        .expect("tools array");
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
        required.map_or(true, |r| r.is_empty()),
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
    let content = resp["result"]["content"]
        .as_array()
        .expect("content array");
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
    let filtered_payload: Value = serde_json::from_str(
        filtered["result"]["content"][0]["text"].as_str().unwrap(),
    )
    .unwrap();
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
