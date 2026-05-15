//! Happy-path wire-shape pins: POST `/v1/events` with Bearer header,
//! single-element JSON array body, `type, data, source` field order.

use serde_json::{json, Value};
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::{capture_publish_body, run_nt_publish, tempdir};

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
    let body: Value = serde_json::from_str(out.stdout.trim()).expect("stdout is JSON");
    assert_eq!(body["ingested"], 1);
    assert_eq!(body["deduped"], 0);
    assert_eq!(body["ids"][0], "evt_abc123");
}

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

/// Inspects the raw request body bytes and asserts `type`, `data`,
/// `source` appear in that declaration order. Same monotonic-byte-
/// position approach as the nt status and list_event_types tests.
#[tokio::test]
async fn publish_wire_body_field_order_is_type_data_source() {
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

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
