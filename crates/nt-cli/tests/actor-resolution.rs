//! End-to-end actor-resolution tests for `no-tickets publish`.
//!
//! These tests drive the binary via `assert_cmd` + `wiremock`, asserting:
//!   - The wire body carries `metadata.actor` when an actor resolved.
//!   - The wire body OMITS `metadata` entirely when nothing resolved
//!     (no `"metadata": null` on the wire).
//!   - The first-publish hint fires once on the unattributed branch,
//!     sets the `state.json` marker, and stays silent thereafter.
//!   - `--quiet` suppresses the stderr hint but still sets the marker.
//!   - `no-tickets session end` clears the marker so a later
//!     unattributed publish re-fires the hint.
//!   - Session-attributed publishes perform ZERO `state.json` IO.
//!
//! Pure precedence-chain unit coverage lives in
//! `crates/nt-cli/src/actor.rs::tests`. These tests focus on the
//! observable CLI contract for harness integrators.

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;

use assert_cmd::cargo::cargo_bin;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

// ─── harness ────────────────────────────────────────────────────────────────

struct Output {
    code: i32,
    #[allow(dead_code)]
    stdout: String,
    stderr: String,
}

const HAPPY_RESPONSE: &str = r#"{"ingested":1,"deduped":0,"ids":["x"]}"#;
const BASE_TYPE: &str = "ai.task.completed.v1";
const BASE_DATA: &str = r#"{"taskId":"t-1"}"#;
const PUSH_TOKEN: &str = "nt_push_test_token";

fn nt_publish_cmd() -> Command {
    Command::new(cargo_bin("no-tickets"))
}

async fn run_publish(server_uri: &str, home: &Path, args: &[&str]) -> Output {
    let mut cmd = nt_publish_cmd();
    cmd.env("NO_TICKETS_HOME", home)
        .env_remove("NO_TICKETS_TOKEN")
        .env_remove("NO_TICKETS_ENV")
        .env_remove("NO_TICKETS_SESSION_FILE")
        .env_remove("NO_TICKETS_QUIET")
        .env_remove("NO_TICKETS_INCLUDE_MACHINE")
        .env("NO_TICKETS_API_URL", server_uri)
        .env("NO_TICKETS_AUTH_URL", "https://unused.example/auth")
        .env("NO_TICKETS_TOKEN", PUSH_TOKEN)
        .env("NT_RETRY_BASE_DELAY_MS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("publish");
    for a in args {
        cmd.arg(a);
    }
    let mut child = cmd.spawn().expect("spawn nt");
    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    let mut stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");
    let (s_out, s_err, status) = tokio::join!(
        stdout.read_to_end(&mut stdout_buf),
        stderr.read_to_end(&mut stderr_buf),
        child.wait(),
    );
    s_out.expect("stdout read");
    s_err.expect("stderr read");
    let status = status.expect("child exits");
    Output {
        code: status.code().unwrap_or(-1),
        stdout: String::from_utf8(stdout_buf).expect("utf8 stdout"),
        stderr: String::from_utf8(stderr_buf).expect("utf8 stderr"),
    }
}

async fn capture_body(server: &MockServer) -> std::sync::Arc<Mutex<Option<String>>> {
    let captured: std::sync::Arc<Mutex<Option<String>>> = std::sync::Arc::new(Mutex::new(None));
    let sink = captured.clone();
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(move |req: &Request| {
            *sink.lock().unwrap() = Some(String::from_utf8(req.body.clone()).unwrap());
            ResponseTemplate::new(200).set_body_raw(HAPPY_RESPONSE.as_bytes(), "application/json")
        })
        .expect(1)
        .mount(server)
        .await;
    captured
}

fn base_args() -> Vec<&'static str> {
    vec![
        "--type",
        BASE_TYPE,
        "--data",
        BASE_DATA,
        "--project",
        "demo",
    ]
}

fn parse_envelope(raw: &str) -> Value {
    let arr: Value = serde_json::from_str(raw).expect("body parses");
    arr.as_array()
        .and_then(|a| a.first())
        .cloned()
        .expect("envelope present")
}

fn session_path(home: &Path) -> PathBuf {
    home.join(".notickets").join("active-session.json")
}

fn state_path(home: &Path) -> PathBuf {
    home.join(".notickets").join("state.json")
}

fn write_session_file(path: &Path, started_at: &str, agent_id: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(
        path,
        format!(
            r#"{{"version":1,"actor":{{"type":"agent","agentId":"{agent_id}"}},"startedAt":"{started_at}","pid":1,"maxAgeHours":24}}"#,
        ),
    )
    .unwrap();
}

/// Recent UTC timestamp string in the wire millisecond-Z form. Always
/// "now" within the 24h default session window — used to seed a fresh
/// session for tests that need an attributed publish.
fn fresh_started_at() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.000Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
}

// Drive a sequence of N publishes against one wiremock server; record
// all bodies in order. Used by the batch-mode hint-once test.
struct SequencedCapture {
    bodies: Mutex<Vec<String>>,
    responses: Mutex<VecDeque<ResponseTemplate>>,
}
impl Respond for SequencedCapture {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        self.bodies
            .lock()
            .unwrap()
            .push(String::from_utf8(req.body.clone()).unwrap());
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| ResponseTemplate::new(599))
    }
}

// ─── attributed publish: session file → metadata.actor on wire ─────────────

#[tokio::test]
async fn publish_with_active_session_stamps_metadata_actor_on_envelope() {
    let server = MockServer::start().await;
    let captured = capture_body(&server).await;

    let home = tempfile::tempdir().unwrap();
    write_session_file(&session_path(home.path()), &fresh_started_at(), "claude");

    let out = run_publish(&server.uri(), home.path(), &base_args()).await;
    assert_eq!(out.code, 0, "publish must succeed; stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("body captured");
    let env = parse_envelope(&body);
    assert_eq!(env["metadata"]["actor"]["type"], "agent");
    assert_eq!(env["metadata"]["actor"]["agentId"], "claude");
}

#[tokio::test]
async fn publish_metadata_serialises_between_data_and_source() {
    // Wire field order: type, data, metadata, source, ... Pinned by
    // byte-position lookup on the raw body so a serde derive reordering
    // would be caught immediately.
    let server = MockServer::start().await;
    let captured = capture_body(&server).await;

    let home = tempfile::tempdir().unwrap();
    write_session_file(&session_path(home.path()), &fresh_started_at(), "claude");
    let _ = run_publish(&server.uri(), home.path(), &base_args()).await;

    let body = captured.lock().unwrap().clone().expect("body captured");
    let i_data = body.find(r#""data":"#).expect("data key");
    let i_meta = body.find(r#""metadata":"#).expect("metadata key");
    let i_source = body.find(r#""source":"#).expect("source key");
    assert!(
        i_data < i_meta && i_meta < i_source,
        "field order must be data, metadata, source; body={body}",
    );
}

#[tokio::test]
async fn publish_with_agent_id_flag_stamps_flag_built_actor() {
    let server = MockServer::start().await;
    let captured = capture_body(&server).await;

    let home = tempfile::tempdir().unwrap();
    let mut args = base_args();
    args.extend([
        "--actor-type",
        "agent",
        "--agent-id",
        "github-actions",
        "--model",
        "model-from-flag",
    ]);
    let out = run_publish(&server.uri(), home.path(), &args).await;
    assert_eq!(out.code, 0, "publish must succeed; stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("body captured");
    let env = parse_envelope(&body);
    assert_eq!(env["metadata"]["actor"]["agentId"], "github-actions");
    assert_eq!(env["metadata"]["actor"]["model"], "model-from-flag");
}

#[tokio::test]
async fn publish_per_call_flags_layer_on_session_actor() {
    let server = MockServer::start().await;
    let captured = capture_body(&server).await;

    let home = tempfile::tempdir().unwrap();
    write_session_file(&session_path(home.path()), &fresh_started_at(), "claude");
    let mut args = base_args();
    args.extend([
        "--call-id",
        "call-xyz",
        "--prompt-tokens",
        "1234",
        "--completion-tokens",
        "567",
        "--latency-ms",
        "812",
    ]);
    let _ = run_publish(&server.uri(), home.path(), &args).await;

    let body = captured.lock().unwrap().clone().expect("body captured");
    let env = parse_envelope(&body);
    let actor = &env["metadata"]["actor"];
    assert_eq!(actor["agentId"], "claude", "identity from session");
    assert_eq!(actor["callId"], "call-xyz");
    assert_eq!(actor["promptTokens"], 1234);
    assert_eq!(actor["completionTokens"], 567);
    assert_eq!(actor["latencyMs"], 812);
}

#[tokio::test]
async fn publish_with_session_file_env_var_uses_alt_path() {
    let server = MockServer::start().await;
    let captured = capture_body(&server).await;

    let home = tempfile::tempdir().unwrap();
    let alt = home.path().join("alt-session.json");
    write_session_file(&alt, &fresh_started_at(), "codex");

    let mut cmd = nt_publish_cmd();
    cmd.env("NO_TICKETS_HOME", home.path())
        .env_remove("NO_TICKETS_TOKEN")
        .env_remove("NO_TICKETS_ENV")
        .env("NO_TICKETS_API_URL", server.uri())
        .env("NO_TICKETS_AUTH_URL", "https://unused.example/auth")
        .env("NO_TICKETS_TOKEN", PUSH_TOKEN)
        .env("NO_TICKETS_SESSION_FILE", alt.to_str().unwrap())
        .env("NT_RETRY_BASE_DELAY_MS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("publish");
    for a in base_args() {
        cmd.arg(a);
    }
    let out = cmd.output().await.expect("spawn");
    assert!(out.status.success(), "stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("body captured");
    let env = parse_envelope(&body);
    assert_eq!(env["metadata"]["actor"]["agentId"], "codex");
}

// ─── unattributed publish: no metadata + first-publish hint ───────────────

#[tokio::test]
async fn publish_without_actor_omits_metadata_entirely_from_wire() {
    let server = MockServer::start().await;
    let captured = capture_body(&server).await;

    let home = tempfile::tempdir().unwrap();
    let out = run_publish(&server.uri(), home.path(), &base_args()).await;
    assert_eq!(out.code, 0, "publish must succeed; stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("body captured");
    let env = parse_envelope(&body);
    assert!(
        env.get("metadata").is_none(),
        "metadata key MUST be absent (not `null`) when no actor resolved; got {env}",
    );
    // Defence-in-depth: no `"metadata":null` anywhere in the raw body.
    assert!(
        !body.contains(r#""metadata":null"#),
        "wire body must not contain `\"metadata\":null`; got {body}",
    );
}

#[tokio::test]
async fn publish_without_actor_prints_first_publish_hint_to_stderr() {
    let server = MockServer::start().await;
    let _ = capture_body(&server).await;

    let home = tempfile::tempdir().unwrap();
    let out = run_publish(&server.uri(), home.path(), &base_args()).await;
    assert_eq!(out.code, 0, "publish must succeed; stderr={:?}", out.stderr);
    assert!(
        out.stderr.contains("no-tickets session start"),
        "first unattributed publish must hint at session start; got {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn publish_without_actor_sets_state_json_marker() {
    let server = MockServer::start().await;
    let _ = capture_body(&server).await;

    let home = tempfile::tempdir().unwrap();
    assert!(
        !state_path(home.path()).exists(),
        "precondition: state.json absent",
    );
    let _ = run_publish(&server.uri(), home.path(), &base_args()).await;
    assert!(
        state_path(home.path()).exists(),
        "state.json must be created after first unattributed publish",
    );
    let parsed: Value =
        serde_json::from_str(&fs::read_to_string(state_path(home.path())).unwrap()).unwrap();
    assert_eq!(parsed["firstPublishHintShown"], true);
}

#[tokio::test]
async fn publish_second_unattributed_does_not_re_fire_hint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(HAPPY_RESPONSE.as_bytes(), "application/json"),
        )
        .expect(2)
        .mount(&server)
        .await;

    let home = tempfile::tempdir().unwrap();
    let first = run_publish(&server.uri(), home.path(), &base_args()).await;
    assert!(first.stderr.contains("no-tickets session start"));
    let second = run_publish(&server.uri(), home.path(), &base_args()).await;
    assert!(
        !second.stderr.contains("no-tickets session start"),
        "second invocation must NOT re-fire hint; got stderr={:?}",
        second.stderr,
    );
}

#[tokio::test]
async fn publish_quiet_flag_suppresses_stderr_but_still_sets_marker() {
    let server = MockServer::start().await;
    let _ = capture_body(&server).await;

    let home = tempfile::tempdir().unwrap();
    let mut args = base_args();
    args.push("--quiet");
    let out = run_publish(&server.uri(), home.path(), &args).await;
    assert_eq!(out.code, 0, "publish must succeed; stderr={:?}", out.stderr);

    assert!(
        !out.stderr.contains("no-tickets session start"),
        "--quiet must suppress stderr hint; got {:?}",
        out.stderr,
    );
    // Marker MUST still be set so a future invocation without --quiet
    // doesn't suddenly emit the hint.
    assert!(
        state_path(home.path()).exists(),
        "state.json must be created even under --quiet",
    );
    let parsed: Value =
        serde_json::from_str(&fs::read_to_string(state_path(home.path())).unwrap()).unwrap();
    assert_eq!(parsed["firstPublishHintShown"], true);
}

// ─── interaction with `session end` ────────────────────────────────────────

#[tokio::test]
async fn session_end_clears_marker_so_next_unattributed_publish_re_fires_hint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(HAPPY_RESPONSE.as_bytes(), "application/json"),
        )
        .expect(2)
        .mount(&server)
        .await;

    let home = tempfile::tempdir().unwrap();
    let first = run_publish(&server.uri(), home.path(), &base_args()).await;
    assert!(first.stderr.contains("no-tickets session start"));

    // Run `nt session end` — clears the marker.
    let mut end = nt_publish_cmd();
    end.env("NO_TICKETS_HOME", home.path())
        .env_remove("NO_TICKETS_TOKEN")
        .arg("session")
        .arg("end");
    let end_out = end.output().await.expect("spawn end");
    assert!(end_out.status.success(), "session end must succeed");

    let second = run_publish(&server.uri(), home.path(), &base_args()).await;
    assert!(
        second.stderr.contains("no-tickets session start"),
        "after `session end`, the next unattributed publish must re-fire the hint; \
         got stderr={:?}",
        second.stderr,
    );
}

// ─── session-attributed paths must not touch state.json ────────────────────

#[tokio::test]
async fn publish_with_active_session_does_not_create_state_json() {
    let server = MockServer::start().await;
    let _ = capture_body(&server).await;

    let home = tempfile::tempdir().unwrap();
    write_session_file(&session_path(home.path()), &fresh_started_at(), "claude");
    assert!(
        !state_path(home.path()).exists(),
        "precondition: state.json absent",
    );

    let _ = run_publish(&server.uri(), home.path(), &base_args()).await;
    assert!(
        !state_path(home.path()).exists(),
        "session-attributed publish MUST NOT create state.json — hint path is gated on the no-actor branch",
    );
}

#[tokio::test]
async fn publish_with_active_session_does_not_modify_existing_state_json() {
    let server = MockServer::start().await;
    let _ = capture_body(&server).await;

    let home = tempfile::tempdir().unwrap();
    write_session_file(&session_path(home.path()), &fresh_started_at(), "claude");

    // Seed an unrelated state.json with the hint flag explicitly false.
    let state_p = state_path(home.path());
    fs::create_dir_all(state_p.parent().unwrap()).unwrap();
    fs::write(
        &state_p,
        r#"{"firstPublishHintShown":false,"experimental":{"k":"v"}}"#,
    )
    .unwrap();
    let before = fs::read_to_string(&state_p).unwrap();

    let _ = run_publish(&server.uri(), home.path(), &base_args()).await;
    let after = fs::read_to_string(&state_p).unwrap();
    assert_eq!(
        before, after,
        "session-attributed publish must not modify state.json",
    );
}

// ─── once-per-invocation (batch parity, not per-event) ─────────────────────

#[tokio::test]
async fn unattributed_publish_resolves_actor_once_per_invocation() {
    // Single-event publish proxies for the "once per CLI invocation"
    // contract. The state.json open count + marker write count must be
    // exactly 1 per binary invocation, regardless of how many envelopes
    // ultimately land. Pin via file-existence + size on a one-shot
    // publish; the batch variant lives in `publish/batch.rs` follow-up.
    let server = MockServer::start().await;
    let _ = capture_body(&server).await;

    let home = tempfile::tempdir().unwrap();
    let _ = run_publish(&server.uri(), home.path(), &base_args()).await;
    let first_size = fs::metadata(state_path(home.path())).unwrap().len();
    assert!(first_size > 0, "state.json must have content");

    // Re-run on the SAME home: now the marker is set, so resolve+hint
    // is a fast no-op (just a read on state.json, no write). Pin that
    // by asserting the file is byte-identical post-second-invocation.
    let before = fs::read_to_string(state_path(home.path())).unwrap();
    let _ = run_publish(&server.uri(), home.path(), &base_args()).await;
    let after = fs::read_to_string(state_path(home.path())).unwrap();
    assert_eq!(
        before, after,
        "subsequent unattributed publishes must not re-write state.json",
    );
}

#[tokio::test]
async fn unattributed_sequenced_publishes_capture_no_metadata() {
    // Mount a single mock that handles two sequential publishes and
    // records both bodies. Both must lack `metadata` (no actor) and
    // serialise identically apart from any retry/timing differences.
    let server = MockServer::start().await;
    let cap = std::sync::Arc::new(SequencedCapture {
        bodies: Mutex::new(Vec::new()),
        responses: Mutex::new(VecDeque::from(vec![
            ResponseTemplate::new(200).set_body_raw(HAPPY_RESPONSE.as_bytes(), "application/json"),
            ResponseTemplate::new(200).set_body_raw(HAPPY_RESPONSE.as_bytes(), "application/json"),
        ])),
    });
    let cap_arc = cap.clone();
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(move |req: &Request| cap_arc.respond(req))
        .expect(2)
        .mount(&server)
        .await;

    let home = tempfile::tempdir().unwrap();
    let _ = run_publish(&server.uri(), home.path(), &base_args()).await;
    let _ = run_publish(&server.uri(), home.path(), &base_args()).await;

    let bodies = cap.bodies.lock().unwrap().clone();
    assert_eq!(bodies.len(), 2);
    for body in &bodies {
        let env = parse_envelope(body);
        assert!(
            env.get("metadata").is_none(),
            "every unattributed publish must omit metadata; got {env}",
        );
    }
}
