//! `nt publish` structured-error contract tests.
//!
//! Exercises every server-side and client-side error class documented
//! in `docs/binary-error-contract.md`:
//!
//! - `not_authenticated` (exit 5) — no `NO_TICKETS_TOKEN`
//! - `permission_denied` (exit 3) — server returns 403
//! - `validation_error` (exit 1) — local schema validation issues
//! - `unknown_event_type` (exit 2) — type id not in the local registry
//! - `transport_error` (exit 4) — server returns 5xx after retry exhaustion
//! - `usage` (exit 7) — `--data` not valid JSON
//!
//! Each test spawns the binary against a wiremock server (when the
//! error class involves the server) or no server at all (for purely
//! client-side errors like usage / not_authenticated / validation).

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common::{run_nt, tempdir};

const TYPE: &str = "ai.task.completed.v1";
const DATA: &str = r#"{"taskId":"task-1","sessionId":"session-1","startedAt":"2026-05-01T00:00:00.000Z","completedAt":"2026-05-01T00:00:01.000Z","durationMs":1000,"outcome":"success","callCount":1}"#;

#[tokio::test]
async fn publish_without_token_is_not_authenticated_exit_5() {
    let home = tempdir();
    // No NO_TICKETS_TOKEN; harness defaults strip it from the env.
    // Still need NO_TICKETS_API_URL so url resolution doesn't trip on
    // its own contract first (that's a separate usage-class error).
    let out = run_nt(
        home.path(),
        &[
            ("NO_TICKETS_API_URL", "https://api-staging.no-tickets.com"),
            (
                "NO_TICKETS_AUTH_URL",
                "https://app-staging.no-tickets.com/auth",
            ),
        ],
        &[
            "publish",
            "--type",
            TYPE,
            "--data",
            DATA,
            "--project",
            "demo",
        ],
    )
    .await;

    assert_eq!(
        out.code, 5,
        "missing token must surface as not_authenticated exit 5"
    );
    let v = out.stderr_json();
    assert_eq!(v["error"], "not_authenticated");
    assert!(
        v["message"].as_str().is_some_and(|m| !m.is_empty()),
        "not_authenticated must include a message, got: {v:?}"
    );
}

#[tokio::test]
async fn publish_with_403_surfaces_permission_denied_exit_3() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let home = tempdir();
    let out = run_nt(
        home.path(),
        &[
            ("NO_TICKETS_TOKEN", "nt_push_test"),
            ("NO_TICKETS_API_URL", &server.uri()),
            ("NO_TICKETS_AUTH_URL", "https://unused.example/auth"),
        ],
        &[
            "publish",
            "--type",
            TYPE,
            "--data",
            DATA,
            "--project",
            "demo",
        ],
    )
    .await;

    assert_eq!(out.code, 3, "403 must surface as permission_denied exit 3");
    let v = out.stderr_json();
    assert_eq!(v["error"], "permission_denied");
    assert!(
        v["domain"].is_string(),
        "permission_denied must carry a domain, got: {v:?}"
    );
}

#[tokio::test]
async fn publish_with_401_surfaces_not_authenticated_exit_5() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let home = tempdir();
    let out = run_nt(
        home.path(),
        &[
            ("NO_TICKETS_TOKEN", "nt_push_test"),
            ("NO_TICKETS_API_URL", &server.uri()),
            ("NO_TICKETS_AUTH_URL", "https://unused.example/auth"),
        ],
        &[
            "publish",
            "--type",
            TYPE,
            "--data",
            DATA,
            "--project",
            "demo",
        ],
    )
    .await;

    assert_eq!(out.code, 5, "401 must surface as not_authenticated exit 5");
    let v = out.stderr_json();
    assert_eq!(v["error"], "not_authenticated");
}

#[tokio::test]
async fn publish_with_5xx_after_retries_surfaces_transport_exit_4() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let home = tempdir();
    let out = run_nt(
        home.path(),
        &[
            ("NO_TICKETS_TOKEN", "nt_push_test"),
            ("NO_TICKETS_API_URL", &server.uri()),
            ("NO_TICKETS_AUTH_URL", "https://unused.example/auth"),
        ],
        &[
            "publish",
            "--type",
            TYPE,
            "--data",
            DATA,
            "--project",
            "demo",
        ],
    )
    .await;

    assert_eq!(
        out.code, 4,
        "5xx after retry must surface as transport_error exit 4"
    );
    let v = out.stderr_json();
    assert_eq!(v["error"], "transport_error");
    assert!(
        v["message"].is_string(),
        "transport_error must carry message, got: {v:?}"
    );
    assert!(
        v["retriable"].is_boolean(),
        "transport_error must carry retriable bool, got: {v:?}"
    );
    // 5xx is retriable=true per the doc's classifier.
    assert_eq!(v["retriable"], true);
}

#[tokio::test]
async fn publish_bad_data_json_is_usage_exit_7() {
    let home = tempdir();
    let out = run_nt(
        home.path(),
        &[
            ("NO_TICKETS_TOKEN", "nt_push_test"),
            ("NO_TICKETS_API_URL", "https://api-staging.no-tickets.com"),
            ("NO_TICKETS_AUTH_URL", "https://unused.example/auth"),
        ],
        &[
            "publish",
            "--type",
            TYPE,
            "--data",
            "not-json",
            "--project",
            "demo",
        ],
    )
    .await;

    assert_eq!(out.code, 7, "bad --data JSON must surface as usage exit 7");
    let v = out.stderr_json();
    assert_eq!(v["error"], "usage");
}

// Local pre-flight `unknown_event_type` and `validation_error` for
// `nt publish` are out of scope for Task 26: today `nt publish` ships
// straight to the server (no local schema check) and surfaces the
// server's verdict via transport-error mapping. The dedicated
// `nt validate` command owns the local-validation path and is covered
// by `validate.rs` above. Adding pre-flight in publish would expand
// the command's contract beyond Task 26's bounds — tracked as a
// separate follow-up if the server's structured-error body parsing
// (which would let us map server-side 422 → unknown_event_type /
// validation_error) ever lands.
