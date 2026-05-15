//! Shared harness for the `nt publish` integration tests.
//!
//! Spawns `nt publish` as a subprocess against a `wiremock` mock server,
//! drives stdin/stdout/stderr, and returns a captured `Output`. Includes
//! the `SequencedResponder` used by the retry suite, the
//! `capture_publish_body` helper used by every wire-shape test, and the
//! `BASE_*` / `base_args` / `envelope` / `batch_file` builders that keep
//! per-test boilerplate down.

use std::collections::VecDeque;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use assert_cmd::cargo::cargo_bin;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// Plays a scripted sequence of `ResponseTemplate`s in FIFO. One template
/// consumed per inbound request. When the queue is exhausted, returns
/// 599 so any extra request fails loudly (rather than silently matching
/// an unwanted default). Used to assert retry-then-success sequencing
/// without leaking sequencing logic across multiple Mock mounts.
pub(crate) struct SequencedResponder {
    responses: Mutex<VecDeque<ResponseTemplate>>,
}

impl SequencedResponder {
    pub(crate) fn new(responses: Vec<ResponseTemplate>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().collect()),
        }
    }
}

impl Respond for SequencedResponder {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let mut q = self.responses.lock().unwrap();
        q.pop_front().unwrap_or_else(|| {
            ResponseTemplate::new(599)
                .set_body_string("SequencedResponder exhausted — too many requests")
        })
    }
}

#[derive(Debug)]
pub(crate) struct Output {
    pub(crate) code: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

/// Run `nt publish` with the wiremock URL, an isolated NO_TICKETS_HOME,
/// and an env-supplied bearer token. Returns the captured exit + IO.
pub(crate) async fn run_nt_publish(
    server_uri: &str,
    token: Option<&str>,
    home: &Path,
    args: &[&str],
) -> Output {
    run_nt_publish_with_env(server_uri, token, home, &[], args).await
}

/// Variant that lets a test set additional env vars on top of the
/// hermetic defaults (e.g. `NO_TICKETS_INCLUDE_MACHINE=1`). The
/// extras are applied AFTER the helper's defaults, so they override
/// — useful for testing env-gated features without forking the helper.
pub(crate) async fn run_nt_publish_with_env(
    server_uri: &str,
    token: Option<&str>,
    home: &Path,
    extra_env: &[(&str, &str)],
    args: &[&str],
) -> Output {
    let mut cmd = Command::new(cargo_bin("nt"));
    cmd.env("NO_TICKETS_HOME", home)
        // ADR-0002 layer 2/3 mutual exclusion: NO_TICKETS_ENV set in the
        // host shell collides with the explicit pair we set below and
        // surfaces EnvAndPairBothSet. Clear it for hermeticity.
        .env_remove("NO_TICKETS_ENV")
        // Default-off for the machine-hash attribute (Task 18). Each
        // test that needs it must opt in explicitly via cmd.env(...)
        // after this helper returns; default tests must NOT pick up
        // a host shell where this is set.
        .env_remove("NO_TICKETS_INCLUDE_MACHINE")
        .env("NO_TICKETS_API_URL", server_uri)
        // The url-resolver enforces both env vars must be set together;
        // give it a placeholder for AUTH (publish never reads it).
        .env("NO_TICKETS_AUTH_URL", "https://unused.example/auth")
        // Test-side speed-up: collapse exponential backoff to zero so
        // the retry suite doesn't pay 100–300ms per worst-case run.
        // Retry behaviour (call counts, classification, give-up
        // surfacing) is unaffected — only the sleep durations change.
        // The unit tests in `transport::retry_tests` own the schedule
        // pin via `RecordingSleeper`.
        .env("NT_RETRY_BASE_DELAY_MS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(t) = token {
        cmd.env("NO_TICKETS_TOKEN", t);
    } else {
        cmd.env_remove("NO_TICKETS_TOKEN");
    }
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    cmd.arg("publish");
    for a in args {
        cmd.arg(a);
    }
    let mut child = cmd.spawn().expect("spawn nt binary");
    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    let mut stdout = child.stdout.take().expect("stdout pipe");
    let mut stderr = child.stderr.take().expect("stderr pipe");
    let (s_out, s_err, status) = tokio::join!(
        stdout.read_to_end(&mut stdout_buf),
        stderr.read_to_end(&mut stderr_buf),
        child.wait(),
    );
    s_out.expect("read stdout");
    s_err.expect("read stderr");
    let status = status.expect("child exits");
    Output {
        code: status.code().unwrap_or(-1),
        stdout: String::from_utf8(stdout_buf).expect("stdout utf8"),
        stderr: String::from_utf8(stderr_buf).expect("stderr utf8"),
    }
}

pub(crate) fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

pub(crate) const HAPPY_RESPONSE: &str = r#"{"ingested":1,"deduped":0,"ids":["x"]}"#;

/// Mounts a wiremock that records the request body and replies 200 with
/// `HAPPY_RESPONSE`. The returned `Arc<Mutex<Option<String>>>` resolves
/// to the captured body once the request lands. Used by every wire-
/// shape assertion in the metadata + batch suites.
pub(crate) async fn capture_publish_body(server: &MockServer) -> Arc<Mutex<Option<String>>> {
    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let captured_for_responder = captured.clone();
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(move |req: &wiremock::Request| {
            let body = String::from_utf8(req.body.clone()).expect("body utf8");
            *captured_for_responder.lock().unwrap() = Some(body);
            ResponseTemplate::new(200).set_body_raw(HAPPY_RESPONSE.as_bytes(), "application/json")
        })
        .expect(1)
        .mount(server)
        .await;
    captured
}

pub(crate) const BASE_TYPE: &str = "ai.task.completed.v1";
pub(crate) const BASE_DATA: &str = r#"{"taskId":"t-1","sessionId":"s-1"}"#;

pub(crate) fn base_args() -> Vec<&'static str> {
    base_args_with_data(BASE_DATA)
}

/// Same shape as `base_args` but with caller-supplied `--data`. Used by
/// the error-handling and retry suites, which need an empty `{}` data
/// payload (the wire-shape pin in `metadata` requires real fields, so
/// it stays on `base_args`). Centralising the four other args keeps a
/// future "rename --project" / "add a required global flag" refactor
/// from being a sweep across nine call sites.
pub(crate) fn base_args_with_data(data: &'static str) -> Vec<&'static str> {
    vec!["--type", BASE_TYPE, "--data", data, "--project", "demo"]
}

/// Parse the captured wire body (a JSON array containing exactly one
/// envelope) and return that envelope as a `Value`. Decoupling from
/// the raw bytes lets the per-field tests assert on `body["subject"]
/// ["type"]` rather than the inner-object substring form, which was
/// brittle to inner-key-order regressions.
pub(crate) fn envelope(raw: &str) -> Value {
    let arr: Value = serde_json::from_str(raw).expect("body parses");
    arr.as_array()
        .and_then(|a| a.first())
        .cloned()
        .expect("envelope at index 0")
}

pub(crate) const VALID_AI_TASK_DATA: &str = r#"{"taskId":"task-1","sessionId":"sess-1","startedAt":"2026-05-01T00:00:00.000Z","completedAt":"2026-05-01T00:00:01.000Z","durationMs":1000,"outcome":"success","callCount":1}"#;

pub(crate) fn batch_file(dir: &Path, lines: &[String]) -> std::path::PathBuf {
    let path = dir.join("events.jsonl");
    let mut file = fs::File::create(&path).expect("create JSONL file");
    for line in lines {
        writeln!(file, "{line}").expect("write JSONL line");
    }
    path
}
