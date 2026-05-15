//! Shared harness for the nt-mcp integration tests.
//!
//! `McpClient` spawns the `nt-mcp` binary as a subprocess and drives a
//! minimal JSON-RPC handshake over stdio. Hand-rolled (rather than
//! using rmcp's client side) so the raw stdout-purity property is
//! directly inspectable.
//!
//! Tool-specific helpers live next to their tests (publish_event has
//! its own `valid_ai_task_data` + `capture_publish_body`;
//! describe_event_type has its own `detail_body_minimal`); only the
//! generally-applicable response extractors live here.

use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

const READ_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct McpClient {
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
    pub(crate) async fn spawn() -> Self {
        Self::spawn_with_env(&[]).await
    }

    /// Spawn nt-mcp with additional env vars (e.g. NO_TICKETS_TOKEN +
    /// NO_TICKETS_API_URL for publish_event tests pointing at a
    /// wiremock instance). Caller-supplied env layers on top of the
    /// inherited process env; callers should also `env_remove` any
    /// host-shell vars they want guaranteed-absent (the helper itself
    /// doesn't strip — different tests need different defaults).
    pub(crate) async fn spawn_with_env(extra_env: &[(&str, &str)]) -> Self {
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

    pub(crate) async fn request(&mut self, method: &str, params: Value) -> Value {
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
    pub(crate) async fn handshake(&mut self) -> Value {
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
    pub(crate) async fn shutdown(mut self) -> (Vec<String>, String) {
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

/// Extract the `tools/call <tool>` text-content JSON payload from a
/// JSON-RPC response. The MCP `CallToolResult` carries content as
/// `[{ type: "text", text: "<json string>" }]`; this helper parses
/// the inner JSON for direct field assertions. Used by both
/// publish_event and describe_event_type tests.
pub(crate) fn extract_tool_result_payload(resp: &Value) -> Value {
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("tools/call response missing text content; got {resp:?}"));
    serde_json::from_str(text)
        .unwrap_or_else(|e| panic!("tool result text is not JSON: {e}; raw={text:?}"))
}

/// Pretty error-message accessor for assertion messages. Looks at
/// both `result.content[0].text` (which carries the structured error
/// when rmcp wraps a McpError into a CallToolResult error) AND
/// `error.message` (which carries protocol-level errors). Used by
/// both publish_event and describe_event_type failure-mode tests.
pub(crate) fn collect_error_text(resp: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(s) = resp["error"]["message"].as_str() {
        parts.push(s.to_string());
    }
    if let Some(s) = resp["result"]["content"][0]["text"].as_str() {
        parts.push(s.to_string());
    }
    parts.join(" | ")
}
