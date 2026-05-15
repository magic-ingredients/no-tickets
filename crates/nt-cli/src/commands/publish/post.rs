//! HTTP orchestration: serialise the envelope, post via the retry
//! transport, map the response (or transport error) to an i32 exit
//! code. Body serialisation lives here rather than at the transport
//! boundary because the wire format (single-element JSON array of
//! envelopes) is a publish-flow concern, not a transport-layer concern.

use serde_json::Value;

use crate::transport::{post_json_with_retry, HttpClient, RetryPolicy, Sleeper};

use super::envelope::{build_envelope, EventMetadata};

/// Stateless core: takes an injected `HttpClient`, the resolved + parsed
/// inputs, sends the publish request, maps the result to an exit code.
///
/// Production wires `Client` (reqwest); tests wire a `FakeHttpClient`
/// that records the call and returns canned responses, enabling
/// in-process coverage of the error-mapping branches without
/// subprocess-plus-wiremock. The integration tests in `tests/publish.rs`
/// still own the end-to-end transport-level coverage (real reqwest, real TLS).
pub(super) async fn publish_event<C: HttpClient, S: Sleeper>(
    client: &C,
    policy: &RetryPolicy,
    sleeper: &S,
    type_id: &str,
    data: &Value,
    meta: EventMetadata<'_>,
) -> i32 {
    let body = vec![build_envelope(type_id, data, meta)];
    // serde_json::to_vec on `Vec<EventEnvelope>` cannot fail — every
    // field is a primitive Serialize impl over owned/borrowed data —
    // so .expect is appropriate here. A panic would indicate a bug
    // in serde, not a runtime condition.
    let body_bytes = serde_json::to_vec(&body).expect("envelope vec always serialises");
    match post_json_with_retry(client, policy, sleeper, "/v1/events", &body_bytes).await {
        Ok(response) => {
            // Server response shape: `{ ingested, deduped, ids }`.
            // serde_json::Value serialisation cannot fail for valid
            // Value, so `.expect` is appropriate here.
            println!(
                "{}",
                serde_json::to_string(&response).expect("serde_json::Value always serialises"),
            );
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::envelope::bare_meta;
    use super::*;
    use crate::transport::TransportError;
    use serde_json::json;
    use std::num::NonZeroU32;
    use std::sync::Mutex;
    use std::time::Duration;

    /// No-retry policy + no-op sleeper for the publish-orchestration
    /// tests below. Each test cares about a single `publish_event`
    /// branch (success / 401 / 422 / 5xx / Config); retry behaviour is
    /// owned by `transport::retry_tests` and exercised end-to-end via
    /// `tests/publish.rs`. Using `max_attempts = 1` keeps the
    /// `FakeHttpClient` response queue size at one and surfaces the
    /// recorded error verbatim, matching the original pre-Task-17 shape.
    fn no_retry() -> RetryPolicy {
        RetryPolicy {
            max_attempts: NonZeroU32::new(1).expect("1 is non-zero"),
            base_delay: Duration::from_millis(0),
            max_delay: Duration::from_millis(0),
        }
    }

    struct NoopSleeper;
    impl Sleeper for NoopSleeper {
        async fn sleep(&self, _: Duration) {
            // `max_attempts=1` means this is never invoked, but the
            // trait requires an impl.
        }
    }

    /// Records every `post_json` call and returns canned responses.
    /// One canned response per call, in FIFO order; missing canned
    /// responses panic so a test mismatch fails loudly.
    struct FakeHttpClient {
        responses: Mutex<Vec<Result<Value, TransportError>>>,
        calls: Mutex<Vec<RecordedCall>>,
    }

    #[derive(Clone, Debug)]
    struct RecordedCall {
        path: String,
        body: Vec<u8>,
    }

    impl FakeHttpClient {
        fn with_response(response: Result<Value, TransportError>) -> Self {
            Self {
                responses: Mutex::new(vec![response]),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<RecordedCall> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl HttpClient for FakeHttpClient {
        async fn post_json(&self, path: &str, body: Vec<u8>) -> Result<Value, TransportError> {
            self.calls.lock().unwrap().push(RecordedCall {
                path: path.to_string(),
                body,
            });
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                panic!(
                    "FakeHttpClient: no canned response left for call to {path:?}. \
                     Did the test set up enough .with_response() entries?",
                );
            }
            responses.remove(0)
        }
    }

    // ─── publish_event: orchestration tests via injected HttpClient ──────
    //
    // Coverage note: the success-path stdout content (`println!` of the
    // server response) is exercised end-to-end by
    // `publish_sends_post_to_v1_events_with_bearer_header_and_prints_response`
    // and `publish_response_passes_through_unknown_fields` in
    // tests/publish.rs. Capturing process stdout from inside the same
    // process requires either fd-level redirection or refactoring
    // println! through a Write-trait seam — both heavier than the
    // integration coverage that already pins the behaviour. The
    // in-process tests below cover branch selection (exit codes +
    // call shape); the integration tests own the stdout/stderr
    // contract.

    #[tokio::test]
    async fn publish_event_returns_zero_on_2xx_response() {
        let fake = FakeHttpClient::with_response(Ok(json!({
            "ingested": 1, "deduped": 0, "ids": ["evt_1"],
        })));
        let data = json!({ "taskId": "t-1" });
        let code = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "ai.task.completed.v1",
            &data,
            bare_meta("demo"),
        )
        .await;
        assert_eq!(code, 0, "2xx must map to exit code 0");
    }

    #[tokio::test]
    async fn publish_event_posts_to_v1_events_path() {
        let fake = FakeHttpClient::with_response(Ok(json!({
            "ingested": 1, "deduped": 0, "ids": ["x"],
        })));
        let data = json!({});
        let _ = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.z.v1",
            &data,
            bare_meta("demo"),
        )
        .await;
        let calls = fake.calls();
        assert_eq!(calls.len(), 1, "exactly one HTTP call");
        assert_eq!(calls[0].path, "/v1/events", "must POST to /v1/events");
    }

    #[tokio::test]
    async fn publish_event_posts_serialised_envelope_array() {
        let fake = FakeHttpClient::with_response(Ok(json!({
            "ingested": 1, "deduped": 0, "ids": ["x"],
        })));
        let data = json!({ "taskId": "t-1" });
        let _ = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "ai.task.completed.v1",
            &data,
            bare_meta("my-project"),
        )
        .await;
        let body_bytes = fake.calls()[0].body.clone();
        let body_str = String::from_utf8(body_bytes).expect("utf8 body");
        // Single-element array wrapping the envelope
        assert!(
            body_str.starts_with('['),
            "wire body starts with [: {body_str}"
        );
        assert!(body_str.ends_with(']'), "wire body ends with ]: {body_str}");
        // Exactly one envelope — pins the "single-event" invariant so
        // a double-wrap regression (`[[{...}]]`) or accidental
        // batching can't slip through.
        assert_eq!(
            body_str.matches(r#""type":"#).count(),
            1,
            "wire body must contain exactly one envelope: {body_str}",
        );
        // Envelope fields present
        assert!(body_str.contains(r#""type":"ai.task.completed.v1""#));
        assert!(body_str.contains(r#""name":"nt-cli""#));
        assert!(body_str.contains(r#""project":"my-project""#));
    }

    #[tokio::test]
    async fn publish_event_returns_one_on_http_401_response() {
        let fake = FakeHttpClient::with_response(Err(TransportError::HttpStatus {
            status: 401,
            body: r#"{"error":"unauthorized"}"#.to_string(),
        }));
        let code = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.v1",
            &json!({}),
            bare_meta("demo"),
        )
        .await;
        assert_eq!(code, 1, "401 must map to exit code 1");
    }

    #[tokio::test]
    async fn publish_event_returns_one_on_http_422_response() {
        let fake = FakeHttpClient::with_response(Err(TransportError::HttpStatus {
            status: 422,
            body: r#"{"error":"validation_error"}"#.to_string(),
        }));
        let code = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.v1",
            &json!({}),
            bare_meta("demo"),
        )
        .await;
        assert_eq!(code, 1, "422 must map to exit code 1");
    }

    #[tokio::test]
    async fn publish_event_returns_one_on_http_5xx_response() {
        let fake = FakeHttpClient::with_response(Err(TransportError::HttpStatus {
            status: 503,
            body: String::new(),
        }));
        let code = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.v1",
            &json!({}),
            bare_meta("demo"),
        )
        .await;
        assert_eq!(code, 1, "5xx must map to exit code 1");
    }

    #[tokio::test]
    async fn publish_event_returns_one_on_config_error() {
        let fake = FakeHttpClient::with_response(Err(TransportError::Config(
            "invalid path \"/v1/events\"".to_string(),
        )));
        let code = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.v1",
            &json!({}),
            bare_meta("demo"),
        )
        .await;
        assert_eq!(code, 1, "Config error must map to exit code 1");
    }
}
