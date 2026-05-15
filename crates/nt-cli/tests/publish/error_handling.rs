//! Auth, server-error response mapping, network failure, malformed
//! input — every non-retry failure mode through the single-event
//! publish path.

use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::{base_args_with_data, run_nt_publish, tempdir};

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
        &base_args_with_data("{}"),
    )
    .await;
    assert_ne!(
        out.code, 0,
        "missing token must fail; stdout={:?}",
        out.stdout
    );
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
        &base_args_with_data("{}"),
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
        &base_args_with_data("{}"),
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
        &base_args_with_data("{}"),
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
        &base_args_with_data("{}"),
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
        &base_args_with_data("{}"),
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

/// ADR-0002 Task 3: when the credentials file's `host` doesn't match the
/// publish target's api_url, the session is stale and must be declined
/// with a stderr warning — same contract as `nt status`. Pinned at the
/// integration layer so a regression in `publish.rs` (where the warning
/// emission lives next to the same eprintln in `status.rs`) can't pass
/// the publish suite while breaking the contract.
#[tokio::test]
async fn publish_session_host_mismatch_emits_warning_and_declines_session() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // must NOT be hit — stale session declined before transport
        .mount(&server)
        .await;

    let home = tempdir();
    // Write a credentials file whose host == staging, but the publish
    // command will resolve to `server.uri()` (the wiremock URL).
    let dir = home.path().join(".notickets");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("credentials"),
        r#"{"token":"nt_session_staging","email":"a@b.com","expiresAt":"2099-01-01T00:00:00.000Z","host":"https://api-staging.no-tickets.com"}"#,
    )
    .unwrap();

    // No env token → publish must fall back to credentials → mismatch fires.
    let out = run_nt_publish(
        &server.uri(),
        None,
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

    assert_ne!(out.code, 0, "publish must fail; got {:?}", out);
    assert!(out.stderr.contains("Warning:"), "got: {:?}", out.stderr);
    assert!(
        out.stderr.contains("https://api-staging.no-tickets.com"),
        "stored host must be named; got: {:?}",
        out.stderr,
    );
    assert!(
        out.stderr.contains("re-authenticate"),
        "warning must tell the user to re-init; got: {:?}",
        out.stderr,
    );
    assert!(
        !out.stderr.contains("nt_session_staging"),
        "token MUST NOT leak into the warning; got: {:?}",
        out.stderr,
    );
}

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
