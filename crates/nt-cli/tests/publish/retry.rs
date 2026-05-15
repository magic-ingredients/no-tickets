//! Task 17: retry/backoff on transient failures. Pinned end-to-end at
//! the binary boundary — the unit tests in `transport::retry_tests`
//! own the schedule pins (sleeper recordings); these tests verify that
//! the retry loop is wired into the publish path AND that the
//! transient/terminal classifier holds at the integration layer.

use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::{base_args_with_data, run_nt_publish, tempdir, SequencedResponder};

/// Persistent 5xx exhausts the retry budget. Server is hit `max_attempts`
/// times (3 per the production default); final exit is non-zero with the
/// LAST attempt's status in stderr.
///
/// Replaces the original Task-14 single-attempt 5xx test — that
/// behaviour is now wrong: 5xx is a transient class, retried by
/// `post_json_with_retry`. The give-up path is what's pinned here.
#[tokio::test]
async fn publish_persistent_5xx_retries_then_gives_up_with_last_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(503))
        // `expect(3)` asserts the binary made exactly 3 attempts —
        // matches `RetryPolicy::default_publish().max_attempts`. A
        // regression that turns retry off (or, worse, doubles it) will
        // fail wiremock's drop-time check.
        .expect(3)
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
    assert_ne!(out.code, 0, "persistent 5xx must produce non-zero exit");
    assert!(
        out.stderr.contains("503")
            || out.stderr.to_lowercase().contains("server")
            || out.stderr.to_lowercase().contains("transport"),
        "stderr must name the server error; got {:?}",
        out.stderr,
    );
}

/// A single 503 followed by 200 must produce a zero exit — proves the
/// retry loop is actually wired into the publish path end-to-end, not
/// just unit-tested in transport.rs. Pins both branches of the retry
/// policy at the binary boundary: transient classification AND
/// give-up-not-reached short-circuit on success.
#[tokio::test]
async fn publish_retries_5xx_then_succeeds_on_200() {
    let server = MockServer::start().await;
    let responder = SequencedResponder::new(vec![
        ResponseTemplate::new(503),
        ResponseTemplate::new(200).set_body_json(json!({
            "ingested": 1,
            "deduped": 0,
            "ids": ["evt_after_retry"],
        })),
    ]);
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(responder)
        .expect(2)
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
    assert_eq!(
        out.code, 0,
        "5xx then 200 must succeed; stderr={:?}",
        out.stderr,
    );
    let body: Value = serde_json::from_str(out.stdout.trim()).expect("stdout JSON");
    assert_eq!(body["ids"][0], "evt_after_retry");
}

/// 4xx responses must NOT be retried — they're terminal. Server is hit
/// exactly once; any retry would burn server-side rate limits / dedupe
/// keys / quota for a class of failure that won't change on a second
/// attempt. Regression pin for the classifier boundary.
#[tokio::test]
async fn publish_persistent_4xx_does_not_retry() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({
            "error": "validation_error",
            "message": "type is required",
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
    assert_ne!(out.code, 0, "4xx must produce non-zero exit");
    assert!(
        out.stderr.to_lowercase().contains("validation")
            || out.stderr.contains("422")
            || out.stderr.to_lowercase().contains("server"),
        "stderr must surface the server validation message; got {:?}",
        out.stderr,
    );
}
