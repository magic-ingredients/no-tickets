//! `nt publish` integration tests via wiremock.
//!
//! Mirrors `src/transport/events.ts::publish` and `src/transport/
//! client.ts::request`: POST `/v1/events` with Bearer auth, single-
//! element JSON array body, `{ ingested, deduped, ids }` response.

use std::path::Path;
use std::process::Stdio;

use assert_cmd::cargo::cargo_bin;
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Output {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Run `nt publish` with the wiremock URL, an isolated NO_TICKETS_HOME,
/// and an env-supplied bearer token. Returns the captured exit + IO.
async fn run_nt_publish(
    server_uri: &str,
    token: Option<&str>,
    home: &Path,
    args: &[&str],
) -> Output {
    let mut cmd = Command::new(cargo_bin("nt"));
    cmd.env("NO_TICKETS_HOME", home)
        .env("NO_TICKETS_API_URL", server_uri)
        // The url-resolver enforces both env vars must be set together;
        // give it a placeholder for AUTH (publish never reads it).
        .env("NO_TICKETS_AUTH_URL", "https://unused.example/auth")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(t) = token {
        cmd.env("NO_TICKETS_TOKEN", t);
    } else {
        cmd.env_remove("NO_TICKETS_TOKEN");
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

fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

// ─── Happy path: POST /v1/events with Bearer header, response on stdout ────

#[tokio::test]
async fn publish_sends_post_to_v1_events_with_bearer_header_and_prints_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .and(header("authorization", "Bearer nt_push_test_token"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ingested": 1,
            "deduped": 0,
            "ids": ["evt_abc123"],
        })))
        .expect(1)
        .mount(&server)
        .await;

    let home = tempdir();
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_test_token"),
        home.path(),
        &[
            "--type",
            "ai.task.completed.v1",
            "--data",
            r#"{"taskId":"t-1"}"#,
            "--project",
            "demo",
        ],
    )
    .await;

    assert_eq!(out.code, 0, "expected success; stderr={:?}", out.stderr);
    let body: Value =
        serde_json::from_str(out.stdout.trim()).expect("stdout is JSON");
    assert_eq!(body["ingested"], 1);
    assert_eq!(body["deduped"], 0);
    assert_eq!(body["ids"][0], "evt_abc123");
}

// ─── Wire-shape: body is a single-element JSON array with the right fields ─

#[tokio::test]
async fn publish_request_body_is_single_element_array_with_event_envelope() {
    let server = MockServer::start().await;
    // body_partial_json asserts the request body is a JSON array with
    // at least one element matching the partial shape. Pin every field
    // the TS sourceSchema requires (name + sdkVersion) PLUS the
    // attributes.project escape-hatch used to surface caller project
    // context — all required for TS parity, all checked here.
    let body_partial = json!([{
        "type": "ai.task.completed.v1",
        "source": {
            "name": "nt-cli",
            "sdkVersion": env!("CARGO_PKG_VERSION"),
            "attributes": { "project": "demo" }
        }
    }]);
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .and(body_partial_json(body_partial))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ingested": 1, "deduped": 0, "ids": ["x"],
        })))
        .expect(1)
        .mount(&server)
        .await;

    let home = tempdir();
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &[
            "--type",
            "ai.task.completed.v1",
            "--data",
            r#"{"taskId":"t-1","sessionId":"s-1"}"#,
            "--project",
            "demo",
        ],
    )
    .await;
    assert_eq!(out.code, 0, "wire-shape mismatch; stderr={:?}", out.stderr);
}

// ─── Pin field order on the wire body for TS parity ───────────────────────

/// Inspects the raw request body bytes and asserts `type`, `data`,
/// `source` appear in that declaration order. Same monotonic-byte-
/// position approach as the nt status and list_event_types tests.
/// Capture is synchronous inside the responder closure (no spawn +
/// sleep race) using a std::sync::Mutex.
#[tokio::test]
async fn publish_wire_body_field_order_is_type_data_source() {
    use std::sync::{Arc, Mutex};

    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let captured_for_responder = captured.clone();

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(move |req: &wiremock::Request| {
            // wiremock invokes this closure synchronously per request.
            // Take the body inline; no spawn, no race.
            let body = String::from_utf8(req.body.clone()).expect("body utf8");
            *captured_for_responder.lock().unwrap() = Some(body);
            ResponseTemplate::new(200).set_body_json(json!({
                "ingested": 1, "deduped": 0, "ids": ["x"],
            }))
        })
        .expect(1)
        .mount(&server)
        .await;

    let home = tempdir();
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &[
            "--type",
            "ai.task.completed.v1",
            "--data",
            // Caller payload deliberately distinctive — no "type" /
            // "source" keys hiding inside data to fool the find()
            // calls below.
            r#"{"taskId":"t-1","sessionId":"s-1"}"#,
            "--project",
            "demo",
        ],
    )
    .await;
    assert_eq!(out.code, 0, "unexpected failure: stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("captured body");
    // Envelope-distinctive matchers. The test payload is deliberately
    // chosen to contain NO `type`, `data`, or `source` keys (a
    // taskId/sessionId object), so each of these substrings appears
    // exactly once in the body — at envelope level. Full-value matches
    // on type and source for extra confidence; "data":{ for the data
    // opening (the value's internal key order may vary).
    let p = |needle: &str| {
        body.find(needle)
            .unwrap_or_else(|| panic!("missing {needle:?} in {body:?}"))
    };
    let t = p(r#""type":"ai.task.completed.v1""#);
    let d = p(r#""data":{"#);
    let s = p(r#""source":{"name":"nt-cli""#);
    assert!(
        t < d && d < s,
        "wire field order must be type, data, source — got {body}",
    );
}

// ─── Missing token short-circuits BEFORE any request ─────────────────────

#[tokio::test]
async fn publish_with_no_token_fails_before_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // must NOT be hit
        .mount(&server)
        .await;

    let home = tempdir();
    let out = run_nt_publish(
        &server.uri(),
        None, // no token
        home.path(),
        &[
            "--type",
            "ai.task.completed.v1",
            "--data",
            "{}",
            "--project",
            "demo",
        ],
    )
    .await;
    assert_ne!(out.code, 0, "missing token must fail; stdout={:?}", out.stdout);
    assert!(
        out.stderr.contains("Not authenticated"),
        "stderr must surface the auth error; got {:?}",
        out.stderr,
    );
}

// ─── Error response mapping ───────────────────────────────────────────────

#[tokio::test]
async fn publish_401_response_surfaces_auth_failure_on_stderr() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": "unauthorized",
            "message": "token rejected",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let home = tempdir();
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_bad"),
        home.path(),
        &[
            "--type",
            "ai.task.completed.v1",
            "--data",
            "{}",
            "--project",
            "demo",
        ],
    )
    .await;
    assert_ne!(out.code, 0, "401 must produce non-zero exit");
    assert!(
        out.stderr.to_lowercase().contains("401")
            || out.stderr.contains("unauthorized")
            || out.stderr.contains("token rejected"),
        "stderr must name the 401 / auth failure; got {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn publish_403_response_surfaces_permission_denied_on_stderr() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "error": "permission_denied",
            "message": "project access denied",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let home = tempdir();
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_demo"),
        home.path(),
        &[
            "--type",
            "ai.task.completed.v1",
            "--data",
            "{}",
            "--project",
            "demo",
        ],
    )
    .await;
    assert_ne!(out.code, 0);
    assert!(
        out.stderr.contains("403") || out.stderr.contains("permission"),
        "stderr must name the 403 / permission failure; got {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn publish_4xx_validation_error_surfaces_server_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({
            "error": "validation_error",
            "typeId": "ai.task.completed.v1",
            "issues": [{ "path": "taskId", "message": "required" }],
        })))
        .expect(1)
        .mount(&server)
        .await;

    let home = tempdir();
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_demo"),
        home.path(),
        &[
            "--type",
            "ai.task.completed.v1",
            "--data",
            "{}",
            "--project",
            "demo",
        ],
    )
    .await;
    assert_ne!(out.code, 0, "422 must produce non-zero exit");
    assert!(
        out.stderr.contains("validation_error")
            || out.stderr.contains("422")
            || out.stderr.contains("taskId"),
        "stderr must surface the server validation message; got {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn publish_5xx_response_maps_to_transport_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;

    let home = tempdir();
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_demo"),
        home.path(),
        &[
            "--type",
            "ai.task.completed.v1",
            "--data",
            "{}",
            "--project",
            "demo",
        ],
    )
    .await;
    assert_ne!(out.code, 0, "5xx must produce non-zero exit");
    assert!(
        out.stderr.contains("503")
            || out.stderr.to_lowercase().contains("server")
            || out.stderr.to_lowercase().contains("transport"),
        "stderr must name the server error; got {:?}",
        out.stderr,
    );
}

// ─── Response passthrough: unknown fields survive to stdout ───────────────

/// Server may add new fields to the response over time (forward-compat).
/// The CLI must not drop them — pin the passthrough behaviour so a
/// future "let's parse into a typed struct" change doesn't silently
/// lose information.
#[tokio::test]
async fn publish_response_passes_through_unknown_fields() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ingested": 1,
            "deduped": 0,
            "ids": ["evt_1"],
            "futureField": "preserved",
            "anotherFutureField": 42,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let home = tempdir();
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &[
            "--type",
            "ai.task.completed.v1",
            "--data",
            "{}",
            "--project",
            "demo",
        ],
    )
    .await;
    assert_eq!(out.code, 0, "expected success; stderr={:?}", out.stderr);
    let body: Value = serde_json::from_str(out.stdout.trim()).unwrap();
    assert_eq!(body["futureField"], "preserved");
    assert_eq!(body["anotherFutureField"], 42);
    assert_eq!(body["ingested"], 1);
}

// ─── Network failure (connection refused) maps to transport error ─────────

#[tokio::test]
async fn publish_connection_refused_maps_to_transport_error() {
    // No wiremock instance running on this URL — TCP connect refuses.
    let home = tempdir();
    let out = run_nt_publish(
        "http://127.0.0.1:1", // port 1 is reserved + closed
        Some("nt_push_token"),
        home.path(),
        &[
            "--type",
            "ai.task.completed.v1",
            "--data",
            "{}",
            "--project",
            "demo",
        ],
    )
    .await;
    assert_ne!(
        out.code, 0,
        "connection refused must produce non-zero exit; stdout={:?} stderr={:?}",
        out.stdout, out.stderr,
    );
    assert!(
        out.stderr.to_lowercase().contains("transport")
            || out.stderr.to_lowercase().contains("connect")
            || out.stderr.to_lowercase().contains("refused"),
        "stderr must surface the network failure; got {:?}",
        out.stderr,
    );
}

// ─── --data must be valid JSON; reject early without a request ────────────

#[tokio::test]
async fn publish_with_malformed_data_fails_before_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // must NOT be hit
        .mount(&server)
        .await;

    let home = tempdir();
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_demo"),
        home.path(),
        &[
            "--type",
            "ai.task.completed.v1",
            "--data",
            "{ this is not json",
            "--project",
            "demo",
        ],
    )
    .await;
    assert_ne!(out.code, 0);
    assert!(
        out.stderr.to_lowercase().contains("json"),
        "stderr must name the JSON parse failure; got {:?}",
        out.stderr,
    );
}
