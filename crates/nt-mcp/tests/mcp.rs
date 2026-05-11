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

    /// Shuts the server down and returns:
    /// - the lines this client captured from stdout (everything we read
    ///   line-by-line via the BufReader during the test), and
    /// - stderr in full from the now-exited child.
    ///
    /// Reading stdout via `wait_with_output()` after we've already
    /// consumed the protocol frames via BufReader would yield nothing —
    /// the captured_stdout buffer is the source of truth for what
    /// crossed the wire.
    async fn shutdown(mut self) -> (Vec<String>, String) {
        drop(self.stdin);
        // Drop the BufReader to release its hold on child.stdout so
        // wait_with_output() doesn't deadlock waiting on a borrow.
        // We use a different approach: spawn drain + wait separately.
        let captured = std::mem::take(&mut self.captured_stdout);
        // The remaining child holds onto stdout via BufReader; convert
        // it back so we can let wait() reap the process.
        drop(self.stdout);
        let output = timeout(READ_TIMEOUT, self.child.wait_with_output())
            .await
            .expect("child exits within timeout")
            .expect("child output");
        let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
        (captured, stderr)
    }
}

// ─── Acceptance criterion: list_event_types is registered and discoverable ──

#[tokio::test]
async fn tools_list_includes_list_event_types_with_ts_parity_description() {
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

    let desc = entry["description"].as_str().unwrap_or("");
    assert!(
        desc.contains("List event types"),
        "description should match TS parity; got {desc:?}",
    );
    assert!(
        desc.contains("domain.entity.action.vN"),
        "description should name the type-id grammar; got {desc:?}",
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
    }

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

#[tokio::test]
async fn list_event_types_filters_by_deprecated_flag() {
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
    // Spike fixture: at least one of each so the contract is exercised.
    assert!(
        !active_payload["types"].as_array().unwrap().is_empty(),
        "active filter must return at least one row in the spike fixture",
    );
    assert!(
        !deprecated_payload["types"].as_array().unwrap().is_empty(),
        "deprecated filter must return at least one row in the spike fixture",
    );

    // Cross-check: active and deprecated rows do not overlap by id.
    let active_ids: std::collections::HashSet<&str> = active_payload["types"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    for t in deprecated_payload["types"].as_array().unwrap() {
        let id = t["id"].as_str().unwrap();
        assert!(
            !active_ids.contains(id),
            "id {id} appeared in both active and deprecated sets",
        );
    }

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
    // Initialize + 5 tool calls = 6 expected responses minimum.
    assert!(
        frame_count >= 6,
        "expected at least 6 JSON-RPC frames on stdout; saw {frame_count}",
    );
}

// ─── Stderr is allowed to carry logs ────────────────────────────────────────

/// Counterpart to the stdout-purity test: confirms that logging is wired
/// to stderr (where it belongs), and that stderr being noisy does NOT
/// corrupt stdout. The exact log content depends on tracing-subscriber
/// configuration; we just assert that the server runs to completion with
/// stderr non-empty (any output at all proves the writer is connected).
#[tokio::test]
async fn stderr_receives_logs_without_polluting_stdout() {
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
